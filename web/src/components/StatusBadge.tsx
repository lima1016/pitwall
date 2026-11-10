import type { Status } from '../api/types.ts'
import { useT } from '../i18n/index.ts'

// 상태는 색만으로 구분하지 않는다: 아이콘 + 라벨 텍스트 (DESIGN.md 8장).
const ICONS: Record<Status, string> = {
  starting: '◌',
  working: '▶',
  needs_permission: '!',
  needs_input: '?',
  idle: '■',
  ended: '✓',
  failed: '✕',
  stopped: '⏹',
}

export function StatusBadge({ status }: { status: Status }) {
  const t = useT()
  return (
    <span className={`badge badge-${status}`} data-status={status}>
      <span className="badge-icon" aria-hidden="true">
        {ICONS[status]}
      </span>
      {t(`status.${status}`)}
    </span>
  )
}
