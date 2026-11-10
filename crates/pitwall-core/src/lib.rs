//! pitwall 도메인 로직. axum이나 HTTP 타입에 의존하지 않는다.

/// pitwall 버전 (Cargo 패키지 버전).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_not_empty() {
        assert!(!VERSION.is_empty());
    }
}
