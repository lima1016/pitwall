import type { Session } from '../api/types.ts'
import { useT } from '../i18n/index.ts'
import { sessionName } from '../session.ts'
import { StatusBadge } from './StatusBadge.tsx'

interface Props {
  session: Session
  projectName: string | null
  onRegister?: (path: string) => void
}

export function SessionCard({ session: s, projectName, onRegister }: Props) {
  const t = useT()
  const waiting = s.status === 'needs_permission' || s.status === 'needs_input'
  return (
    <article className={`card${waiting ? ' card-waiting' : ''}`} data-session={s.id}>
      <header className="card-head">
        <h3 className="card-title" title={s.workspace}>
          {sessionName(s)}
        </h3>
        <StatusBadge status={s.status} />
      </header>
      <p className="card-meta">
        <span>{projectName ?? t('projects.unclassified')}</span>
        <span aria-hidden="true"> · </span>
        <span>{s.branch ?? t('card.noBranch')}</span>
        {s.stale && <span className="card-stale"> · {t('card.stale')}</span>}
      </p>

      {s.pending?.kind === 'permission' && (
        <div className="card-pending">
          <div className="card-label">{t('card.permission')}</div>
          <code className="card-code">{s.pending.summary || s.pending.tool_name}</code>
          <p className="card-hint">
            {t('card.answerInIde')} {t('card.approveLater')}
          </p>
        </div>
      )}
      {s.pending?.kind === 'input' && (
        <div className="card-pending">
          <p className="card-message">{s.pending.message}</p>
          <p className="card-hint">{t('card.answerInIde')}</p>
        </div>
      )}

      {!waiting && s.current_activity && (
        <p className="card-line">
          <span className="card-label">{t('card.activity')}</span> <code>{s.current_activity}</code>
        </p>
      )}
      {s.last_prompt && (
        <p className="card-line card-prompt" title={s.last_prompt}>
          <span className="card-label">{t('card.lastPrompt')}</span> {s.last_prompt}
        </p>
      )}
      {projectName === null && onRegister && (
        <button type="button" className="link" onClick={() => onRegister(s.workspace)}>
          {t('projects.registerHint')}
        </button>
      )}
    </article>
  )
}
