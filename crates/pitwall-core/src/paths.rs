//! 경로 처리 유틸. 경로 비교와 pitwall 파일 위치는 모두 여기를 거친다.
//! Windows: 대소문자 무시, `\`와 `/` 통일, `\\?\` 접두사 제거.

use std::path::{Path, PathBuf};

/// 실제 경로로 정규화한다. 경로가 없으면 존재하는 가장 가까운 상위 폴더까지 실제 경로로 풀고
/// 나머지를 붙인다. 그래야 심볼릭 링크(macOS `/var` → `/private/var`)나 8.3 짧은 이름
/// (Windows `RUNNER~1`) 아래의 없는 경로도 같은 키가 나온다.
pub fn normalize(path: &Path) -> PathBuf {
    if let Ok(real) = dunce::canonicalize(path) {
        return real;
    }
    let mut rest = Vec::new();
    let mut cur = path;
    while let Some(parent) = cur.parent() {
        if let Some(name) = cur.file_name() {
            rest.push(name.to_owned());
        }
        if let Ok(real) = dunce::canonicalize(parent) {
            return rest.iter().rev().fold(real, |acc, n| acc.join(n));
        }
        cur = parent;
    }
    dunce::simplified(path).to_path_buf()
}

/// 비교용 문자열 키. 같은 위치면 같은 키가 나온다.
pub fn compare_key(path: &Path) -> String {
    key_with(&normalize(path), cfg!(windows))
}

fn key_with(path: &Path, case_insensitive: bool) -> String {
    let mut s = path.to_string_lossy().replace('\\', "/");
    while s.len() > 1 && s.ends_with('/') && !s.ends_with(":/") {
        s.pop();
    }
    if case_insensitive {
        s = s.to_lowercase();
    }
    s
}

/// `child`가 `parent`와 같거나 그 아래에 있으면 true.
pub fn is_within(child: &Path, parent: &Path) -> bool {
    is_within_key(&compare_key(child), &compare_key(parent))
}

fn is_within_key(child: &str, parent: &str) -> bool {
    child == parent
        || child
            .strip_prefix(parent)
            .is_some_and(|rest| rest.starts_with('/') || parent.ends_with('/'))
}

/// 사용자 홈 디렉터리.
pub fn home_dir() -> Option<PathBuf> {
    std::env::home_dir()
}

/// pitwall 데이터 폴더. 기본 `~/.pitwall`, `PITWALL_HOME`으로 바꿀 수 있다 (테스트용).
pub fn pitwall_home() -> PathBuf {
    match std::env::var_os("PITWALL_HOME") {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".pitwall"),
    }
}

/// Claude Code 사용자 전역 설정 파일 위치.
pub fn claude_user_settings() -> Option<PathBuf> {
    home_dir().map(|h| h.join(".claude").join("settings.json"))
}

/// 표시용 이름: 경로의 마지막 조각.
pub fn display_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_keys_ignore_case_and_separators() {
        let a = key_with(Path::new(r"C:\Work\CDP-Backend\"), true);
        let b = key_with(Path::new("c:/work/cdp-backend"), true);
        assert_eq!(a, b);
    }

    #[test]
    fn unix_keys_keep_case() {
        assert_ne!(
            key_with(Path::new("/work/App"), false),
            key_with(Path::new("/work/app"), false)
        );
    }

    #[test]
    fn drive_root_keeps_slash() {
        assert_eq!(key_with(Path::new(r"C:\"), true), "c:/");
    }

    #[test]
    fn within_checks_component_boundary() {
        assert!(is_within_key("c:/work/app", "c:/work/app"));
        assert!(is_within_key("c:/work/app/src", "c:/work/app"));
        assert!(!is_within_key("c:/work/app-wt", "c:/work/app"));
        assert!(is_within_key("c:/work", "c:/"));
    }

    #[test]
    fn within_real_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("a").join("b");
        std::fs::create_dir_all(&sub).unwrap();
        assert!(is_within(&sub, dir.path()));
        assert!(!is_within(dir.path(), &sub));
    }

    #[test]
    fn missing_child_resolves_through_existing_parent() {
        let dir = tempfile::tempdir().unwrap();
        let real = normalize(dir.path());
        let missing = dir.path().join("not").join("yet");
        assert_eq!(normalize(&missing), real.join("not").join("yet"));
        assert!(is_within(&missing, dir.path()));
    }

    /// 링크 아래의 없는 경로도 실제 경로와 같은 키가 나와야 한다 (macOS /var, Windows 8.3 이름).
    #[cfg(unix)]
    #[test]
    fn missing_path_under_symlink_matches_real_path() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real");
        std::fs::create_dir(&real).unwrap();
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        assert_eq!(
            compare_key(&link.join("src")),
            compare_key(&real.join("src"))
        );
    }

    #[test]
    fn normalize_strips_verbatim_prefix() {
        let dir = tempfile::tempdir().unwrap();
        let n = normalize(dir.path());
        assert!(!n.to_string_lossy().starts_with(r"\\?\"));
    }
}
