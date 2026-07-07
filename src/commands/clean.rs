use crate::config::Config;
use crate::git::GitRepo;
use crate::status::{get_all_statuses, is_worktree_dirty};
use crate::ui::{print_info, print_success, print_warning};
use crate::worktree::{list_worktrees, same_path, Worktree};
use anyhow::{bail, Context, Result};
use dialoguer::Confirm;
use std::io::IsTerminal;
use std::process::Command;

pub struct CleanOptions {
    pub merged: bool,
    pub gone: bool,
    pub stale_days: Option<u32>,
    pub dry_run: bool,
    pub yes: bool,
}

pub fn execute(repo: &GitRepo, opts: CleanOptions) -> Result<()> {
    let config = Config::load(repo)?;
    let worktrees = list_worktrees(repo)?;
    let current_path = std::env::current_dir().unwrap_or_default();

    // Get statuses if we need them for --gone or --stale
    let statuses = if opts.gone || opts.stale_days.is_some() {
        Some(get_all_statuses(repo, &worktrees))
    } else {
        None
    };

    // Helper to find status for a worktree
    let get_status = |wt: &Worktree| {
        statuses.as_ref().and_then(|s| {
            s.iter()
                .find(|(w, _)| w.path == wt.path)
                .map(|(_, status)| status)
        })
    };

    let has_filter = opts.merged || opts.gone || opts.stale_days.is_some();

    if !has_filter {
        print_info("No filter specified. Use one of:");
        println!("  --merged      Remove worktrees whose branches are merged into base");
        println!("  --gone        Remove worktrees whose upstream branch was deleted");
        println!("  --stale N     Remove worktrees not touched in N days");
        println!("\nAdd --dry-run to preview what would be removed.");
        return Ok(());
    }

    let candidates: Vec<&Worktree> = worktrees
        .iter()
        .filter(|wt| {
            if same_path(&wt.path, &current_path) {
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

            // Check --merged
            if opts.merged {
                if let Some(branch) = &wt.branch_short {
                    if matches!(repo.is_merged(branch, &config.base), Ok(true)) {
                        return true;
                    }
                }
            }

            // Check --gone (upstream branch deleted)
            if opts.gone {
                if let Some(status) = get_status(wt) {
                    if status.upstream_gone {
                        return true;
                    }
                }
            }

            // Check --stale (not touched in X days)
            if let Some(days) = opts.stale_days {
                if let Some(status) = get_status(wt) {
                    if let Some(seconds) = status.last_commit_time {
                        let stale_seconds = (days as i64) * 24 * 60 * 60;
                        if seconds > stale_seconds {
                            return true;
                        }
                    }
                }
            }

            false
        })
        .collect();

    if candidates.is_empty() {
        print_info("No worktrees to clean up.");
        return Ok(());
    }

    // Compute dirty status once per candidate to avoid redundant checks
    let candidates_with_dirty: Vec<(&Worktree, bool)> = candidates
        .into_iter()
        .map(|wt| {
            let is_dirty = is_worktree_dirty(wt);
            (wt, is_dirty)
        })
        .collect();

    println!("Worktrees to remove:");
    for (wt, is_dirty) in &candidates_with_dirty {
        let dirty_str = if *is_dirty { " (dirty)" } else { "" };
        println!("  - {}{}", wt.name(), dirty_str);
    }

    if opts.dry_run {
        print_info("Dry run - no worktrees removed.");
        return Ok(());
    }

    let dirty_count = candidates_with_dirty.iter().filter(|(_, d)| *d).count();
    if dirty_count > 0 {
        print_warning(&format!(
            "{} worktree(s) have uncommitted changes and will be skipped.",
            dirty_count
        ));
    }

    let clean_candidates: Vec<&Worktree> = candidates_with_dirty
        .iter()
        .filter(|(_, is_dirty)| !is_dirty)
        .map(|(wt, _)| *wt)
        .collect();

    if clean_candidates.is_empty() {
        print_info("All candidate worktrees have uncommitted changes. Nothing to remove.");
        return Ok(());
    }

    if !opts.yes && std::io::stdin().is_terminal() {
        let confirm = Confirm::new()
            .with_prompt(format!("Remove {} worktree(s)?", clean_candidates.len()))
            .default(false)
            .interact()?;

        if !confirm {
            eprintln!("Aborted.");
            return Ok(());
        }
    } else if !opts.yes {
        bail!("Non-interactive mode requires --yes flag for destructive operations");
    }

    let mut removed = 0;
    for wt in clean_candidates {
        let path_str = wt
            .path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Path contains invalid UTF-8: {:?}", wt.path))?;

        let output = Command::new("git")
            .current_dir(&repo.root)
            .args(["worktree", "remove", path_str])
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
