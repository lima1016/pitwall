// 서버 API 타입은 이 파일 한 곳에 모은다 (CLAUDE.md 구조 규칙).
// Rust 쪽 정의: crates/pitwall-core/src/session/*.rs, project.rs, events.rs

export type Status =
  | 'starting'
  | 'working'
  | 'needs_permission'
  | 'needs_input'
  | 'idle'
  | 'ended'
  | 'failed'
  | 'stopped'

export const WAITING_STATUSES: readonly Status[] = ['needs_permission', 'needs_input']
export const TERMINAL_STATUSES: readonly Status[] = ['ended', 'failed', 'stopped']

export type Origin = { kind: 'observed' }

export type PendingRequest =
  | { kind: 'permission'; tool_name: string; summary: string }
  | { kind: 'input'; message: string }

export interface Session {
  id: string
  /** null이면 미분류 */
  project_id: string | null
  workspace: string
  branch: string | null
  origin: Origin
  status: Status
  current_activity: string | null
  last_prompt: string | null
  pending: PendingRequest | null
  stale: boolean
  started_at: string
  updated_at: string
}

export interface Project {
  id: string
  name: string
  path: string
}

export interface ProjectSummary extends Project {
  session_count: number
  waiting_count: number
}

/** WebSocket `/ws` 서버 → 클라이언트 메시지 */
export type BoardEvent =
  | { type: 'session.upserted'; session: Session }
  | { type: 'session.removed'; session_id: string }

export interface ApiError {
  error: string
}
