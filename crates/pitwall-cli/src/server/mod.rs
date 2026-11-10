//! HTTP 서버. 항상 127.0.0.1에만 바인딩한다.

use std::net::{Ipv4Addr, SocketAddr};

use axum::Router;

use crate::embed;

/// 기본 포트. 사용 중일 때의 동작은 DESIGN.md 12장 열린 질문.
pub const DEFAULT_PORT: u16 = 7311;

/// 바인딩 주소. 127.0.0.1 외 주소는 만들 수 없게 포트만 받는다.
pub fn bind_addr(port: u16) -> SocketAddr {
    SocketAddr::from((Ipv4Addr::LOCALHOST, port))
}

pub fn router() -> Router {
    Router::new().fallback(embed::static_handler)
}

/// 서버를 띄우고 Ctrl+C가 올 때까지 실행한다.
pub async fn serve(port: u16) -> std::io::Result<()> {
    let addr = bind_addr(port);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("pitwall listening on http://{addr}/");
    println!("pitwall: http://{addr}/");
    axum::serve(listener, router())
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
}
#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode, header};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    async fn get(path: &str) -> (StatusCode, String, String) {
        let res = router()
            .oneshot(Request::get(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = res.status();
        let content_type = res
            .headers()
            .get(header::CONTENT_TYPE)
            .map(|v| v.to_str().unwrap().to_owned())
            .unwrap_or_default();
        let body = res.into_body().collect().await.unwrap().to_bytes();
        (
            status,
            content_type,
            String::from_utf8_lossy(&body).into_owned(),
        )
    }

    #[test]
    fn binds_to_loopback_only() {
        let addr = bind_addr(DEFAULT_PORT);
        assert_eq!(addr.ip().to_string(), "127.0.0.1");
        assert_eq!(addr.port(), 7311);
    }

    #[tokio::test]
    async fn root_serves_html() {
        let (status, content_type, body) = get("/").await;
        assert_eq!(status, StatusCode::OK);
        assert!(content_type.starts_with("text/html"), "{content_type}");
        assert!(body.contains("pitwall"));
    }

    #[tokio::test]
    async fn unknown_route_falls_back_to_index() {
        let (status, content_type, _) = get("/projects/abc").await;
        assert_eq!(status, StatusCode::OK);
        assert!(content_type.starts_with("text/html"), "{content_type}");
    }

    #[tokio::test]
    async fn missing_asset_file_is_404() {
        let (status, _, _) = get("/assets/does-not-exist.js").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
}
