//! git 명령 실행. `git2`는 쓰지 않는다 (DESIGN.md 3.2).

use std::path::{Path, PathBuf};
use std::process::Command;

fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// `git worktree list --porcelain` 결과의 worktree 경로들. git 저장소가 아니면 빈 목록.
pub fn worktree_paths(repo: &Path) -> Vec<PathBuf> {
    git(repo, &["worktree", "list", "--porcelain"])
        .map(|s| parse_worktree_paths(&s))
        .unwrap_or_default()
}

fn parse_worktree_paths(porcelain: &str) -> Vec<PathBuf> {
    porcelain
        .lines()
        .filter_map(|l| l.strip_prefix("worktree "))
        .map(PathBuf::from)
        .collect()
}

/// 현재 브랜치 이름. detached HEAD나 git 저장소가 아니면 None.
pub fn current_branch(dir: &Path) -> Option<String> {
    let name = git(dir, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    let name = name.trim();
    (!name.is_empty() && name != "HEAD").then(|| name.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_porcelain_output() {
        let out = "worktree C:/work/app\nHEAD abc\nbranch refs/heads/main\n\nworktree C:/work/app-feat\nHEAD def\nbranch refs/heads/feat\n\n";
        assert_eq!(
            parse_worktree_paths(out),
            vec![
                PathBuf::from("C:/work/app"),
                PathBuf::from("C:/work/app-feat")
            ]
        );
    }

    #[test]
    fn non_repo_has_no_worktrees_or_branch() {
        let dir = tempfile::tempdir().unwrap();
        assert!(worktree_paths(dir.path()).is_empty());
        assert_eq!(current_branch(dir.path()), None);
    }
}
