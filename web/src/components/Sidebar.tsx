import { useState, type FormEvent } from 'react'
import type { ProjectSummary, Session } from '../api/types.ts'
import { useT } from '../i18n/index.ts'
import type { ProjectFilter } from '../store.ts'
import { sessionName } from '../session.ts'
import { StatusBadge } from './StatusBadge.tsx'

interface Props {
  projects: ProjectSummary[]
  sessions: Session[]
  filter: ProjectFilter
  onFilter: (f: ProjectFilter) => void
  onAdd: (path: string) => Promise<void>
  onRemove: (p: ProjectSummary) => void
  error: string | null
}

const isLive = (s: Session) => !['ended', 'failed', 'stopped'].includes(s.status)
const isWaiting = (s: Session) => s.status === 'needs_permission' || s.status === 'needs_input'

export function Sidebar({ projects, sessions, filter, onFilter, onAdd, onRemove, error }: Props) {
  const t = useT()
  const [path, setPath] = useState('')
  const waiting = sessions.filter(isWaiting)
  const unclassified = sessions.filter((s) => s.project_id === null && isLive(s))

  const submit = async (e: FormEvent) => {
    e.preventDefault()
    if (!path.trim()) return
    await onAdd(path.trim())
    setPath('')
  }

  const item = (id: ProjectFilter, label: string, list: Session[], extra?: React.ReactNode) => (
    <li key={id}>
      <button
        type="button"
        className={`nav-item${filter === id ? ' active' : ''}`}
        aria-current={filter === id}
        onClick={() => onFilter(id)}
      >
        <span className="nav-name">{label}</span>
        <span className="nav-count">{list.filter(isLive).length}</span>
        {list.some(isWaiting) && (
          <span className="nav-waiting">{list.filter(isWaiting).length}</span>
        )}
      </button>
      {extra}
    </li>
  )

  return (
    <aside className="sidebar">
      <section>
        <h2>{t('projects.title')}</h2>
        <ul className="nav">
          {item('all', t('projects.all'), sessions)}
          {projects.map((p) =>
            item(
              p.id,
              p.name,
              sessions.filter((s) => s.project_id === p.id),
              <button
                type="button"
                className="nav-remove"
                title={t('projects.remove')}
                aria-label={`${t('projects.remove')}: ${p.name}`}
                onClick={() => onRemove(p)}
              >
                ×
              </button>,
            ),
          )}
          {unclassified.length > 0 && item('unclassified', t('projects.unclassified'), unclassified)}
        </ul>
        <form className="add-form" onSubmit={submit}>
          <label htmlFor="add-path">{t('projects.add')}</label>
          <div className="add-row">
            <input
              id="add-path"
              value={path}
              placeholder={t('projects.addPlaceholder')}
              onChange={(e) => setPath(e.target.value)}
            />
            <button type="submit">{t('projects.addSubmit')}</button>
          </div>
          {error && <p className="error">{error}</p>}
        </form>
      </section>

      <section>
        <h2>{t('waiting.title')}</h2>
        {waiting.length === 0 ? (
          <p className="muted">{t('waiting.empty')}</p>
        ) : (
          <ul className="waiting">
            {waiting.map((s) => (
              <li key={s.id}>
                <a href={`#session-${s.id}`}>{sessionName(s)}</a>
                <StatusBadge status={s.status} />
              </li>
            ))}
          </ul>
        )}
      </section>
    </aside>
  )
}
