import type { ApiError, Project, ProjectSummary, Session } from './types.ts'

// 토큰은 시작 URL의 `#token=...`으로 받는다. 메모리에만 두고 주소창에서는 지운다 (DESIGN.md 7장).
let token = ''

export function takeTokenFromLocation(): string {
  const match = /(?:^#|&)token=([0-9a-fA-F]+)/.exec(window.location.hash)
  if (match) {
    token = match[1]
    history.replaceState(null, '', window.location.pathname + window.location.search)
  }
  return token
}

export function getToken(): string {
  return token
}

export function setToken(value: string): void {
  token = value
}

export class HttpError extends Error {
  readonly status: number
  constructor(status: number, message: string) {
    super(message)
    this.status = status
  }
}

async function request<T>(method: string, path: string, body?: unknown): Promise<T> {
  const res = await fetch(path, {
    method,
    headers: {
      Authorization: `Bearer ${token}`,
      ...(body === undefined ? {} : { 'Content-Type': 'application/json' }),
    },
    body: body === undefined ? undefined : JSON.stringify(body),
  })
  if (!res.ok) {
    let message = res.statusText
    try {
      message = ((await res.json()) as ApiError).error ?? message
    } catch {
      // 본문이 JSON이 아니면 상태 문구를 쓴다
    }
    throw new HttpError(res.status, message)
  }
  return (res.status === 204 ? undefined : await res.json()) as T
}

export const api = {
  projects: () => request<ProjectSummary[]>('GET', '/api/projects'),
  addProject: (path: string) => request<Project>('POST', '/api/projects', { path }),
  removeProject: (id: string) => request<void>('DELETE', `/api/projects/${encodeURIComponent(id)}`),
  sessions: () => request<Session[]>('GET', '/api/sessions'),
}
