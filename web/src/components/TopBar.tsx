import type { ConnectionState } from '../api/socket.ts'
import type { Session } from '../api/types.ts'
import { LANGS, useLang, useT } from '../i18n/index.ts'

export function TopBar({ sessions, connection }: { sessions: Session[]; connection: ConnectionState }) {
  const t = useT()
  const { lang, setLang } = useLang()
  const count = (pred: (s: Session) => boolean) => sessions.filter(pred).length
  const waiting = count((s) => s.status === 'needs_permission' || s.status === 'needs_input')
  const working = count((s) => s.status === 'working' || s.status === 'starting')
  const done = count((s) => s.status === 'idle' || s.status === 'ended')

  return (
    <header className="topbar">
      <div className="brand">
        <h1>pitwall</h1>
        <span className="muted">{t('app.tagline')}</span>
      </div>
      <dl className="counters">
        <div className={waiting > 0 ? 'counter hot' : 'counter'}>
          <dt>{t('top.waiting')}</dt>
          <dd data-testid="count-waiting">{waiting}</dd>
        </div>
        <div className="counter">
          <dt>{t('top.working')}</dt>
          <dd data-testid="count-working">{working}</dd>
        </div>
        <div className="counter">
          <dt>{t('top.done')}</dt>
          <dd data-testid="count-done">{done}</dd>
        </div>
      </dl>
      <div className="topbar-right">
        <span className={`conn conn-${connection}`} role="status">
          {t(`conn.${connection}`)}
        </span>
        <div className="lang" role="group" aria-label={t('top.language')}>
          {LANGS.map((l) => (
            <button
              key={l}
              type="button"
              aria-pressed={lang === l}
              className={lang === l ? 'active' : ''}
              onClick={() => setLang(l)}
            >
              {l === 'ko' ? '한국어' : 'English'}
            </button>
          ))}
        </div>
      </div>
    </header>
  )
}
