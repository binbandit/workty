use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

use std::sync::Mutex;

pub struct GitRepo {
    pub repo: Mutex<git2::Repository>,
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
            repo: Mutex::new(repo),
            root: root.canonicalize().unwrap_or(root),
            common_dir: common_dir.canonicalize().unwrap_or(common_dir),
        })
    }

    pub fn run_git(&self, args: &[&str]) -> Result<String> {
        run_git_command(Some(&self.root), args)
    }

    #[allow(dead_code)]
    pub fn run_git_in(&self, worktree_path: &Path, args: &[&str]) -> Result<String> {
        run_git_command(Some(worktree_path), args)
    }

    pub fn origin_url(&self) -> Option<String> {
        self.repo
            .lock()
            .ok()?
            .find_remote("origin")
            .ok()
            .and_then(|remote| remote.url().map(|s| s.to_string()))
    }

    pub fn default_branch(&self) -> Option<String> {
        const FALLBACK_BRANCHES: [&str; 2] = ["main", "master"];
        let repo = self.repo.lock().ok()?;

        for branch in FALLBACK_BRANCHES {
            if repo.find_branch(branch, git2::BranchType::Local).is_ok() {
                return Some(branch.to_string());
            }
        }
        None
    }

    pub fn branch_exists(&self, branch_name: &str) -> bool {
        let repo = match self.repo.lock() {
            Ok(r) => r,
            Err(_) => return false,
        };
        let exists = repo
            .find_branch(branch_name, git2::BranchType::Local)
            .is_ok();
        exists
    }

    pub fn is_merged(&self, branch: &str, base: &str) -> Result<bool> {
        let repo = self
            .repo
            .lock()
            .map_err(|_| anyhow::anyhow!("Failed to lock repository"))?;

        let branch_oid = match repo.revparse_single(branch) {
            Ok(obj) => obj.id(),
            Err(_) => return Ok(false),
        };

        // 1. Check against local base
        if let Ok(base_obj) = repo.revparse_single(base) {
            if let Ok(true) = repo.graph_descendant_of(base_obj.id(), branch_oid) {
                return Ok(true);
            }
        }

        // 2. Check against remote base (e.g. origin/main)
        let remote_base = format!("origin/{}", base);
        if let Ok(remote_base_obj) = repo.revparse_single(&remote_base) {
            if let Ok(true) = repo.graph_descendant_of(remote_base_obj.id(), branch_oid) {
                return Ok(true);
            }
        }

        Ok(false)
    }
}

pub fn run_git_command(working_directory: Option<&Path>, args: &[&str]) -> Result<String> {
    let mut cmd = Command::new("git");
    if let Some(directory) = working_directory {
        cmd.current_dir(directory);
    }
    cmd.args(args);

    let output = cmd.output().context("Failed to execute git command")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git {} failed: {}", args.join(" "), stderr.trim());
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
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
