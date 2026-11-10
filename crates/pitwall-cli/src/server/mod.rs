//! HTTP 서버. 항상 127.0.0.1에만 바인딩한다.

mod api;
pub mod auth;
mod ws;

use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::middleware;
use axum::routing::{delete, get, post};
use pitwall_core::session::registry::Board;

use crate::embed;
use crate::runtime::{self, Runtime};

/// 변경 사항을 모아 UI로 보내는 간격 (PreToolUse 디바운스).
const FLUSH_EVERY: Duration = Duration::from_millis(250);
const STALE_CHECK_EVERY: Duration = Duration::from_secs(60);

#[derive(Clone)]
pub struct AppState {
    pub board: Arc<Board>,
    pub token: String,
    pub port: u16,
}

/// 바인딩 주소. 127.0.0.1 외 주소는 만들 수 없게 포트만 받는다.
pub fn bind_addr(port: u16) -> SocketAddr {
    SocketAddr::from((Ipv4Addr::LOCALHOST, port))
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route(
            "/api/projects",
            get(api::list_projects).post(api::add_project),
        )
        .route("/api/projects/{id}", delete(api::remove_project))
        .route("/api/sessions", get(api::list_sessions))
        .route("/hooks/{event}", post(api::receive_hook))
        .route("/ws", get(ws::handler))
        .fallback(embed::static_handler)
        .layer(middleware::from_fn_with_state(state.clone(), auth::guard))
        .with_state(state)
}

pub struct ServeOptions {
    pub config_path: PathBuf,
    pub port: Option<u16>,
    pub open_browser: bool,
}

/// 서버를 띄우고 Ctrl+C가 올 때까지 실행한다.
pub async fn serve(opts: ServeOptions) -> anyhow::Result<()> {
    let config_path = opts.config_path.clone();
    let board = tokio::task::spawn_blocking(move || Board::load(config_path)).await??;
    let board = Arc::new(board);
    let port = opts.port.unwrap_or_else(|| board.port());
    let addr = bind_addr(port);
    let listener = tokio::net::TcpListener::bind(addr).await?;

    let token = runtime::new_token();
    let runtime_path = runtime::path();
    Runtime {
        port,
        token: token.clone(),
        pid: std::process::id(),
    }
    .write(&runtime_path)?;

    spawn_background(board.clone());

    let url = format!("http://{addr}/#token={token}");
    tracing::info!("pitwall listening on http://{addr}/");
    println!("pitwall: {url}");
    if opts.open_browser && webbrowser::open(&url).is_err() {
        println!("Could not open a browser. Open the URL above manually.");
    }

    let state = AppState { board, token, port };
    let result = axum::serve(listener, router(state))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await;
    let _ = std::fs::remove_file(&runtime_path);
    Ok(result?)
}

fn spawn_background(board: Arc<Board>) {
    let b = board.clone();
    tokio::spawn(async move {
        let mut t = tokio::time::interval(FLUSH_EVERY);
        loop {
            t.tick().await;
            b.flush();
        }
    });
    tokio::spawn(async move {
        let mut t = tokio::time::interval(STALE_CHECK_EVERY);
        loop {
            t.tick().await;
            board.tick_stale(chrono::Utc::now());
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode, header};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    const TOKEN: &str = "test-token";
    const HOST: &str = "127.0.0.1:7311";

    fn state(dir: &std::path::Path) -> AppState {
        AppState {
            board: Arc::new(Board::load(dir.join("config.toml")).unwrap()),
            token: TOKEN.into(),
            port: 7311,
        }
    }

    fn req(method: &str, path: &str) -> axum::http::request::Builder {
        Request::builder()
            .method(method)
            .uri(path)
            .header(header::HOST, HOST)
    }

    fn authed(method: &str, path: &str) -> axum::http::request::Builder {
        req(method, path).header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
    }

    async fn send(st: &AppState, r: Request<Body>) -> (StatusCode, String, String) {
        let res = router(st.clone()).oneshot(r).await.unwrap();
        let status = res.status();
        let ct = res
            .headers()
            .get(header::CONTENT_TYPE)
            .map(|v| v.to_str().unwrap().to_owned())
            .unwrap_or_default();
        let body = res.into_body().collect().await.unwrap().to_bytes();
        (status, ct, String::from_utf8_lossy(&body).into_owned())
    }

    fn json_body(v: serde_json::Value) -> Body {
        Body::from(v.to_string())
    }

    #[test]
    fn binds_to_loopback_only() {
        let addr = bind_addr(pitwall_core::project::DEFAULT_PORT);
        assert_eq!(addr.ip().to_string(), "127.0.0.1");
        assert_eq!(addr.port(), 7311);
    }

    #[tokio::test]
    async fn root_serves_html_without_token() {
        let dir = tempfile::tempdir().unwrap();
        let st = state(dir.path());
        let (status, ct, body) = send(&st, req("GET", "/").body(Body::empty()).unwrap()).await;
        assert_eq!(status, StatusCode::OK);
        assert!(ct.starts_with("text/html"), "{ct}");
        assert!(body.contains("pitwall"));
    }

    #[tokio::test]
    async fn unknown_route_falls_back_and_missing_asset_404() {
        let dir = tempfile::tempdir().unwrap();
        let st = state(dir.path());
        let (s, ct, _) = send(&st, req("GET", "/projects/x").body(Body::empty()).unwrap()).await;
        assert_eq!(s, StatusCode::OK);
        assert!(ct.starts_with("text/html"));
        let (s, _, _) = send(
            &st,
            req("GET", "/assets/nope.js").body(Body::empty()).unwrap(),
        )
        .await;
        assert_eq!(s, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn api_without_token_is_401() {
        let dir = tempfile::tempdir().unwrap();
        let st = state(dir.path());
        for path in ["/api/projects", "/api/sessions"] {
            let (s, _, _) = send(&st, req("GET", path).body(Body::empty()).unwrap()).await;
            assert_eq!(s, StatusCode::UNAUTHORIZED, "{path}");
        }
        let r = req("GET", "/api/sessions")
            .header(header::AUTHORIZATION, "Bearer wrong")
            .body(Body::empty())
            .unwrap();
        assert_eq!(send(&st, r).await.0, StatusCode::UNAUTHORIZED);
        let r = req("POST", "/hooks/Stop").body(Body::from("{}")).unwrap();
        assert_eq!(send(&st, r).await.0, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn wrong_origin_or_host_is_403() {
        let dir = tempfile::tempdir().unwrap();
        let st = state(dir.path());
        let r = authed("GET", "/api/sessions")
            .header(header::ORIGIN, "http://evil.example")
            .body(Body::empty())
            .unwrap();
        assert_eq!(send(&st, r).await.0, StatusCode::FORBIDDEN);

        // DNS rebinding: 다른 이름으로 들어온 요청
        let r = Request::get("/")
            .header(header::HOST, "evil.example:7311")
            .body(Body::empty())
            .unwrap();
        assert_eq!(send(&st, r).await.0, StatusCode::FORBIDDEN);

        let r = authed("GET", "/api/sessions")
            .header(header::ORIGIN, "http://127.0.0.1:7311")
            .body(Body::empty())
            .unwrap();
        assert_eq!(send(&st, r).await.0, StatusCode::OK);
    }

    #[tokio::test]
    async fn hook_then_sessions_api_shows_session() {
        let dir = tempfile::tempdir().unwrap();
        let st = state(dir.path());
        let hook = json!({"session_id": "s1", "cwd": dir.path(), "prompt": "hi"});
        let r = authed("POST", "/hooks/UserPromptSubmit")
            .body(json_body(hook))
            .unwrap();
        assert_eq!(send(&st, r).await.0, StatusCode::NO_CONTENT);

        let (s, _, body) = send(
            &st,
            authed("GET", "/api/sessions").body(Body::empty()).unwrap(),
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v[0]["id"], "s1");
        assert_eq!(v[0]["status"], "working");
        assert_eq!(v[0]["last_prompt"], "hi");
    }

    #[tokio::test]
    async fn unknown_hook_404_and_bad_json_400() {
        let dir = tempfile::tempdir().unwrap();
        let st = state(dir.path());
        let r = authed("POST", "/hooks/Bogus")
            .body(Body::from("{}"))
            .unwrap();
        assert_eq!(send(&st, r).await.0, StatusCode::NOT_FOUND);
        let r = authed("POST", "/hooks/Stop")
            .body(Body::from("not json"))
            .unwrap();
        assert_eq!(send(&st, r).await.0, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn project_crud() {
        let dir = tempfile::tempdir().unwrap();
        let proj = dir.path().join("app");
        std::fs::create_dir_all(&proj).unwrap();
        let st = state(dir.path());

        let add = |p: &std::path::Path| {
            authed("POST", "/api/projects")
                .header(header::CONTENT_TYPE, "application/json")
                .body(json_body(json!({ "path": p })))
                .unwrap()
        };
        let (s, _, body) = send(&st, add(&proj)).await;
        assert_eq!(s, StatusCode::CREATED);
        let id = serde_json::from_str::<serde_json::Value>(&body).unwrap()["id"]
            .as_str()
            .unwrap()
            .to_owned();
        assert_eq!(send(&st, add(&proj)).await.0, StatusCode::CONFLICT);
        assert_eq!(
            send(&st, add(&proj.join("nope"))).await.0,
            StatusCode::BAD_REQUEST
        );

        let (_, _, body) = send(
            &st,
            authed("GET", "/api/projects").body(Body::empty()).unwrap(),
        )
        .await;
        let v: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v[0]["name"], "app");
        assert_eq!(v[0]["session_count"], 0);

        let del = || {
            authed("DELETE", &format!("/api/projects/{id}"))
                .body(Body::empty())
                .unwrap()
        };
        assert_eq!(send(&st, del()).await.0, StatusCode::NO_CONTENT);
        assert_eq!(send(&st, del()).await.0, StatusCode::NOT_FOUND);
    }

    /// 실제 소켓으로 WebSocket 연결: 토큰 프로토콜, 스냅샷, 실시간 upsert.
    #[tokio::test]
    async fn websocket_streams_session_upserts() {
        use futures_util::StreamExt;
        use tokio_tungstenite::tungstenite::client::IntoClientRequest;

        let dir = tempfile::tempdir().unwrap();
        let listener = tokio::net::TcpListener::bind(bind_addr(0)).await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let mut st = state(dir.path());
        st.port = port;
        let board = st.board.clone();
        tokio::spawn(async move { axum::serve(listener, router(st)).await });

        // 토큰 없이 연결하면 거절
        let url = format!("ws://127.0.0.1:{port}/ws");
        assert!(
            tokio_tungstenite::connect_async(url.as_str())
                .await
                .is_err()
        );

        let mut request = url.as_str().into_client_request().unwrap();
        request.headers_mut().insert(
            header::SEC_WEBSOCKET_PROTOCOL,
            format!("pitwall, {TOKEN}").parse().unwrap(),
        );
        let (mut ws, resp) = tokio_tungstenite::connect_async(request).await.unwrap();
        assert_eq!(
            resp.headers().get(header::SEC_WEBSOCKET_PROTOCOL).unwrap(),
            "pitwall"
        );

        let payload = pitwall_core::hooks::HookPayload {
            session_id: "s1".into(),
            cwd: Some(dir.path().to_owned()),
            ..Default::default()
        };
        board.handle_hook(
            pitwall_core::hooks::HookKind::Stop,
            &payload,
            chrono::Utc::now(),
        );
        board.flush();

        let msg = tokio::time::timeout(Duration::from_secs(5), ws.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let v: serde_json::Value = serde_json::from_str(msg.to_text().unwrap()).unwrap();
        assert_eq!(v["type"], "session.upserted");
        assert_eq!(v["session"]["id"], "s1");
        assert_eq!(v["session"]["status"], "idle");
    }

    use serde_json::json;
}
