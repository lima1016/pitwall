import { describe, expect, it } from 'vitest'
import type { Session } from './api/types.ts'
import { translate } from './i18n/index.ts'
import { sortSessions, useBoard } from './store.ts'

const s = (id: string, updated_at: string) =>
  ({ id, updated_at, status: 'working' }) as unknown as Session

describe('board store', () => {
  it('upserts and removes sessions from events', () => {
    const { applyEvent } = useBoard.getState()
    applyEvent({ type: 'session.upserted', session: s('a', '1') })
    applyEvent({ type: 'session.upserted', session: { ...s('a', '2'), status: 'idle' } })
    expect(useBoard.getState().sessions.a.status).toBe('idle')
    applyEvent({ type: 'session.removed', session_id: 'a' })
    expect(useBoard.getState().sessions.a).toBeUndefined()
  })

  it('sorts by most recently updated', () => {
    const sorted = sortSessions([s('old', '2026-01-01'), s('new', '2026-02-01')])
    expect(sorted.map((x) => x.id)).toEqual(['new', 'old'])
  })
})

describe('i18n', () => {
  it('fills variables in both languages', () => {
    expect(translate('en', 'projects.removeConfirm', { name: 'app' })).toContain('"app"')
    expect(translate('ko', 'projects.removeConfirm', { name: 'app' })).toContain('"app"')
  })
})
