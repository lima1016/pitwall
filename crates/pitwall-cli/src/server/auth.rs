//! 요청 검사: Host, Origin, 토큰 (DESIGN.md 7장).

use axum::extract::{Request, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use super::AppState;

/// WebSocket 하위 프로토콜 이름. 브라우저는 `["pitwall", token]`으로 연결한다.
pub const WS_PROTOCOL: &str = "pitwall";

pub async fn guard(State(st): State<AppState>, req: Request, next: Next) -> Response {
    let headers = req.headers();
    let expected_host = format!("127.0.0.1:{}", st.port);
    let host = headers.get(header::HOST).and_then(|v| v.to_str().ok());
    if host != Some(expected_host.as_str()) {
        return (StatusCode::FORBIDDEN, "forbidden host").into_response();
    }
    // Origin이 없으면 비브라우저 클라이언트(`pitwall hook`)다. 있으면 반드시 우리 주소여야 한다.
    if let Some(origin) = headers.get(header::ORIGIN)
        && origin.to_str().ok() != Some(format!("http://{expected_host}").as_str())
    {
        return (StatusCode::FORBIDDEN, "forbidden origin").into_response();
    }

    let path = req.uri().path();
    let protected = path.starts_with("/api/") || path.starts_with("/hooks/") || path == "/ws";
    if protected && !has_token(headers, &st.token) {
        return (StatusCode::UNAUTHORIZED, "missing or invalid token").into_response();
    }
    next.run(req).await
}

fn has_token(headers: &HeaderMap, token: &str) -> bool {
    let bearer = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));
    if bearer.is_some_and(|t| constant_time_eq(t, token)) {
        return true;
    }
    // WebSocket: Sec-WebSocket-Protocol: pitwall, <token>
    headers
        .get(header::SEC_WEBSOCKET_PROTOCOL)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| {
            let mut parts = v.split(',').map(str::trim);
            parts.any(|p| p == WS_PROTOCOL)
                && v.split(',').any(|p| constant_time_eq(p.trim(), token))
        })
}

fn constant_time_eq(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_time_eq_works() {
        assert!(constant_time_eq("abc", "abc"));
        assert!(!constant_time_eq("abc", "abd"));
        assert!(!constant_time_eq("abc", "abcd"));
    }

    #[test]
    fn token_from_bearer_or_ws_protocol() {
        let mut h = HeaderMap::new();
        assert!(!has_token(&h, "t0k"));
        h.insert(header::AUTHORIZATION, "Bearer t0k".parse().unwrap());
        assert!(has_token(&h, "t0k"));

        let mut h = HeaderMap::new();
        h.insert(
            header::SEC_WEBSOCKET_PROTOCOL,
            "pitwall, t0k".parse().unwrap(),
        );
        assert!(has_token(&h, "t0k"));
        h.insert(
            header::SEC_WEBSOCKET_PROTOCOL,
            "other, t0k".parse().unwrap(),
        );
        assert!(!has_token(&h, "t0k"));
    }
}
