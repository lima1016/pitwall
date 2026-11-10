//! 내부 이벤트 버스. UI로 보낼 변경 사항을 broadcast로 흘린다.

use serde::Serialize;
use tokio::sync::broadcast;

use crate::session::{Session, SessionId};

/// WebSocket으로 그대로 직렬화되는 이벤트 (DESIGN.md 6.2).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type")]
pub enum BoardEvent {
    #[serde(rename = "session.upserted")]
    SessionUpserted { session: Box<Session> },
    #[serde(rename = "session.removed")]
    SessionRemoved { session_id: SessionId },
}

#[derive(Debug, Clone)]
pub struct EventBus {
    tx: broadcast::Sender<BoardEvent>,
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        EventBus { tx }
    }

    /// 구독자가 없어도 오류가 아니다.
    pub fn publish(&self, event: BoardEvent) {
        let _ = self.tx.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<BoardEvent> {
        self.tx.subscribe()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        EventBus::new(256)
    }
}
