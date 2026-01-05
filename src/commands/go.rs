use crate::git::GitRepo;
use crate::ui::print_error;
use crate::worktree::{find_worktree, list_worktrees};
use anyhow::Result;

pub fn execute(repo: &GitRepo, name: &str) -> Result<()> {
    let worktrees = list_worktrees(repo)?;

    if let Some(wt) = find_worktree(&worktrees, name) {
        println!("{}", wt.path.display());
        Ok(())
    } else {
        print_error(
            &format!("Worktree '{}' not found", name),
            Some("Use `git workty pick` to interactively select a worktree, or `git workty list` to see all worktrees."),
        );
        std::process::exit(1);
    }
}
