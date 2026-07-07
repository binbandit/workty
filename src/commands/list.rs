use crate::git::GitRepo;
use crate::status::WorktreeStatus;
use crate::status::{get_all_statuses, get_all_statuses_fast};
use crate::ui::{UiOptions, print_worktree_list};
use crate::worktree::{Worktree, list_worktrees};
use anyhow::Result;
use std::path::PathBuf;

#[derive(Debug, Default, clap::Args)]
pub struct ListArgs {
    /// Skip dirty file check for faster output
    #[arg(long)]
    pub fast: bool,
}

pub fn execute(repo: &GitRepo, opts: &UiOptions, args: ListArgs) -> Result<()> {
    let worktrees = list_worktrees(repo)?;
    let statuses = if args.fast {
        get_all_statuses_fast(repo, &worktrees)
    } else {
        get_all_statuses(repo, &worktrees)
    };

    let current_path = std::env::current_dir().unwrap_or_else(|_| PathBuf::new());

    let sorted = sort_worktrees(statuses, &current_path);

    print_worktree_list(repo, &sorted, &current_path, opts);

    Ok(())
}

fn sort_worktrees(
    mut worktrees: Vec<(Worktree, WorktreeStatus)>,
    current_path: &PathBuf,
) -> Vec<(Worktree, WorktreeStatus)> {
    worktrees.sort_by(|(a, status_a), (b, status_b)| {
        let a_is_current = a.path == *current_path;
        let b_is_current = b.path == *current_path;

        if a_is_current != b_is_current {
            return b_is_current.cmp(&a_is_current);
        }

        let a_dirty = status_a.is_dirty();
        let b_dirty = status_b.is_dirty();

        if a_dirty != b_dirty {
            return b_dirty.cmp(&a_dirty);
        }

        a.name().cmp(b.name())
    });

    worktrees
}
