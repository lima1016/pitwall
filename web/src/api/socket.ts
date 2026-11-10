import type { BoardEvent } from './types.ts'

export type ConnectionState = 'connecting' | 'open' | 'closed'

export interface SocketOptions {
  url: string
  token: string
  onEvent: (event: BoardEvent) => void
  onState: (state: ConnectionState) => void
  /** 테스트에서 바꿔 끼운다 */
  createSocket?: (url: string, protocols: string[]) => WebSocket
  /** 재연결 대기 (ms). 시도할수록 늘어나고 최대 10초 */
  backoff?: (attempt: number) => number
}

export const defaultBackoff = (attempt: number) => Math.min(10_000, 500 * 2 ** attempt)

/**
 * 서버 이벤트 스트림. 끊기면 자동으로 다시 연결한다.
 * 토큰은 `Sec-WebSocket-Protocol: pitwall, <token>`으로 보낸다 (DESIGN.md 7장).
 * 다시 연결하면 서버가 전체 세션 스냅샷을 먼저 보내므로 따로 동기화할 필요가 없다.
 */
export function connectBoard(opts: SocketOptions): () => void {
  const create = opts.createSocket ?? ((url, protocols) => new WebSocket(url, protocols))
  const backoff = opts.backoff ?? defaultBackoff
  let attempt = 0
  let stopped = false
  let socket: WebSocket | null = null
  let timer: ReturnType<typeof setTimeout> | undefined

  const open = () => {
    if (stopped) return
    opts.onState('connecting')
    socket = create(opts.url, ['pitwall', opts.token])
    socket.onopen = () => {
      attempt = 0
      opts.onState('open')
    }
    socket.onmessage = (msg) => {
      try {
        opts.onEvent(JSON.parse(String(msg.data)) as BoardEvent)
      } catch {
        // 알 수 없는 메시지는 무시한다
      }
    }
    socket.onclose = () => {
      socket = null
      if (stopped) return
      opts.onState('closed')
      timer = setTimeout(open, backoff(attempt++))
    }
  }

  open()
  return () => {
    stopped = true
    clearTimeout(timer)
    socket?.close()
  }
}

export function boardSocketUrl(): string {
  const proto = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
  return `${proto}//${window.location.host}/ws`
}
