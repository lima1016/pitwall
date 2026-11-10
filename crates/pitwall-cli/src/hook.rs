//! `pitwall hook <event>`: Claude Code가 실행하는 명령.
//! stdin의 hook JSON을 서버로 보낸다. 어떤 경우에도 Claude Code를 막지 않는다:
//! 서버가 꺼져 있거나, 응답이 없거나, 오류가 나도 아무것도 출력하지 않고 exit 0.

use std::io::Read;
use std::time::Duration;

use crate::runtime::{self, Runtime};

/// 일반 이벤트 전송 타임아웃. SessionEnd hook 전체 예산(1.5초)보다 짧다.
const SEND_TIMEOUT: Duration = Duration::from_secs(1);
/// stdin 최대 크기. 이보다 큰 입력은 잘라내지 않고 보내지 않는다.
const MAX_INPUT: u64 = 4 * 1024 * 1024;

pub fn run(event: &str) {
    let mut body = Vec::new();
    if std::io::stdin()
        .take(MAX_INPUT + 1)
        .read_to_end(&mut body)
        .is_err()
        || body.len() as u64 > MAX_INPUT
    {
        return;
    }
    let Some(rt) = Runtime::read(&runtime::path()) else {
        return;
    };
    let _ = send(&rt, event, &body);
}

fn send(rt: &Runtime, event: &str, body: &[u8]) -> Result<(), ureq::Error> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(SEND_TIMEOUT))
        .http_status_as_error(false)
        .build()
        .into();
    agent
        .post(format!("http://127.0.0.1:{}/hooks/{event}", rt.port))
        .header("Authorization", format!("Bearer {}", rt.token))
        .content_type("application/json")
        .send(body)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unreachable_server_returns_quickly_with_error() {
        // 아무도 듣지 않는 포트. 연결 거부가 바로 와야 한다.
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let rt = Runtime {
            port,
            token: "t".into(),
            pid: 0,
        };
        let start = std::time::Instant::now();
        assert!(send(&rt, "Stop", b"{}").is_err());
        assert!(start.elapsed() < Duration::from_secs(3));
    }
}
