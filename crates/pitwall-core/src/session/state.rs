//! 세션 상태 머신. 상태 전환은 이 파일에서만 일어난다 (DESIGN.md 2.1).

use chrono::{DateTime, Duration, Utc};
use serde::Serialize;

use super::{Origin, PendingRequest, Session, SessionEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Starting,
    Working,
    NeedsPermission,
    NeedsInput,
    Idle,
    Ended,
    Failed,
    Stopped,
}

impl Status {
    pub fn is_terminal(self) -> bool {
        matches!(self, Status::Ended | Status::Failed | Status::Stopped)
    }

    pub fn is_waiting(self) -> bool {
        matches!(self, Status::NeedsPermission | Status::NeedsInput)
    }
}

/// hook이 이 시간 동안 오지 않으면 stale.
pub const STALE_AFTER: Duration = Duration::minutes(30);

/// 다음 상태. 순수 함수라 전환표를 그대로 테스트한다.
pub fn transition(current: Status, event: &SessionEvent) -> Status {
    use SessionEvent as E;
    if current.is_terminal() {
        // 끝난 세션은 같은 id로 다시 시작(resume)할 때만 살아난다.
        return match event {
            E::Started => Status::Starting,
            _ => current,
        };
    }
    match event {
        E::Started => Status::Starting,
        E::PromptSubmitted { .. } | E::ToolStarting { .. } | E::ToolFinished => Status::Working,
        E::PermissionRequested { .. } | E::PermissionPrompt { .. } => Status::NeedsPermission,
        E::InputRequested { .. } => Status::NeedsInput,
        E::Stopped => Status::Idle,
        E::Ended => Status::Ended,
    }
}

/// 이벤트를 세션에 반영한다. 상태, 대기 요청, 현재 활동을 함께 갱신한다.
pub fn apply(session: &mut Session, event: &SessionEvent, now: DateTime<Utc>) {
    use SessionEvent as E;
    let next = transition(session.status, event);
    let changed = next != session.status || !session.status.is_terminal();
    session.status = next;
    session.updated_at = now;
    session.stale = false;
    if !changed {
        return;
    }

    match event {
        E::Started => {
            session.pending = None;
            session.current_activity = None;
        }
        E::PromptSubmitted { prompt } => {
            session.last_prompt = Some(prompt.clone());
            session.pending = None;
            session.current_activity = None;
        }
        E::ToolStarting { activity } => {
            session.current_activity = Some(activity.clone());
            session.pending = None;
        }
        // 터미널에서 먼저 답한 경우를 포함해, 도구가 끝났으면 대기 요청은 끝난 것이다.
        E::ToolFinished => session.pending = None,
        E::PermissionRequested { tool_name, summary } => {
            session.pending = Some(PendingRequest::Permission {
                tool_name: tool_name.clone(),
                summary: summary.clone(),
            });
        }
        E::PermissionPrompt { message } => {
            // PermissionRequest가 먼저 왔으면 그 상세 내용을 유지한다.
            if !matches!(session.pending, Some(PendingRequest::Permission { .. })) {
                session.pending = Some(PendingRequest::Permission {
                    tool_name: String::new(),
                    summary: message.clone(),
                });
            }
        }
        E::InputRequested { message } => {
            session.pending = Some(PendingRequest::Input {
                message: message.clone(),
            });
        }
        E::Stopped | E::Ended => {
            session.pending = None;
            session.current_activity = None;
        }
    }
}

/// stale 플래그를 다시 계산한다. 바뀌었으면 true.
pub fn refresh_stale(session: &mut Session, now: DateTime<Utc>) -> bool {
    let stale = session.origin == Origin::Observed
        && !session.status.is_terminal()
        && now - session.updated_at > STALE_AFTER;
    let changed = stale != session.stale;
    session.stale = stale;
    changed
}

#[cfg(test)]
mod tests {
    use super::Status::*;
    use super::*;
    use std::path::PathBuf;

    fn ev_prompt() -> SessionEvent {
        SessionEvent::PromptSubmitted {
            prompt: "hi".into(),
        }
    }
    fn ev_tool() -> SessionEvent {
        SessionEvent::ToolStarting {
            activity: "Bash: ls".into(),
        }
    }
    fn ev_perm() -> SessionEvent {
        SessionEvent::PermissionRequested {
            tool_name: "Bash".into(),
            summary: "Bash: rm x".into(),
        }
    }
    fn ev_perm_prompt() -> SessionEvent {
        SessionEvent::PermissionPrompt {
            message: "needs permission".into(),
        }
    }
    fn ev_input() -> SessionEvent {
        SessionEvent::InputRequested {
            message: "Which one?".into(),
        }
    }

    /// 전환표. (현재 상태, 이벤트, 기대 상태)
    #[test]
    fn transition_table() {
        let live = [Starting, Working, NeedsPermission, NeedsInput, Idle];
        for s in live {
            assert_eq!(transition(s, &SessionEvent::Started), Starting, "{s:?}");
            assert_eq!(transition(s, &ev_prompt()), Working, "{s:?}");
            assert_eq!(transition(s, &ev_tool()), Working, "{s:?}");
            assert_eq!(transition(s, &SessionEvent::ToolFinished), Working, "{s:?}");
            assert_eq!(transition(s, &ev_perm()), NeedsPermission, "{s:?}");
            assert_eq!(transition(s, &ev_perm_prompt()), NeedsPermission, "{s:?}");
            assert_eq!(transition(s, &ev_input()), NeedsInput, "{s:?}");
            assert_eq!(transition(s, &SessionEvent::Stopped), Idle, "{s:?}");
            assert_eq!(transition(s, &SessionEvent::Ended), Ended, "{s:?}");
        }
    }

    #[test]
    fn terminal_states_only_restart_on_session_start() {
        for s in [Ended, Failed, Stopped] {
            assert_eq!(transition(s, &SessionEvent::Started), Starting, "{s:?}");
            for e in [
                ev_prompt(),
                ev_tool(),
                ev_perm(),
                ev_input(),
                SessionEvent::Stopped,
            ] {
                assert_eq!(transition(s, &e), s, "{s:?} {e:?}");
            }
        }
    }

    fn session() -> Session {
        Session::new("s1".into(), PathBuf::from("/w"), Utc::now())
    }

    #[test]
    fn apply_tracks_prompt_activity_and_pending() {
        let now = Utc::now();
        let mut s = session();
        apply(&mut s, &ev_prompt(), now);
        assert_eq!(s.status, Working);
        assert_eq!(s.last_prompt.as_deref(), Some("hi"));

        apply(&mut s, &ev_tool(), now);
        assert_eq!(s.current_activity.as_deref(), Some("Bash: ls"));

        apply(&mut s, &ev_perm(), now);
        assert_eq!(s.status, NeedsPermission);
        assert_eq!(
            s.pending,
            Some(PendingRequest::Permission {
                tool_name: "Bash".into(),
                summary: "Bash: rm x".into()
            })
        );

        // 터미널에서 먼저 답한 경우: PostToolUse가 오면 대기 요청을 정리한다.
        apply(&mut s, &SessionEvent::ToolFinished, now);
        assert_eq!(s.status, Working);
        assert_eq!(s.pending, None);

        apply(&mut s, &SessionEvent::Stopped, now);
        assert_eq!(s.status, Idle);
        assert_eq!(s.current_activity, None);
    }

    #[test]
    fn permission_prompt_keeps_existing_permission_details() {
        let now = Utc::now();
        let mut s = session();
        apply(&mut s, &ev_perm(), now);
        apply(&mut s, &ev_perm_prompt(), now);
        assert!(matches!(
            s.pending,
            Some(PendingRequest::Permission { ref summary, .. }) if summary == "Bash: rm x"
        ));
    }

    #[test]
    fn permission_prompt_without_request_uses_message() {
        let now = Utc::now();
        let mut s = session();
        apply(&mut s, &ev_perm_prompt(), now);
        assert!(matches!(
            s.pending,
            Some(PendingRequest::Permission { ref summary, .. }) if summary == "needs permission"
        ));
    }

    #[test]
    fn input_request_sets_pending_message_and_prompt_clears_it() {
        let now = Utc::now();
        let mut s = session();
        apply(&mut s, &ev_input(), now);
        assert_eq!(s.status, NeedsInput);
        assert_eq!(
            s.pending,
            Some(PendingRequest::Input {
                message: "Which one?".into()
            })
        );
        apply(&mut s, &ev_prompt(), now);
        assert_eq!(s.pending, None);
    }

    #[test]
    fn apply_updates_timestamp_and_clears_stale() {
        let start = Utc::now();
        let mut s = session();
        s.stale = true;
        let later = start + Duration::minutes(1);
        apply(&mut s, &ev_prompt(), later);
        assert_eq!(s.updated_at, later);
        assert!(!s.stale);
    }

    #[test]
    fn stale_after_thirty_minutes_without_hooks() {
        let mut s = session();
        let t0 = s.updated_at;
        assert!(!refresh_stale(&mut s, t0 + Duration::minutes(29)));
        assert!(!s.stale);
        assert!(refresh_stale(&mut s, t0 + Duration::minutes(31)));
        assert!(s.stale);
        assert_eq!(s.status, Starting, "stale은 상태를 바꾸지 않는다");
        assert!(
            !refresh_stale(&mut s, t0 + Duration::minutes(40)),
            "이미 stale"
        );
    }

    #[test]
    fn ended_sessions_never_go_stale() {
        let mut s = session();
        let t0 = s.updated_at;
        apply(&mut s, &SessionEvent::Ended, t0);
        assert!(!refresh_stale(&mut s, t0 + Duration::hours(5)));
        assert_eq!(s.origin, Origin::Observed);
    }
}
