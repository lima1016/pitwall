import { useCallback, useEffect, useMemo, useState } from 'react'
import { api, getToken, HttpError, takeTokenFromLocation } from './api/client.ts'
import { boardSocketUrl, connectBoard } from './api/socket.ts'
import type { ProjectSummary } from './api/types.ts'
import { SessionCard } from './components/SessionCard.tsx'
import { Sidebar } from './components/Sidebar.tsx'
import { TopBar } from './components/TopBar.tsx'
import { useLang, useT } from './i18n/index.ts'
import { sortSessions, useBoard } from './store.ts'

export default function App() {
  const t = useT()
  const lang = useLang((s) => s.lang)
  const [token] = useState(() => takeTokenFromLocation() || getToken())
  const [error, setError] = useState<string | null>(null)
  const { sessions, projects, connection, filter } = useBoard()
  const { applyEvent, setSessions, setProjects, setConnection, setFilter } = useBoard()

  const refreshProjects = useCallback(async () => {
    try {
      setProjects(await api.projects())
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    }
  }, [setProjects])

  useEffect(() => {
    document.documentElement.lang = lang
  }, [lang])

  useEffect(() => {
    if (!token) return
    api.projects().then(setProjects, (e: unknown) => setError(String(e)))
    api.sessions().then(setSessions, () => {})
    return connectBoard({
      url: boardSocketUrl(),
      token,
      onEvent: applyEvent,
      onState: setConnection,
    })
  }, [token, setProjects, setSessions, applyEvent, setConnection])

  const list = useMemo(() => sortSessions(Object.values(sessions)), [sessions])
  const visible = list.filter((s) =>
    filter === 'all' ? true : filter === 'unclassified' ? s.project_id === null : s.project_id === filter,
  )
  const projectName = (id: string | null) => projects.find((p) => p.id === id)?.name ?? null

  const addProject = async (path: string) => {
    setError(null)
    try {
      await api.addProject(path)
      await refreshProjects()
    } catch (e) {
      setError(e instanceof HttpError ? e.message : String(e))
    }
  }

  const removeProject = async (p: ProjectSummary) => {
    if (!window.confirm(t('projects.removeConfirm', { name: p.name }))) return
    try {
      await api.removeProject(p.id)
      if (filter === p.id) setFilter('all')
      await refreshProjects()
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    }
  }

  if (!token) {
    return (
      <div className="app">
        <TopBar sessions={[]} connection="closed" />
        <main className="notice">
          <p>{t('auth.missing')}</p>
        </main>
      </div>
    )
  }

  return (
    <div className="app">
      <TopBar sessions={list} connection={connection} />
      <div className="layout">
        <Sidebar
          projects={projects}
          sessions={list}
          filter={filter}
          onFilter={setFilter}
          onAdd={addProject}
          onRemove={removeProject}
          error={error}
        />
        <main className="board">
          {visible.length === 0 ? (
            <p className="muted empty">{t('board.empty')}</p>
          ) : (
            <div className="grid">
              {visible.map((s) => (
                <div key={s.id} id={`session-${s.id}`}>
                  <SessionCard
                    session={s}
                    projectName={projectName(s.project_id)}
                    onRegister={addProject}
                  />
                </div>
              ))}
            </div>
          )}
        </main>
      </div>
    </div>
  )
}
