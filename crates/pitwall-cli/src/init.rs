//! `pitwall init`: Claude Code 사용자 설정에 hook을 설치/제거한다 (DESIGN.md 4.1).
//! 원칙: 백업 → 병합 → diff 확인 → 쓰기. 기존 hook은 지우거나 바꾸지 않는다.
//! 파싱할 수 없는 파일에는 절대 쓰지 않는다.

use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

use pitwall_core::hooks::HookKind;
use serde_json::{Map, Value, json};

#[derive(Debug, thiserror::Error)]
pub enum InitError {
    #[error("{path} is not valid JSON, so it was not modified: {source}")]
    BadJson {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("{0}")]
    BadShape(String),
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
}

/// settings.json에 넣는 hook 명령. 실행 파일 절대 경로를 쓴다.
/// `/`로 통일해서 bash, cmd 어느 쪽에서 실행돼도 경로가 깨지지 않게 한다.
pub fn hook_command(exe: &Path, kind: HookKind) -> String {
    let exe = exe.to_string_lossy().replace('\\', "/");
    format!("\"{exe}\" hook {}", kind.as_str())
}

/// pitwall이 추가한 명령인지. 실행 파일 이름(`pitwall`, `pitwall.exe`)과 ` hook `이 함께 있어야 한다.
pub fn is_pitwall_command(cmd: &str) -> bool {
    let lower = cmd.to_ascii_lowercase();
    let Some((program, _)) = lower.split_once(" hook ") else {
        return false;
    };
    let program = program.trim().trim_matches('"');
    let name = program.rsplit(['/', '\\']).next().unwrap_or(program);
    name == "pitwall" || name == "pitwall.exe"
}

fn shape(msg: &str) -> InitError {
    InitError::BadShape(format!(
        "unexpected settings.json shape: {msg}. Nothing was modified."
    ))
}

fn hooks_object(root: &mut Value) -> Result<&mut Map<String, Value>, InitError> {
    let obj = root
        .as_object_mut()
        .ok_or_else(|| shape("top level is not an object"))?;
    obj.entry("hooks")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| shape("\"hooks\" is not an object"))
}

/// pitwall hook을 추가한다. 이미 있으면 그대로, 경로가 바뀌었으면 명령만 고친다.
pub fn install(root: &mut Value, exe: &Path) -> Result<(), InitError> {
    let hooks = hooks_object(root)?;
    for kind in HookKind::ALL {
        let wanted = hook_command(exe, kind);
        let groups = hooks
            .entry(kind.as_str())
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .ok_or_else(|| shape(&format!("hooks.{} is not an array", kind.as_str())))?;

        let mut found = false;
        for handler in groups
            .iter_mut()
            .filter_map(|g| g.get_mut("hooks").and_then(Value::as_array_mut))
            .flatten()
        {
            let is_ours = handler
                .get("command")
                .and_then(Value::as_str)
                .is_some_and(|c| {
                    is_pitwall_command(c) && c.ends_with(&format!(" hook {}", kind.as_str()))
                });
            if is_ours {
                found = true;
                handler["command"] = Value::String(wanted.clone());
            }
        }
        if !found {
            groups.push(json!({ "hooks": [{ "type": "command", "command": wanted }] }));
        }
    }
    Ok(())
}

/// pitwall이 추가한 hook만 제거한다. 비게 된 그룹과 배열도 정리한다.
pub fn uninstall(root: &mut Value) -> Result<(), InitError> {
    let Some(obj) = root.as_object_mut() else {
        return Err(shape("top level is not an object"));
    };
    let Some(hooks) = obj.get_mut("hooks") else {
        return Ok(());
    };
    let hooks = hooks
        .as_object_mut()
        .ok_or_else(|| shape("\"hooks\" is not an object"))?;
    for groups in hooks.values_mut().filter_map(Value::as_array_mut) {
        for group in groups.iter_mut() {
            if let Some(list) = group.get_mut("hooks").and_then(Value::as_array_mut) {
                list.retain(|h| {
                    !h.get("command")
                        .and_then(Value::as_str)
                        .is_some_and(is_pitwall_command)
                });
            }
        }
        groups.retain(|g| {
            g.get("hooks")
                .and_then(Value::as_array)
                .is_none_or(|l| !l.is_empty())
        });
    }
    hooks.retain(|_, v| v.as_array().is_none_or(|a| !a.is_empty()));
    if hooks.is_empty() {
        obj.remove("hooks");
    }
    Ok(())
}

/// 바꾼 뒤의 파일 내용. 변경이 없으면 None.
pub fn plan(
    original: Option<&str>,
    path: &Path,
    exe: &Path,
    remove: bool,
) -> Result<Option<String>, InitError> {
    let mut root = match original {
        Some(text) if !text.trim().is_empty() => {
            serde_json::from_str(text).map_err(|source| InitError::BadJson {
                path: path.to_owned(),
                source,
            })?
        }
        _ => json!({}),
    };
    let before = root.clone();
    if remove {
        uninstall(&mut root)?;
    } else {
        install(&mut root, exe)?;
    }
    if root == before {
        return Ok(None);
    }
    let mut text = serde_json::to_string_pretty(&root).expect("JSON value serializes");
    text.push('\n');
    Ok(Some(text))
}

pub struct Options {
    pub settings: PathBuf,
    pub exe: PathBuf,
    pub yes: bool,
    pub uninstall: bool,
}

/// 실제 실행. 사용자 확인은 `confirm`으로 받는다 (테스트에서 바꿀 수 있게).
pub fn run(
    opts: &Options,
    out: &mut impl Write,
    confirm: &mut impl BufRead,
) -> Result<(), InitError> {
    let io = |source| InitError::Io {
        path: opts.settings.clone(),
        source,
    };
    let original = match std::fs::read_to_string(&opts.settings) {
        Ok(t) => Some(t),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(io(e)),
    };
    let Some(updated) = plan(
        original.as_deref(),
        &opts.settings,
        &opts.exe,
        opts.uninstall,
    )?
    else {
        let _ = writeln!(out, "No changes needed: {}", opts.settings.display());
        return Ok(());
    };

    let old = original.clone().unwrap_or_default();
    let diff = similar::TextDiff::from_lines(&old, &updated);
    let _ = writeln!(
        out,
        "{}",
        diff.unified_diff().context_radius(3).header(
            &format!("{} (current)", opts.settings.display()),
            "(after pitwall init)"
        )
    );

    if !opts.yes {
        let _ = write!(out, "Apply these changes? [y/N] ");
        let _ = out.flush();
        let mut answer = String::new();
        let _ = confirm.read_line(&mut answer);
        if !matches!(answer.trim(), "y" | "Y" | "yes") {
            let _ = writeln!(out, "Cancelled. Nothing was modified.");
            return Ok(());
        }
    }

    if let Some(dir) = opts.settings.parent() {
        std::fs::create_dir_all(dir).map_err(io)?;
    }
    if original.is_some() {
        let stamp = chrono::Local::now().format("%Y%m%d%H%M%S");
        let mut backup = opts.settings.clone().into_os_string();
        backup.push(format!(".pitwall-backup-{stamp}"));
        let backup = PathBuf::from(backup);
        std::fs::copy(&opts.settings, &backup).map_err(io)?;
        let _ = writeln!(out, "Backup: {}", backup.display());
    }
    // 임시 파일에 쓰고 바꿔치기해서 중간에 끊겨도 원본이 깨지지 않게 한다.
    let mut tmp = opts.settings.clone().into_os_string();
    tmp.push(".pitwall-tmp");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, &updated).map_err(io)?;
    std::fs::rename(&tmp, &opts.settings).map_err(io)?;
    let _ = writeln!(
        out,
        "{} {}",
        if opts.uninstall {
            "Removed pitwall hooks from"
        } else {
            "Installed pitwall hooks in"
        },
        opts.settings.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXE: &str = r"C:\tools\pitwall.exe";

    fn exe() -> PathBuf {
        PathBuf::from(EXE)
    }

    fn opts(settings: PathBuf, yes: bool, uninstall: bool) -> Options {
        Options {
            settings,
            exe: exe(),
            yes,
            uninstall,
        }
    }

    fn run_with(o: &Options, answer: &str) -> (Result<(), InitError>, String) {
        let mut out = Vec::new();
        let r = run(o, &mut out, &mut answer.as_bytes());
        (r, String::from_utf8(out).unwrap())
    }

    #[test]
    fn command_format_and_detection() {
        let c = hook_command(&exe(), HookKind::Stop);
        assert_eq!(c, "\"C:/tools/pitwall.exe\" hook Stop");
        assert!(is_pitwall_command(&c));
        assert!(is_pitwall_command("pitwall hook Stop"));
        assert!(is_pitwall_command(
            "/usr/local/bin/pitwall hook Notification"
        ));
        assert!(!is_pitwall_command("my-pitwall-helper hook Stop"));
        assert!(!is_pitwall_command("echo pitwall"));
        assert!(!is_pitwall_command("notify.sh"));
    }

    #[test]
    fn install_into_empty_adds_all_events() {
        let mut v = json!({});
        install(&mut v, &exe()).unwrap();
        for k in HookKind::ALL {
            assert_eq!(
                v["hooks"][k.as_str()][0]["hooks"][0]["command"],
                hook_command(&exe(), k)
            );
        }
    }

    #[test]
    fn install_keeps_existing_hooks_and_other_settings() {
        let mut v = json!({
            "model": "opus",
            "hooks": {
                "Stop": [{ "hooks": [{ "type": "command", "command": "notify.sh" }] }],
                "PreToolUse": [{ "matcher": "Bash", "hooks": [{ "type": "command", "command": "guard.sh" }] }]
            }
        });
        install(&mut v, &exe()).unwrap();
        assert_eq!(v["model"], "opus");
        assert_eq!(v["hooks"]["Stop"][0]["hooks"][0]["command"], "notify.sh");
        assert_eq!(v["hooks"]["Stop"].as_array().unwrap().len(), 2);
        assert_eq!(v["hooks"]["PreToolUse"][0]["matcher"], "Bash");
    }

    #[test]
    fn install_is_idempotent_and_updates_moved_exe() {
        let mut v = json!({});
        install(&mut v, &exe()).unwrap();
        let once = v.clone();
        install(&mut v, &exe()).unwrap();
        assert_eq!(v, once);

        install(&mut v, Path::new("/new/place/pitwall")).unwrap();
        assert_eq!(v["hooks"]["Stop"].as_array().unwrap().len(), 1);
        assert_eq!(
            v["hooks"]["Stop"][0]["hooks"][0]["command"],
            "\"/new/place/pitwall\" hook Stop"
        );
    }

    #[test]
    fn uninstall_removes_only_pitwall_entries() {
        let mut v = json!({
            "hooks": {
                "Stop": [{ "hooks": [{ "type": "command", "command": "notify.sh" }] }]
            }
        });
        let original = v.clone();
        install(&mut v, &exe()).unwrap();
        uninstall(&mut v).unwrap();
        assert_eq!(v, original);

        let mut only_ours = json!({ "model": "x" });
        install(&mut only_ours, &exe()).unwrap();
        uninstall(&mut only_ours).unwrap();
        assert_eq!(only_ours, json!({ "model": "x" }));
    }

    #[test]
    fn bad_shapes_are_rejected() {
        assert!(install(&mut json!([]), &exe()).is_err());
        assert!(install(&mut json!({"hooks": []}), &exe()).is_err());
        assert!(install(&mut json!({"hooks": {"Stop": {}}}), &exe()).is_err());
    }

    #[test]
    fn run_on_missing_file_creates_it_without_backup() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".claude").join("settings.json");
        let (r, out) = run_with(&opts(path.clone(), true, false), "");
        r.unwrap();
        assert!(!out.contains("Backup:"));
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert!(v["hooks"]["SessionStart"].is_array());
    }

    #[test]
    fn run_with_existing_hooks_backs_up_and_merges() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let original = "{\n  \"hooks\": {\n    \"Stop\": [{\"hooks\": [{\"type\": \"command\", \"command\": \"notify.sh\"}]}]\n  }\n}\n";
        std::fs::write(&path, original).unwrap();

        let (r, out) = run_with(&opts(path.clone(), false, false), "y\n");
        r.unwrap();
        assert!(out.contains("+"), "diff is shown");
        assert!(out.contains("Backup:"));

        let backups: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains(".pitwall-backup-"))
            .collect();
        assert_eq!(backups.len(), 1);
        assert_eq!(
            std::fs::read_to_string(backups[0].path()).unwrap(),
            original
        );

        let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(v["hooks"]["Stop"][0]["hooks"][0]["command"], "notify.sh");
        assert_eq!(v["hooks"]["Stop"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn run_without_confirmation_does_not_write() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, "{}").unwrap();
        let (r, out) = run_with(&opts(path.clone(), false, false), "\n");
        r.unwrap();
        assert!(out.contains("Cancelled"));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{}");
        assert_eq!(
            std::fs::read_dir(dir.path()).unwrap().count(),
            1,
            "no backup"
        );
    }

    #[test]
    fn run_on_broken_json_errors_and_does_not_write() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, "{ broken").unwrap();
        let (r, _) = run_with(&opts(path.clone(), true, false), "");
        assert!(matches!(r, Err(InitError::BadJson { .. })));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{ broken");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn run_twice_reports_no_changes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        run_with(&opts(path.clone(), true, false), "").0.unwrap();
        let (r, out) = run_with(&opts(path.clone(), true, false), "");
        r.unwrap();
        assert!(out.contains("No changes needed"));
    }

    #[test]
    fn run_uninstall_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, "{\n  \"model\": \"opus\"\n}\n").unwrap();
        run_with(&opts(path.clone(), true, false), "").0.unwrap();
        run_with(&opts(path.clone(), true, true), "").0.unwrap();
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(v, json!({ "model": "opus" }));
    }
}
