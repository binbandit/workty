use crate::git::GitRepo;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

const CONFIG_FILENAME: &str = "workty.toml";
const DEFAULT_BASE: &str = "main";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub version: u32,
    pub base: String,
    pub root: String,
    pub layout: String,
    pub open_cmd: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: 1,
            base: DEFAULT_BASE.to_string(),
            root: "~/.workty/{repo}-{id}".to_string(),
            layout: "flat".to_string(),
            open_cmd: None,
        }
    }
}

impl Config {
    pub fn load(repo: &GitRepo) -> Result<Self> {
        let mut candidates = vec![
            // 1. Repo root
            repo.root.join(CONFIG_FILENAME),
            // 2. Git dir
            config_path(repo),
        ];

        // 3. User config dir (~/.config/workty/workty.toml)
        if let Some(config_dir) = dirs::config_dir() {
            candidates.push(config_dir.join("workty").join(CONFIG_FILENAME));
        }

        if let Some(home) = dirs::home_dir() {
            // 4. ~/.workty.toml
            candidates.push(home.join(format!(".{}", CONFIG_FILENAME)));
            // 5. ~/workty.toml
            candidates.push(home.join(CONFIG_FILENAME));
        }

        let mut config: Self = candidates
            .into_iter()
            .find(|path| path.exists())
            .map(|path| {
                let contents = std::fs::read_to_string(&path)
                    .with_context(|| format!("Failed to read config from {}", path.display()))?;
                toml::from_str(&contents)
                    .with_context(|| format!("Failed to parse config from {}", path.display()))
            })
            .transpose()?
            .unwrap_or_default();

        config.adjust_defaults(repo);

        Ok(config)
    }

    fn adjust_defaults(&mut self, repo: &GitRepo) {
        // If the base branch is the default one but it doesn't exist,
        // we try to detect the actual default branch (e.g. master, trunk, etc)
        if self.base == DEFAULT_BASE && !repo.branch_exists(DEFAULT_BASE) {
            if let Some(default) = repo.default_branch() {
                self.base = default;
            }
        }
    }

    pub fn workspace_root(&self, repo: &GitRepo) -> PathBuf {
        let repo_name = repo
            .root
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("repo");

        let id = compute_repo_id(repo);

        let expanded = self.root.replace("{repo}", repo_name).replace("{id}", &id);

        expand_tilde(&expanded)
    }

    pub fn worktree_path(&self, repo: &GitRepo, branch_slug: &str) -> PathBuf {
        let root = self.workspace_root(repo);
        root.join(branch_slug)
    }
}

pub fn config_path(repo: &GitRepo) -> PathBuf {
    repo.common_dir.join(CONFIG_FILENAME)
}

pub fn config_exists(repo: &GitRepo) -> bool {
    config_path(repo).exists()
}

fn compute_repo_id(repo: &GitRepo) -> String {
    let input = repo
        .origin_url()
        .unwrap_or_else(|| repo.common_dir.to_string_lossy().to_string());

    let normalized = normalize_url(&input);
    let mut hasher = Sha256::new();
    hasher.update(normalized.as_bytes());
    let result = hasher.finalize();
    // 8 lowercase hex chars; must stay stable, existing worktree paths embed it
    result[..4].iter().map(|b| format!("{:02x}", b)).collect()
}

fn normalize_url(url: &str) -> String {
    url.trim()
        .trim_end_matches('/')
        .trim_end_matches(".git")
        .to_lowercase()
}

fn expand_tilde(path: &str) -> PathBuf {
    if path == "~" {
        return dirs::home_dir().unwrap_or_else(|| PathBuf::from("~"));
    }

    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert_eq!(config.version, 1);
        assert_eq!(config.base, "main");
        assert_eq!(config.layout, "flat");
    }

    #[test]
    fn test_normalize_url() {
        assert_eq!(
            normalize_url("https://github.com/user/repo.git"),
            "https://github.com/user/repo"
        );
        assert_eq!(
            normalize_url("git@github.com:user/repo.git/"),
            "git@github.com:user/repo"
        );
    }

    #[test]
    fn test_expand_tilde() {
        // We can't easily verify the exact home dir path in a cross-platform way without dirs::home_dir
        // but we can check that it doesn't panic and returns something different than "~" if home exists
        if let Some(home) = dirs::home_dir() {
            assert_eq!(expand_tilde("~"), home);
            assert_eq!(expand_tilde("~/foo"), home.join("foo"));
        }

        assert_eq!(expand_tilde("/abs/path"), PathBuf::from("/abs/path"));
        assert_eq!(expand_tilde("rel/path"), PathBuf::from("rel/path"));
    }

    #[test]
    fn test_config_roundtrip() {
        let config = Config {
            version: 1,
            base: "develop".to_string(),
            root: "~/.worktrees/{repo}".to_string(),
            layout: "flat".to_string(),
            open_cmd: Some("code".to_string()),
        };

        let serialized = toml::to_string_pretty(&config).unwrap();
        let deserialized: Config = toml::from_str(&serialized).unwrap();

        assert_eq!(config.base, deserialized.base);
        assert_eq!(config.open_cmd, deserialized.open_cmd);
    }
}
