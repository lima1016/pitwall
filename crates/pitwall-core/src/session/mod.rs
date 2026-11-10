//! 세션 모델. 상태 전환은 `state` 모듈에서만 한다.

pub mod registry;
pub mod state;

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::project::ProjectId;

pub use state::Status;

/// Claude Code `session_id`.
pub type SessionId = String;

/// 세션 출처.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Origin {
    /// 사용자가 직접 띄운 세션. hook으로만 상태를 받는다.
    Observed,
}

/// 세션이 사용자에게 요청한 것.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PendingRequest {
    Permission { tool_name: String, summary: String },
    Input { message: String },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Session {
    pub id: SessionId,
    /// None이면 미분류.
    pub project_id: Option<ProjectId>,
    pub workspace: PathBuf,
    pub branch: Option<String>,
    pub origin: Origin,
    pub status: Status,
    /// 예: "Bash: ./gradlew test"
    pub current_activity: Option<String>,
    pub last_prompt: Option<String>,
    pub pending: Option<PendingRequest>,
    /// observed 세션에서 hook이 오래 오지 않음. 상태는 바꾸지 않는 표시용 플래그.
    pub stale: bool,
    pub started_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Session {
    pub fn new(id: SessionId, workspace: PathBuf, now: DateTime<Utc>) -> Self {
        Session {
            id,
            project_id: None,
            workspace,
            branch: None,
            origin: Origin::Observed,
            status: Status::Starting,
            current_activity: None,
            last_prompt: None,
            pending: None,
            stale: false,
            started_at: now,
            updated_at: now,
        }
    }
}

/// hook에서 나온, 상태 머신 입력 이벤트.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionEvent {
    Started,
    PromptSubmitted {
        prompt: String,
    },
    ToolStarting {
        activity: String,
    },
    ToolFinished,
    PermissionRequested {
        tool_name: String,
        summary: String,
    },
    /// Notification(permission_prompt). PermissionRequest의 보조 신호.
    PermissionPrompt {
        message: String,
    },
    /// Notification(elicitation_dialog, agent_needs_input).
    InputRequested {
        message: String,
    },
    Stopped,
    Ended,
}
