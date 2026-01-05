use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct GitRepo {
    pub root: PathBuf,
    pub common_dir: PathBuf,
}

impl GitRepo {
    pub fn discover(start_path: Option<&Path>) -> Result<Self> {
        let cwd = start_path
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

        let root = git_rev_parse(&cwd, &["--show-toplevel"])?;
        let common_dir = git_rev_parse(&cwd, &["--git-common-dir"])?;

        let root = PathBuf::from(root.trim());
        let common_dir_str = common_dir.trim();

        let common_dir = if Path::new(common_dir_str).is_absolute() {
            PathBuf::from(common_dir_str)
        } else {
            root.join(common_dir_str)
        };

        Ok(Self {
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
        self.run_git(&["remote", "get-url", "origin"]).ok().map(|s| s.trim().to_string())
    }
}

fn git_rev_parse(cwd: &Path, args: &[&str]) -> Result<String> {
    let mut cmd_args = vec!["rev-parse"];
    cmd_args.extend(args);
    run_git_command(Some(cwd), &cmd_args)
}

pub fn run_git_command(cwd: Option<&Path>, args: &[&str]) -> Result<String> {
    let mut cmd = Command::new("git");
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    cmd.args(args);

    let output = cmd.output().context("Failed to execute git command")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "git {} failed: {}",
            args.first().unwrap_or(&""),
            stderr.trim()
        );
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

pub fn branch_exists(repo: &GitRepo, branch: &str) -> bool {
    repo.run_git(&["rev-parse", "--verify", &format!("refs/heads/{}", branch)])
        .is_ok()
}

pub fn is_ancestor(repo: &GitRepo, ancestor: &str, descendant: &str) -> Result<bool> {
    let result = Command::new("git")
        .current_dir(&repo.root)
        .args(["merge-base", "--is-ancestor", ancestor, descendant])
        .output()
        .context("Failed to check ancestry")?;
    Ok(result.status.success())
}
