use crate::config::Config;
use crate::git::{is_ancestor, GitRepo};
use crate::status::is_worktree_dirty;
use crate::ui::{print_info, print_success, print_warning};
use crate::worktree::{list_worktrees, Worktree};
use anyhow::{Context, Result};
use dialoguer::Confirm;
use is_terminal::IsTerminal;
use std::process::Command;

pub struct CleanOptions {
    pub merged: bool,
    pub dry_run: bool,
    pub yes: bool,
}

pub fn execute(repo: &GitRepo, opts: CleanOptions) -> Result<()> {
    let config = Config::load(repo)?;
    let worktrees = list_worktrees(repo)?;
    let current_path = std::env::current_dir().unwrap_or_default();

    let candidates: Vec<&Worktree> = worktrees
        .iter()
        .filter(|wt| {
            if wt.path == current_path {
                return false;
            }

            if wt.is_main_worktree(repo) {
                return false;
            }

            if wt.detached {
                return false;
            }

            if let Some(branch) = &wt.branch_short {
                if branch == &config.base {
                    return false;
                }
            }

            if opts.merged {
                if let Some(branch) = &wt.branch_short {
                    matches!(is_ancestor(repo, branch, &config.base), Ok(true))
                } else {
                    false
                }
            } else {
                true
            }
        })
        .collect();

    if candidates.is_empty() {
        print_info("No worktrees to clean up.");
        return Ok(());
    }

    println!("Worktrees to remove:");
    for wt in &candidates {
        let dirty = if is_worktree_dirty(wt) { " (dirty)" } else { "" };
        println!("  - {}{}", wt.name(), dirty);
    }

    if opts.dry_run {
        print_info("Dry run - no worktrees removed.");
        return Ok(());
    }

    let dirty_count = candidates.iter().filter(|wt| is_worktree_dirty(wt)).count();
    if dirty_count > 0 {
        print_warning(&format!(
            "{} worktree(s) have uncommitted changes and will be skipped.",
            dirty_count
        ));
    }

    let clean_candidates: Vec<&&Worktree> = candidates
        .iter()
        .filter(|wt| !is_worktree_dirty(wt))
        .collect();

    if clean_candidates.is_empty() {
        print_info("All candidate worktrees have uncommitted changes. Nothing to remove.");
        return Ok(());
    }

    if !opts.yes && std::io::stdin().is_terminal() {
        let confirm = Confirm::new()
            .with_prompt(format!(
                "Remove {} worktree(s)?",
                clean_candidates.len()
            ))
            .default(false)
            .interact()?;

        if !confirm {
            eprintln!("Aborted.");
            return Ok(());
        }
    } else if !opts.yes {
        print_warning("Non-interactive mode requires --yes flag for destructive operations.");
        std::process::exit(1);
    }

    let mut removed = 0;
    for wt in clean_candidates {
        let output = Command::new("git")
            .current_dir(&repo.root)
            .args(["worktree", "remove", wt.path.to_str().unwrap()])
            .output()
            .context("Failed to remove worktree")?;

        if output.status.success() {
            print_success(&format!("Removed worktree '{}'", wt.name()));
            removed += 1;
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            print_warning(&format!(
                "Failed to remove '{}': {}",
                wt.name(),
                stderr.trim()
            ));
        }
    }

    print_info(&format!("Cleaned up {} worktree(s).", removed));

    Ok(())
}
