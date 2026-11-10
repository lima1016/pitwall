//! 보드: 세션 레지스트리 + 등록 프로젝트 + 이벤트 버스.
//! 상태 전환 자체는 `state` 모듈이 하고, 여기서는 세션을 찾고 만들고 알리기만 한다.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use chrono::{DateTime, Utc};
use serde::Serialize;

use super::{Session, SessionId, state};
use crate::events::{BoardEvent, EventBus};
use crate::git;
use crate::hooks::{HookKind, HookPayload};
use crate::project::{
    AddProjectError, Config, ConfigError, Project, ProjectId, ProjectRoots, match_project,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectSummary {
    #[serde(flatten)]
    pub project: Project,
    pub session_count: usize,
    pub waiting_count: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum BoardError {
    #[error(transparent)]
    Add(#[from] AddProjectError),
    #[error(transparent)]
    Config(#[from] ConfigError),
}

struct Inner {
    config: Config,
    roots: Vec<ProjectRoots>,
    sessions: HashMap<SessionId, Session>,
    /// 아직 UI로 보내지 않은 변경. `flush`가 모아서 보낸다 (PreToolUse 디바운스).
    dirty: HashSet<SessionId>,
}

pub struct Board {
    config_path: PathBuf,
    inner: Mutex<Inner>,
    bus: EventBus,
}

fn roots_for(projects: Vec<Project>) -> Vec<ProjectRoots> {
    projects
        .into_iter()
        .map(|project| {
            let mut roots = git::worktree_paths(&project.path);
            roots.push(project.path.clone());
            ProjectRoots { project, roots }
        })
        .collect()
}

impl Board {
    pub fn load(config_path: PathBuf) -> Result<Board, ConfigError> {
        let config = Config::load(&config_path)?;
        let roots = roots_for(config.projects());
        Ok(Board {
            config_path,
            inner: Mutex::new(Inner {
                config,
                roots,
                sessions: HashMap::new(),
                dirty: HashSet::new(),
            }),
            bus: EventBus::default(),
        })
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        // 다른 스레드가 패닉해도 데이터는 단순한 맵이라 계속 쓴다.
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn port(&self) -> u16 {
        self.lock().config.port
    }

    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<BoardEvent> {
        self.bus.subscribe()
    }

    /// hook 하나를 반영한다. 반영한 세션 id를 돌려준다. session_id가 없으면 무시.
    pub fn handle_hook(
        &self,
        kind: HookKind,
        payload: &HookPayload,
        now: DateTime<Utc>,
    ) -> Option<SessionId> {
        if payload.session_id.is_empty() {
            return None;
        }
        let id = payload.session_id.clone();
        let mut inner = self.lock();

        if !inner.sessions.contains_key(&id) {
            // 서버가 세션 도중에 켜진 경우에도 어떤 hook이든 세션을 만든다.
            let workspace = payload.cwd.clone().unwrap_or_else(|| PathBuf::from("."));
            let mut session = Session::new(id.clone(), workspace.clone(), now);
            session.branch = git::current_branch(&workspace);
            session.project_id = Self::match_or_refresh(&mut inner, &workspace);
            if kind != HookKind::SessionStart {
                // SessionStart를 못 본 세션은 일단 작업 중으로 본다. 아래 이벤트가 덮어쓴다.
                session.status = state::Status::Working;
            }
            inner.sessions.insert(id.clone(), session);
        }

        let session = inner.sessions.get_mut(&id)?;
        if let Some(event) = payload.to_event(kind) {
            state::apply(session, &event, now);
        } else {
            session.updated_at = now;
            session.stale = false;
        }
        inner.dirty.insert(id.clone());
        Some(id)
    }

    fn match_or_refresh(inner: &mut Inner, cwd: &Path) -> Option<ProjectId> {
        if let Some(p) = match_project(cwd, &inner.roots) {
            return Some(p.id.clone());
        }
        // 새로 만든 worktree일 수 있으니 한 번 다시 읽는다.
        if inner.roots.is_empty() {
            return None;
        }
        inner.roots = roots_for(inner.config.projects());
        match_project(cwd, &inner.roots).map(|p| p.id.clone())
    }

    /// 모아 둔 변경을 이벤트로 보낸다. 보낸 개수를 돌려준다.
    pub fn flush(&self) -> usize {
        let sessions: Vec<Session> = {
            let mut inner = self.lock();
            let dirty: Vec<SessionId> = inner.dirty.drain().collect();
            dirty
                .iter()
                .filter_map(|id| inner.sessions.get(id).cloned())
                .collect()
        };
        let n = sessions.len();
        for session in sessions {
            self.bus.publish(BoardEvent::SessionUpserted {
                session: Box::new(session),
            });
        }
        n
    }

    /// stale 플래그를 다시 계산하고 바뀐 세션을 변경 목록에 넣는다.
    pub fn tick_stale(&self, now: DateTime<Utc>) {
        let mut inner = self.lock();
        let Inner {
            sessions, dirty, ..
        } = &mut *inner;
        for (id, s) in sessions.iter_mut() {
            if state::refresh_stale(s, now) {
                dirty.insert(id.clone());
            }
        }
    }

    /// 세션 목록. 최근 갱신 순. `project`가 있으면 그 프로젝트만.
    pub fn sessions(&self, project: Option<&str>) -> Vec<Session> {
        let inner = self.lock();
        let mut list: Vec<Session> = inner
            .sessions
            .values()
            .filter(|s| project.is_none_or(|p| s.project_id.as_deref() == Some(p)))
            .cloned()
            .collect();
        list.sort_by(|a, b| b.updated_at.cmp(&a.updated_at).then(a.id.cmp(&b.id)));
        list
    }

    pub fn projects(&self) -> Vec<ProjectSummary> {
        let inner = self.lock();
        inner
            .config
            .projects()
            .into_iter()
            .map(|project| {
                let mine = || {
                    inner
                        .sessions
                        .values()
                        .filter(|s| s.project_id.as_deref() == Some(project.id.as_str()))
                };
                ProjectSummary {
                    session_count: mine().filter(|s| !s.status.is_terminal()).count(),
                    waiting_count: mine().filter(|s| s.status.is_waiting()).count(),
                    project,
                }
            })
            .collect()
    }

    pub fn add_project(&self, path: &Path) -> Result<Project, BoardError> {
        let mut inner = self.lock();
        let mut config = inner.config.clone();
        let project = config.add_project(path)?;
        config.save(&self.config_path)?;
        inner.config = config;
        inner.roots = roots_for(inner.config.projects());
        // 미분류 세션 중 새 프로젝트에 속하는 것을 옮긴다.
        let Inner {
            sessions,
            roots,
            dirty,
            ..
        } = &mut *inner;
        for (id, s) in sessions.iter_mut().filter(|(_, s)| s.project_id.is_none()) {
            if let Some(p) = match_project(&s.workspace, roots) {
                s.project_id = Some(p.id.clone());
                dirty.insert(id.clone());
            }
        }
        Ok(project)
    }

    /// 등록 해제. 파일은 건드리지 않는다. 있었으면 true.
    pub fn remove_project(&self, id: &str) -> Result<bool, BoardError> {
        let mut inner = self.lock();
        let mut config = inner.config.clone();
        if !config.remove_project(id) {
            return Ok(false);
        }
        config.save(&self.config_path)?;
        inner.config = config;
        inner.roots.retain(|r| r.project.id != id);
        let Inner {
            sessions, dirty, ..
        } = &mut *inner;
        for (sid, s) in sessions.iter_mut() {
            if s.project_id.as_deref() == Some(id) {
                s.project_id = None;
                dirty.insert(sid.clone());
            }
        }
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{PendingRequest, Status};

    fn board(dir: &Path) -> Board {
        Board::load(dir.join("config.toml")).unwrap()
    }

    fn payload(session: &str, cwd: &Path) -> HookPayload {
        HookPayload {
            session_id: session.into(),
            cwd: Some(cwd.to_owned()),
            ..Default::default()
        }
    }

    #[test]
    fn hook_creates_session_matched_to_project() {
        let dir = tempfile::tempdir().unwrap();
        let proj = dir.path().join("app");
        std::fs::create_dir_all(proj.join("src")).unwrap();
        let b = board(dir.path());
        let p = b.add_project(&proj).unwrap();

        let now = Utc::now();
        b.handle_hook(
            HookKind::SessionStart,
            &payload("s1", &proj.join("src")),
            now,
        );
        let s = &b.sessions(None)[0];
        assert_eq!(s.project_id.as_deref(), Some(p.id.as_str()));
        assert_eq!(s.status, Status::Starting);
        assert_eq!(b.sessions(Some(&p.id)).len(), 1);
        assert_eq!(b.sessions(Some("other")).len(), 0);
    }

    #[test]
    fn unknown_cwd_is_unclassified_until_project_added() {
        let dir = tempfile::tempdir().unwrap();
        let proj = dir.path().join("app");
        std::fs::create_dir_all(&proj).unwrap();
        let b = board(dir.path());
        b.handle_hook(HookKind::SessionStart, &payload("s1", &proj), Utc::now());
        assert_eq!(b.sessions(None)[0].project_id, None);

        let p = b.add_project(&proj).unwrap();
        assert_eq!(
            b.sessions(None)[0].project_id.as_deref(),
            Some(p.id.as_str())
        );

        assert!(b.remove_project(&p.id).unwrap());
        assert_eq!(b.sessions(None)[0].project_id, None);
        assert!(!b.remove_project(&p.id).unwrap());
    }

    #[test]
    fn mid_session_hook_without_session_start_still_registers() {
        let dir = tempfile::tempdir().unwrap();
        let b = board(dir.path());
        let mut p = payload("s1", dir.path());
        p.tool_name = Some("Bash".into());
        p.tool_input = Some(serde_json::json!({"command": "ls"}));
        p.notification_type = None;
        b.handle_hook(HookKind::PermissionRequest, &p, Utc::now());
        let s = &b.sessions(None)[0];
        assert_eq!(s.status, Status::NeedsPermission);
        assert!(matches!(s.pending, Some(PendingRequest::Permission { .. })));
    }

    #[test]
    fn empty_session_id_is_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let b = board(dir.path());
        let p = HookPayload::default();
        assert_eq!(b.handle_hook(HookKind::Stop, &p, Utc::now()), None);
        assert!(b.sessions(None).is_empty());
    }

    #[test]
    fn flush_coalesces_and_publishes_upserts() {
        let dir = tempfile::tempdir().unwrap();
        let b = board(dir.path());
        let mut rx = b.subscribe();
        let now = Utc::now();
        for _ in 0..5 {
            b.handle_hook(HookKind::PreToolUse, &payload("s1", dir.path()), now);
        }
        assert_eq!(b.flush(), 1, "같은 세션의 연속 변경은 한 번만 보낸다");
        assert_eq!(b.flush(), 0);
        match rx.try_recv().unwrap() {
            BoardEvent::SessionUpserted { session } => assert_eq!(session.id, "s1"),
            e => panic!("unexpected {e:?}"),
        }
    }

    #[test]
    fn project_summary_counts_live_and_waiting() {
        let dir = tempfile::tempdir().unwrap();
        let proj = dir.path().join("app");
        std::fs::create_dir_all(&proj).unwrap();
        let b = board(dir.path());
        b.add_project(&proj).unwrap();
        let now = Utc::now();
        b.handle_hook(HookKind::PermissionRequest, &payload("a", &proj), now);
        b.handle_hook(HookKind::UserPromptSubmit, &payload("b", &proj), now);
        b.handle_hook(HookKind::SessionEnd, &payload("c", &proj), now);
        let s = &b.projects()[0];
        assert_eq!(s.session_count, 2);
        assert_eq!(s.waiting_count, 1);
    }

    #[test]
    fn tick_marks_stale_sessions_dirty() {
        let dir = tempfile::tempdir().unwrap();
        let b = board(dir.path());
        let now = Utc::now();
        b.handle_hook(HookKind::SessionStart, &payload("s1", dir.path()), now);
        b.flush();
        b.tick_stale(now + chrono::Duration::minutes(31));
        assert_eq!(b.flush(), 1);
        assert!(b.sessions(None)[0].stale);
    }

    #[test]
    fn projects_persist_in_config() {
        let dir = tempfile::tempdir().unwrap();
        let proj = dir.path().join("app");
        std::fs::create_dir_all(&proj).unwrap();
        board(dir.path()).add_project(&proj).unwrap();
        assert_eq!(board(dir.path()).projects().len(), 1);
    }
}
