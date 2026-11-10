import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { connectBoard, defaultBackoff, type ConnectionState } from './socket.ts'
import type { BoardEvent } from './types.ts'

class FakeSocket {
  static all: FakeSocket[] = []
  onopen: (() => void) | null = null
  onmessage: ((m: { data: string }) => void) | null = null
  onclose: (() => void) | null = null
  closed = false
  readonly url: string
  readonly protocols: string[]
  constructor(url: string, protocols: string[]) {
    this.url = url
    this.protocols = protocols
    FakeSocket.all.push(this)
  }
  close() {
    this.closed = true
    this.onclose?.()
  }
}

beforeEach(() => {
  FakeSocket.all = []
  vi.useFakeTimers()
})
afterEach(() => vi.useRealTimers())

function setup() {
  const events: BoardEvent[] = []
  const states: ConnectionState[] = []
  const stop = connectBoard({
    url: 'ws://127.0.0.1:7311/ws',
    token: 'tok',
    onEvent: (e) => events.push(e),
    onState: (s) => states.push(s),
    createSocket: (url, protocols) => new FakeSocket(url, protocols) as unknown as WebSocket,
    backoff: () => 100,
  })
  return { events, states, stop }
}

describe('connectBoard', () => {
  it('sends the token as a websocket subprotocol', () => {
    setup()
    expect(FakeSocket.all[0].protocols).toEqual(['pitwall', 'tok'])
  })

  it('parses events and ignores garbage', () => {
    const { events } = setup()
    const ws = FakeSocket.all[0]
    ws.onopen?.()
    ws.onmessage?.({ data: '{"type":"session.removed","session_id":"s1"}' })
    ws.onmessage?.({ data: 'not json' })
    expect(events).toEqual([{ type: 'session.removed', session_id: 's1' }])
  })

  it('reconnects after the connection drops', () => {
    const { states } = setup()
    FakeSocket.all[0].onopen?.()
    FakeSocket.all[0].onclose?.()
    expect(states).toEqual(['connecting', 'open', 'closed'])
    expect(FakeSocket.all).toHaveLength(1)
    vi.advanceTimersByTime(100)
    expect(FakeSocket.all).toHaveLength(2)
    FakeSocket.all[1].onopen?.()
    expect(states.at(-1)).toBe('open')
  })

  it('stops reconnecting once stopped', () => {
    const { stop } = setup()
    stop()
    expect(FakeSocket.all[0].closed).toBe(true)
    vi.advanceTimersByTime(10_000)
    expect(FakeSocket.all).toHaveLength(1)
  })

  it('backs off up to 10 seconds', () => {
    expect(defaultBackoff(0)).toBe(500)
    expect(defaultBackoff(1)).toBe(1000)
    expect(defaultBackoff(20)).toBe(10_000)
  })
})
