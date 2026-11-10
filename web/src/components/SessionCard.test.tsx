import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { Session, Status } from '../api/types.ts'
import { useLang } from '../i18n/index.ts'
import { SessionCard } from './SessionCard.tsx'

afterEach(cleanup)
beforeEach(() => useLang.getState().setLang('en'))

function session(over: Partial<Session> = {}): Session {
  return {
    id: 'abcdef1234567890',
    project_id: 'p1',
    workspace: 'C:/work/app',
    branch: 'main',
    origin: { kind: 'observed' },
    status: 'working',
    current_activity: 'Bash: npm test',
    last_prompt: 'fix the tests',
    pending: null,
    stale: false,
    started_at: '2026-11-10T00:00:00Z',
    updated_at: '2026-11-10T00:00:00Z',
    ...over,
  }
}

const LABELS: Record<Status, string> = {
  starting: 'Starting',
  working: 'Working',
  needs_permission: 'Needs permission',
  needs_input: 'Needs input',
  idle: 'Idle',
  ended: 'Ended',
  failed: 'Failed',
  stopped: 'Stopped',
}

describe('SessionCard', () => {
  it('shows a text label for every status, not only a color', () => {
    for (const [status, label] of Object.entries(LABELS)) {
      render(<SessionCard session={session({ status: status as Status })} projectName="app" />)
      expect(screen.getByText(label), status).toBeTruthy()
      cleanup()
    }
  })

  it('shows name, project, branch, activity and last prompt', () => {
    render(<SessionCard session={session()} projectName="app" />)
    expect(screen.getByText('app · abcdef12')).toBeTruthy()
    expect(screen.getByText('main')).toBeTruthy()
    expect(screen.getByText('Bash: npm test')).toBeTruthy()
    expect(screen.getByText(/fix the tests/)).toBeTruthy()
  })

  it('shows the permission summary and points to the IDE terminal', () => {
    const s = session({
      status: 'needs_permission',
      pending: { kind: 'permission', tool_name: 'Bash', summary: 'Bash: rm -rf node_modules' },
    })
    render(<SessionCard session={s} projectName="app" />)
    expect(screen.getByText('Bash: rm -rf node_modules')).toBeTruthy()
    expect(screen.getByText(/Answer in your IDE terminal/)).toBeTruthy()
    expect(screen.queryByRole('button', { name: /allow/i })).toBeNull()
  })

  it('shows the question for needs_input', () => {
    const s = session({ status: 'needs_input', pending: { kind: 'input', message: 'Which DB?' } })
    render(<SessionCard session={s} projectName="app" />)
    expect(screen.getByText('Which DB?')).toBeTruthy()
  })

  it('offers to register the folder for unclassified sessions', () => {
    const onRegister = vi.fn()
    render(<SessionCard session={session({ project_id: null })} projectName={null} onRegister={onRegister} />)
    expect(screen.getByText('Unclassified')).toBeTruthy()
    fireEvent.click(screen.getByRole('button', { name: 'Register this folder as a project' }))
    expect(onRegister).toHaveBeenCalledWith('C:/work/app')
  })

  it('marks stale sessions', () => {
    render(<SessionCard session={session({ stale: true })} projectName="app" />)
    expect(screen.getByText(/No updates for 30\+ min/)).toBeTruthy()
  })

  it('renders in Korean', () => {
    useLang.getState().setLang('ko')
    render(<SessionCard session={session({ status: 'needs_input', pending: { kind: 'input', message: 'q' } })} projectName="app" />)
    expect(screen.getByText('입력 대기')).toBeTruthy()
    expect(screen.getByText('IDE 터미널에서 답하세요.')).toBeTruthy()
  })
})
