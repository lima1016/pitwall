//! Claude Code hook 이벤트 파싱. 필드는 관대하게 받는다: 모르는 필드는 무시하고, 없는 필드는 기본값.
//! 공식 문서: https://code.claude.com/docs/en/hooks (2026-11-10 확인)

use std::path::PathBuf;
use std::str::FromStr;

use serde::Deserialize;

use crate::session::SessionEvent;

/// pitwall이 등록하는 hook 이벤트.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookKind {
    SessionStart,
    SessionEnd,
    UserPromptSubmit,
    PreToolUse,
    PostToolUse,
    Notification,
    Stop,
    PermissionRequest,
}

impl HookKind {
    pub const ALL: [HookKind; 8] = [
        HookKind::SessionStart,
        HookKind::SessionEnd,
        HookKind::UserPromptSubmit,
        HookKind::PreToolUse,
        HookKind::PostToolUse,
        HookKind::Notification,
        HookKind::Stop,
        HookKind::PermissionRequest,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            HookKind::SessionStart => "SessionStart",
            HookKind::SessionEnd => "SessionEnd",
            HookKind::UserPromptSubmit => "UserPromptSubmit",
            HookKind::PreToolUse => "PreToolUse",
            HookKind::PostToolUse => "PostToolUse",
            HookKind::Notification => "Notification",
            HookKind::Stop => "Stop",
            HookKind::PermissionRequest => "PermissionRequest",
        }
    }
}

impl FromStr for HookKind {
    type Err = UnknownHook;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        HookKind::ALL
            .into_iter()
            .find(|k| k.as_str() == s)
            .ok_or_else(|| UnknownHook(s.to_owned()))
    }
}

#[derive(Debug, thiserror::Error)]
#[error("unknown hook event: {0}")]
pub struct UnknownHook(pub String);

/// hook 입력 JSON. 이벤트마다 쓰는 필드가 다르므로 전부 선택 필드로 둔다.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct HookPayload {
    pub session_id: String,
    pub cwd: Option<PathBuf>,
    pub hook_event_name: Option<String>,
    pub source: Option<String>,
    pub reason: Option<String>,
    pub prompt: Option<String>,
    pub tool_name: Option<String>,
    pub tool_input: Option<serde_json::Value>,
    pub message: Option<String>,
    pub title: Option<String>,
    pub notification_type: Option<String>,
    pub last_assistant_message: Option<String>,
}

impl HookPayload {
    pub fn parse(json: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(json)
    }

    /// 세션 상태 머신에 넣을 이벤트로 바꾼다. 상태에 영향이 없는 hook이면 None.
    pub fn to_event(&self, kind: HookKind) -> Option<SessionEvent> {
        let tool = || self.tool_name.clone().unwrap_or_else(|| "tool".into());
        Some(match kind {
            HookKind::SessionStart => SessionEvent::Started,
            HookKind::SessionEnd => SessionEvent::Ended,
            HookKind::UserPromptSubmit => SessionEvent::PromptSubmitted {
                prompt: self.prompt.clone().unwrap_or_default(),
            },
            HookKind::PreToolUse => SessionEvent::ToolStarting {
                activity: tool_summary(&tool(), self.tool_input.as_ref()),
            },
            HookKind::PostToolUse => SessionEvent::ToolFinished,
            HookKind::PermissionRequest => SessionEvent::PermissionRequested {
                tool_name: tool(),
                summary: tool_summary(&tool(), self.tool_input.as_ref()),
            },
            HookKind::Stop => SessionEvent::Stopped,
            HookKind::Notification => {
                let message = self.message.clone().unwrap_or_default();
                match self.notification_type.as_deref() {
                    Some("elicitation_dialog" | "agent_needs_input") => {
                        SessionEvent::InputRequested { message }
                    }
                    Some("permission_prompt") => SessionEvent::PermissionPrompt { message },
                    _ => return None,
                }
            }
        })
    }
}

const SUMMARY_MAX: usize = 120;

/// 카드에 보여 줄 한 줄 요약. 예: `Bash: ./gradlew test`
pub fn tool_summary(tool_name: &str, input: Option<&serde_json::Value>) -> String {
    let detail = input.and_then(|v| {
        [
            "command",
            "file_path",
            "path",
            "pattern",
            "url",
            "description",
        ]
        .iter()
        .find_map(|k| v.get(*k).and_then(|x| x.as_str()))
    });
    let text = match detail {
        Some(d) => format!("{tool_name}: {}", d.lines().next().unwrap_or("")),
        None => tool_name.to_owned(),
    };
    truncate(&text, SUMMARY_MAX)
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_owned()
    } else {
        let mut out: String = s.chars().take(max - 1).collect();
        out.push('…');
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> HookPayload {
        let path = format!("{}/tests/fixtures/{name}.json", env!("CARGO_MANIFEST_DIR"));
        HookPayload::parse(&std::fs::read(path).unwrap()).unwrap()
    }

    #[test]
    fn kind_round_trips() {
        for k in HookKind::ALL {
            assert_eq!(k.as_str().parse::<HookKind>().unwrap(), k);
        }
        assert!("Nope".parse::<HookKind>().is_err());
    }

    #[test]
    fn every_fixture_parses_with_session_and_cwd() {
        for k in HookKind::ALL {
            let p = fixture(k.as_str());
            assert_eq!(p.session_id, "abc123", "{}", k.as_str());
            assert!(p.cwd.is_some(), "{}", k.as_str());
            assert_eq!(p.hook_event_name.as_deref(), Some(k.as_str()));
        }
    }

    #[test]
    fn unknown_fields_and_missing_fields_are_fine() {
        let p = HookPayload::parse(br#"{"session_id":"x","brand_new_field":{"a":1}}"#).unwrap();
        assert_eq!(p.session_id, "x");
        assert!(p.cwd.is_none());
        let p = HookPayload::parse(b"{}").unwrap();
        assert_eq!(p.session_id, "");
    }

    #[test]
    fn pre_tool_use_summary() {
        let e = fixture("PreToolUse").to_event(HookKind::PreToolUse);
        assert_eq!(
            e,
            Some(SessionEvent::ToolStarting {
                activity: "Bash: npm test".into()
            })
        );
    }

    #[test]
    fn permission_request_event() {
        let e = fixture("PermissionRequest").to_event(HookKind::PermissionRequest);
        assert_eq!(
            e,
            Some(SessionEvent::PermissionRequested {
                tool_name: "Bash".into(),
                summary: "Bash: rm -rf node_modules".into()
            })
        );
    }

    #[test]
    fn notification_types_map_to_events() {
        let mut p = fixture("Notification");
        assert!(matches!(
            p.to_event(HookKind::Notification),
            Some(SessionEvent::PermissionPrompt { .. })
        ));
        p.notification_type = Some("elicitation_dialog".into());
        assert!(matches!(
            p.to_event(HookKind::Notification),
            Some(SessionEvent::InputRequested { .. })
        ));
        p.notification_type = Some("idle_prompt".into());
        assert_eq!(p.to_event(HookKind::Notification), None);
        p.notification_type = None;
        assert_eq!(p.to_event(HookKind::Notification), None);
    }

    #[test]
    fn summary_truncates_and_uses_first_line() {
        let long = serde_json::json!({ "command": format!("{}\nsecond", "x".repeat(300)) });
        let s = tool_summary("Bash", Some(&long));
        assert_eq!(s.chars().count(), SUMMARY_MAX);
        assert!(!s.contains("second"));
        assert_eq!(tool_summary("Task", None), "Task");
    }
}
