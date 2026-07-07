use crate::git::GitRepo;
use crate::worktree::{find_worktree, list_worktrees};
use anyhow::{Result, bail};

#[derive(Debug, clap::Args)]
pub struct GoArgs {
    /// Worktree name (branch name or directory name)
    pub name: String,
}

pub fn execute(repo: &GitRepo, name: &str) -> Result<()> {
    let worktrees = list_worktrees(repo)?;

    if let Some(wt) = find_worktree(&worktrees, name) {
        println!("{}", wt.path.display());
        Ok(())
    } else {
        bail!(
            "Worktree '{}' not found. Use `git workty list` to see available worktrees.",
            name
        );
    }
}
