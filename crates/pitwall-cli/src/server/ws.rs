//! WebSocket `/ws`: 서버 → 브라우저 이벤트 스트림.

use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use pitwall_core::events::BoardEvent;
use tokio::sync::broadcast::error::RecvError;

use super::AppState;
use super::auth::WS_PROTOCOL;

pub async fn handler(State(st): State<AppState>, ws: WebSocketUpgrade) -> Response {
    ws.protocols([WS_PROTOCOL])
        .on_upgrade(move |socket| run(socket, st))
}

async fn send_snapshot(socket: &mut WebSocket, st: &AppState) -> bool {
    for session in st.board.sessions(None) {
        let event = BoardEvent::SessionUpserted {
            session: Box::new(session),
        };
        if !send(socket, &event).await {
            return false;
        }
    }
    true
}

async fn send(socket: &mut WebSocket, event: &BoardEvent) -> bool {
    match serde_json::to_string(event) {
        Ok(text) => socket.send(Message::Text(text.into())).await.is_ok(),
        Err(_) => true,
    }
}

async fn run(mut socket: WebSocket, st: AppState) {
    // 구독을 먼저 해야 스냅샷과 그 사이의 변경을 놓치지 않는다.
    let mut rx = st.board.subscribe();
    if !send_snapshot(&mut socket, &st).await {
        return;
    }
    loop {
        tokio::select! {
            event = rx.recv() => match event {
                Ok(event) => {
                    if !send(&mut socket, &event).await {
                        return;
                    }
                }
                // 너무 밀렸으면 전체 상태를 다시 보낸다.
                Err(RecvError::Lagged(_)) => {
                    if !send_snapshot(&mut socket, &st).await {
                        return;
                    }
                }
                Err(RecvError::Closed) => return,
            },
            msg = socket.recv() => match msg {
                // M1에서는 클라이언트 → 서버 메시지를 쓰지 않는다 (pty.*는 M2).
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => return,
                Some(Ok(_)) => {}
            },
        }
    }
}
