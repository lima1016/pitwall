import { create } from 'zustand'
import type { ConnectionState } from './api/socket.ts'
import type { BoardEvent, ProjectSummary, Session } from './api/types.ts'

/** 'all' | 'unclassified' | 프로젝트 id */
export type ProjectFilter = string

interface BoardState {
  sessions: Record<string, Session>
  projects: ProjectSummary[]
  connection: ConnectionState
  filter: ProjectFilter
  applyEvent: (event: BoardEvent) => void
  setSessions: (sessions: Session[]) => void
  setProjects: (projects: ProjectSummary[]) => void
  setConnection: (state: ConnectionState) => void
  setFilter: (filter: ProjectFilter) => void
}

export const useBoard = create<BoardState>((set) => ({
  sessions: {},
  projects: [],
  connection: 'connecting',
  filter: 'all',
  applyEvent: (event) =>
    set((s) => {
      if (event.type === 'session.upserted') {
        return { sessions: { ...s.sessions, [event.session.id]: event.session } }
      }
      if (event.type === 'session.removed') {
        const { [event.session_id]: _removed, ...rest } = s.sessions
        return { sessions: rest }
      }
      return s
    }),
  setSessions: (list) => set({ sessions: Object.fromEntries(list.map((x) => [x.id, x])) }),
  setProjects: (projects) => set({ projects }),
  setConnection: (connection) => set({ connection }),
  setFilter: (filter) => set({ filter }),
}))

/** 최근 갱신 순 */
export function sortSessions(list: Session[]): Session[] {
  return [...list].sort((a, b) => b.updated_at.localeCompare(a.updated_at))
}
