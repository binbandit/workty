use crate::git::GitRepo;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

const CONFIG_FILENAME: &str = "workty.toml";

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
            base: "main".to_string(),
            root: "~/.workty/{repo}-{id}".to_string(),
            layout: "flat".to_string(),
            open_cmd: None,
        }
    }
}

impl Config {
    pub fn load(repo: &GitRepo) -> Result<Self> {
        let path = config_path(repo);
        if path.exists() {
            let contents = std::fs::read_to_string(&path)
                .with_context(|| format!("Failed to read config from {}", path.display()))?;
            toml::from_str(&contents)
                .with_context(|| format!("Failed to parse config from {}", path.display()))
        } else {
            Ok(Self::default())
        }
    }

    #[allow(dead_code)]
    pub fn save(&self, repo: &GitRepo) -> Result<()> {
        let path = config_path(repo);
        let contents = toml::to_string_pretty(self).context("Failed to serialize config")?;
        std::fs::write(&path, contents)
            .with_context(|| format!("Failed to write config to {}", path.display()))
    }

    pub fn workspace_root(&self, repo: &GitRepo) -> PathBuf {
        let repo_name = repo
            .root
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("repo");

        let id = compute_repo_id(repo);

        let expanded = self
            .root
            .replace("{repo}", repo_name)
            .replace("{id}", &id);

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
    hex::encode(&result[..4])
}

fn normalize_url(url: &str) -> String {
    url.trim()
        .trim_end_matches('/')
        .trim_end_matches(".git")
        .to_lowercase()
}

fn expand_tilde(path: &str) -> PathBuf {
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
