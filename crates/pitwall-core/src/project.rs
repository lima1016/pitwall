//! 프로젝트 등록과 설정 파일(`~/.pitwall/config.toml`).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::paths;

/// 프로젝트 식별자. 정규화한 경로에서 만든 고정 해시라 설정 파일에 따로 저장하지 않는다.
pub type ProjectId = String;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Project {
    pub id: ProjectId,
    pub name: String,
    pub path: PathBuf,
}

impl Project {
    pub fn new(name: String, path: PathBuf) -> Self {
        Project {
            id: project_id(&path),
            name,
            path,
        }
    }
}

/// FNV-1a 64비트. 빌드마다 바뀌지 않는 해시가 필요해서 직접 구현한다.
pub fn project_id(path: &Path) -> ProjectId {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in paths::compare_key(path).bytes() {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{hash:016x}")
}

pub const DEFAULT_PORT: u16 = 7311;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub port: u16,
    pub projects: Vec<ProjectEntry>,
    pub notifications: Notifications,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            port: DEFAULT_PORT,
            projects: Vec::new(),
            notifications: Notifications::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectEntry {
    pub name: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Notifications {
    pub needs_permission: bool,
    pub needs_input: bool,
    pub done: bool,
}

impl Default for Notifications {
    fn default() -> Self {
        Notifications {
            needs_permission: true,
            needs_input: true,
            done: false,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to read or write {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("invalid config file {path}: {source}")]
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
    #[error("failed to serialize config: {0}")]
    Serialize(#[from] toml::ser::Error),
}

impl Config {
    pub fn default_path() -> PathBuf {
        paths::pitwall_home().join("config.toml")
    }

    /// 파일이 없으면 기본값.
    pub fn load(path: &Path) -> Result<Config, ConfigError> {
        match std::fs::read_to_string(path) {
            Ok(text) => toml::from_str(&text).map_err(|source| ConfigError::Parse {
                path: path.to_owned(),
                source,
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
            Err(source) => Err(ConfigError::Io {
                path: path.to_owned(),
                source,
            }),
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        let io = |source| ConfigError::Io {
            path: path.to_owned(),
            source,
        };
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(io)?;
        }
        std::fs::write(path, toml::to_string_pretty(self)?).map_err(io)
    }

    pub fn projects(&self) -> Vec<Project> {
        self.projects
            .iter()
            .map(|p| Project::new(p.name.clone(), p.path.clone()))
            .collect()
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AddProjectError {
    #[error("folder does not exist: {0}")]
    NotFound(PathBuf),
    #[error("project already registered: {0}")]
    Duplicate(PathBuf),
}

impl Config {
    /// 프로젝트를 등록한다. 경로는 정규화해서 저장한다.
    pub fn add_project(&mut self, path: &Path) -> Result<Project, AddProjectError> {
        if !path.is_dir() {
            return Err(AddProjectError::NotFound(path.to_owned()));
        }
        let path = paths::normalize(path);
        let key = paths::compare_key(&path);
        if self
            .projects
            .iter()
            .any(|p| paths::compare_key(&p.path) == key)
        {
            return Err(AddProjectError::Duplicate(path));
        }
        let entry = ProjectEntry {
            name: paths::display_name(&path),
            path,
        };
        self.projects.push(entry.clone());
        Ok(Project::new(entry.name, entry.path))
    }

    /// 등록 해제. 파일은 건드리지 않는다. 있었으면 true.
    pub fn remove_project(&mut self, id: &str) -> bool {
        let before = self.projects.len();
        self.projects.retain(|p| project_id(&p.path) != id);
        self.projects.len() != before
    }
}

/// 프로젝트와 그 worktree 경로들. cwd 매칭에 쓴다.
#[derive(Debug, Clone)]
pub struct ProjectRoots {
    pub project: Project,
    pub roots: Vec<PathBuf>,
}

/// `cwd`가 속한 프로젝트를 찾는다. 여러 개가 맞으면 가장 깊은(구체적인) 경로가 이긴다.
pub fn match_project<'a>(cwd: &Path, candidates: &'a [ProjectRoots]) -> Option<&'a Project> {
    let cwd_key = paths::compare_key(cwd);
    candidates
        .iter()
        .flat_map(|c| c.roots.iter().map(move |r| (c, paths::compare_key(r))))
        .filter(|(_, root)| {
            cwd_key == *root
                || cwd_key
                    .strip_prefix(root.as_str())
                    .is_some_and(|rest| rest.starts_with('/') || root.ends_with('/'))
        })
        .max_by_key(|(_, root)| root.len())
        .map(|(c, _)| &c.project)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_round_trip_and_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let cfg = Config::load(&path).unwrap();
        assert_eq!(cfg, Config::default());
        assert_eq!(cfg.port, 7311);

        let mut cfg = cfg;
        cfg.add_project(dir.path()).unwrap();
        cfg.save(&path).unwrap();
        assert_eq!(Config::load(&path).unwrap(), cfg);
    }

    #[test]
    fn config_example_from_design_parses() {
        let cfg: Config = toml::from_str(
            r#"
port = 7311

[[projects]]
name = "cdp-backend"
path = "C:/work/cdp-backend"

[notifications]
needs_permission = true
needs_input = true
done = false
"#,
        )
        .unwrap();
        assert_eq!(cfg.projects[0].name, "cdp-backend");
    }

    #[test]
    fn broken_config_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "port = [").unwrap();
        assert!(matches!(
            Config::load(&path),
            Err(ConfigError::Parse { .. })
        ));
    }

    #[test]
    fn add_project_rejects_missing_and_duplicate() {
        let dir = tempfile::tempdir().unwrap();
        let mut cfg = Config::default();
        assert!(matches!(
            cfg.add_project(&dir.path().join("nope")),
            Err(AddProjectError::NotFound(_))
        ));
        let p = cfg.add_project(dir.path()).unwrap();
        assert!(matches!(
            cfg.add_project(dir.path()),
            Err(AddProjectError::Duplicate(_))
        ));
        assert!(cfg.remove_project(&p.id));
        assert!(!cfg.remove_project(&p.id));
    }

    #[test]
    fn project_id_is_stable_for_same_location() {
        let dir = tempfile::tempdir().unwrap();
        let a = project_id(dir.path());
        let b = project_id(&dir.path().join("."));
        assert_eq!(a, b);
        assert_eq!(a.len(), 16);
    }

    #[test]
    fn match_prefers_deepest_root() {
        let dir = tempfile::tempdir().unwrap();
        let main = dir.path().join("app");
        let wt = dir.path().join("app-feature");
        let nested = main.join("libs").join("inner");
        for d in [&main, &wt, &nested] {
            std::fs::create_dir_all(d).unwrap();
        }
        let app = Project::new("app".into(), main.clone());
        let inner = Project::new("inner".into(), nested.clone());
        let candidates = vec![
            ProjectRoots {
                project: app.clone(),
                roots: vec![main.clone(), wt.clone()],
            },
            ProjectRoots {
                project: inner.clone(),
                roots: vec![nested.clone()],
            },
        ];
        assert_eq!(match_project(&main.join("src"), &candidates), Some(&app));
        assert_eq!(match_project(&wt, &candidates), Some(&app));
        assert_eq!(match_project(&nested, &candidates), Some(&inner));
        assert_eq!(match_project(dir.path(), &candidates), None);
    }
}
