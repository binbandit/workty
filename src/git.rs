use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct GitRepo {
    pub repo: git2::Repository,
    pub root: PathBuf,
    pub common_dir: PathBuf,
}

impl GitRepo {
    pub fn discover(start_path: Option<&Path>) -> Result<Self> {
        let working_directory = match start_path {
            Some(p) => PathBuf::from(p),
            None => std::env::current_dir().context("Failed to determine current directory")?,
        };

        let repo = git2::Repository::discover(&working_directory)
            .context("Failed to discover git repository")?;

        let root = repo
            .workdir()
            .map(PathBuf::from)
            .unwrap_or_else(|| repo.path().to_path_buf());

        // The common dir is shared across all worktrees (the main repo's .git),
        // unlike path() which points at .git/worktrees/<name> for linked worktrees.
        let common_dir = repo.commondir().to_path_buf();

        Ok(Self {
            repo,
            root: root.canonicalize().unwrap_or(root),
            common_dir: common_dir.canonicalize().unwrap_or(common_dir),
        })
    }

    pub fn origin_url(&self) -> Option<String> {
        self.repo
            .find_remote("origin")
            .ok()
            .and_then(|remote| remote.url().map(|s| s.to_string()))
    }

    pub fn default_branch(&self) -> Option<String> {
        ["main", "master"]
            .into_iter()
            .find(|branch| self.branch_exists(branch))
            .map(|branch| branch.to_string())
    }

    pub fn branch_exists(&self, branch_name: &str) -> bool {
        self.repo
            .find_branch(branch_name, git2::BranchType::Local)
            .is_ok()
    }

    pub fn is_merged(&self, branch: &str, base: &str) -> Result<bool> {
        let branch_oid = match self.repo.revparse_single(branch) {
            Ok(obj) => obj.id(),
            Err(_) => return Ok(false),
        };

        // Check against the local base and its remote counterpart (origin/<base>)
        for base_ref in [base.to_string(), format!("origin/{}", base)] {
            if let Ok(base_obj) = self.repo.revparse_single(&base_ref) {
                if let Ok(true) = self.repo.graph_descendant_of(base_obj.id(), branch_oid) {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }
}

pub fn is_git_installed() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn is_in_git_repo(path: &Path) -> bool {
    Command::new("git")
        .current_dir(path)
        .args(["rev-parse", "--git-dir"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
