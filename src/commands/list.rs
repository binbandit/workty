use crate::git::GitRepo;
use crate::status::get_all_statuses;
use crate::ui::{print_worktree_list, UiOptions};
use crate::worktree::{list_worktrees, Worktree};
use crate::status::WorktreeStatus;
use anyhow::Result;
use std::path::PathBuf;

pub fn execute(repo: &GitRepo, opts: &UiOptions) -> Result<()> {
    let worktrees = list_worktrees(repo)?;
    let statuses = get_all_statuses(repo, &worktrees);

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
