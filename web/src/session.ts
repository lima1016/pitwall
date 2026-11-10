import type { Session } from './api/types.ts'

/** 카드 제목: 작업 폴더 이름 + 세션 id 앞 8자 */
export function sessionName(s: Session): string {
  const parts = s.workspace.split(/[\\/]/).filter(Boolean)
  return `${parts.at(-1) ?? s.workspace} · ${s.id.slice(0, 8)}`
}
