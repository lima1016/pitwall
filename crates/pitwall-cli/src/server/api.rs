//! REST 핸들러와 hook 수신.

use std::path::PathBuf;

use axum::Json;
use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use pitwall_core::hooks::{HookKind, HookPayload};
use pitwall_core::project::AddProjectError;
use pitwall_core::session::registry::BoardError;
use serde::Deserialize;
use serde_json::json;

use super::AppState;

fn error(status: StatusCode, message: impl ToString) -> Response {
    (status, Json(json!({ "error": message.to_string() }))).into_response()
}

fn board_error(e: BoardError) -> Response {
    let status = match &e {
        BoardError::Add(AddProjectError::NotFound(_)) => StatusCode::BAD_REQUEST,
        BoardError::Add(AddProjectError::Duplicate(_)) => StatusCode::CONFLICT,
        BoardError::Config(_) => StatusCode::INTERNAL_SERVER_ERROR,
    };
    error(status, e)
}

/// 블로킹 작업(git 명령, 파일 쓰기)은 런타임 스레드를 막지 않게 넘긴다.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    tokio::task::spawn_blocking(f).await.ok()
}

fn internal() -> Response {
    error(StatusCode::INTERNAL_SERVER_ERROR, "internal error")
}

pub async fn list_projects(State(st): State<AppState>) -> Response {
    Json(st.board.projects()).into_response()
}

#[derive(Deserialize)]
pub struct AddProject {
    path: PathBuf,
}

pub async fn add_project(State(st): State<AppState>, Json(body): Json<AddProject>) -> Response {
    let board = st.board.clone();
    match blocking(move || board.add_project(&body.path)).await {
        Some(Ok(project)) => (StatusCode::CREATED, Json(project)).into_response(),
        Some(Err(e)) => board_error(e),
        None => internal(),
    }
}

pub async fn remove_project(State(st): State<AppState>, Path(id): Path<String>) -> Response {
    let board = st.board.clone();
    match blocking(move || board.remove_project(&id)).await {
        Some(Ok(true)) => StatusCode::NO_CONTENT.into_response(),
        Some(Ok(false)) => error(StatusCode::NOT_FOUND, "project not found"),
        Some(Err(e)) => board_error(e),
        None => internal(),
    }
}

#[derive(Deserialize)]
pub struct SessionFilter {
    project: Option<String>,
}

pub async fn list_sessions(State(st): State<AppState>, Query(q): Query<SessionFilter>) -> Response {
    Json(st.board.sessions(q.project.as_deref())).into_response()
}

/// `pitwall hook <event>` 전용. 본문은 Claude Code hook 입력 JSON 그대로.
/// 프롬프트와 도구 입력은 로그에 남기지 않는다.
pub async fn receive_hook(
    State(st): State<AppState>,
    Path(event): Path<String>,
    body: Bytes,
) -> Response {
    let Ok(kind) = event.parse::<HookKind>() else {
        return error(StatusCode::NOT_FOUND, "unknown hook event");
    };
    let payload = match HookPayload::parse(&body) {
        Ok(p) => p,
        Err(e) => return error(StatusCode::BAD_REQUEST, e),
    };
    tracing::debug!(event = kind.as_str(), session = %payload.session_id, "hook");
    let board = st.board.clone();
    match blocking(move || board.handle_hook(kind, &payload, chrono::Utc::now())).await {
        Some(_) => StatusCode::NO_CONTENT.into_response(),
        None => internal(),
    }
}
