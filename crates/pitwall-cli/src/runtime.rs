//! `~/.pitwall/runtime.json`: 실행 중인 서버의 포트와 토큰. hook이 읽는다.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Runtime {
    pub port: u16,
    pub token: String,
    pub pid: u32,
}

pub fn path() -> PathBuf {
    pitwall_core::paths::pitwall_home().join("runtime.json")
}

/// 256비트 랜덤 토큰 (hex 64자).
pub fn new_token() -> String {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).expect("OS random number generator failed");
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

impl Runtime {
    pub fn write(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let json = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
        write_private(path, &json)
    }

    /// 없거나 읽을 수 없으면 None.
    pub fn read(path: &Path) -> Option<Runtime> {
        serde_json::from_slice(&std::fs::read(path).ok()?).ok()
    }
}

/// Unix는 0600으로 만든다. Windows는 사용자 프로필의 기본 ACL에 맡긴다 (DESIGN.md 결정 기록).
#[cfg(unix)]
fn write_private(path: &Path, data: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    f.write_all(data)
}

#[cfg(not(unix))]
fn write_private(path: &Path, data: &[u8]) -> std::io::Result<()> {
    std::fs::write(path, data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_is_256_bit_hex_and_random() {
        let a = new_token();
        assert_eq!(a.len(), 64);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, new_token());
    }

    #[test]
    fn round_trip_and_missing() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("sub").join("runtime.json");
        assert_eq!(Runtime::read(&p), None);
        let rt = Runtime {
            port: 7311,
            token: new_token(),
            pid: 1,
        };
        rt.write(&p).unwrap();
        assert_eq!(Runtime::read(&p), Some(rt));
    }

    #[cfg(unix)]
    #[test]
    fn file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("runtime.json");
        Runtime {
            port: 1,
            token: "t".into(),
            pid: 1,
        }
        .write(&p)
        .unwrap();
        let mode = std::fs::metadata(&p).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
