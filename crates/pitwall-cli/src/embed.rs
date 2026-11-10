//! 빌드된 웹 UI(`web/dist`)를 실행 파일에 내장해서 제공한다.
//! 디버그 빌드에서는 rust-embed가 디스크에서 직접 읽으므로 `npm run build` 후 재컴파일이 필요 없다.

use axum::http::{StatusCode, Uri, header};
use axum::response::{Html, IntoResponse, Response};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "../../web/dist"]
struct Assets;

/// `web/dist`가 비어 있을 때(프론트 미빌드) 보여 주는 안내 페이지.
const NOT_BUILT_PAGE: &str = "<!doctype html><html><head><meta charset=\"utf-8\">\
<title>pitwall</title></head><body><h1>pitwall</h1>\
<p>Web UI is not built yet. Run <code>cd web &amp;&amp; npm run build</code>.</p>\
</body></html>";

pub async fn static_handler(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');

    if !path.is_empty() {
        if let Some(file) = Assets::get(path) {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            return ([(header::CONTENT_TYPE, mime.as_ref())], file.data).into_response();
        }
        // 확장자가 있는 경로는 실제 파일 요청이다. 없으면 404.
        if path
            .rsplit('/')
            .next()
            .is_some_and(|name| name.contains('.'))
        {
            return StatusCode::NOT_FOUND.into_response();
        }
    }

    // 그 외 경로는 SPA 라우팅을 위해 index.html을 돌려준다.
    match Assets::get("index.html") {
        Some(index) => Html(index.data).into_response(),
        None => Html(NOT_BUILT_PAGE).into_response(),
    }
}
