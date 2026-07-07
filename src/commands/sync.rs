use crate::git::GitRepo;
use crate::status::get_all_statuses;
use crate::ui;
use crate::worktree::list_worktrees;
use anyhow::{Context, Result};
use std::process::Command;

pub struct SyncOptions {
    pub dry_run: bool,
    pub fetch: bool,
}

pub fn execute(repo: &GitRepo, opts: SyncOptions) -> Result<()> {
    // Optionally fetch first
    if opts.fetch {
        ui::print_info("Fetching from origin...");
        let output = Command::new("git")
            .current_dir(&repo.root)
            .args(["fetch", "--prune", "origin"])
            .output()
            .context("Failed to fetch")?;

        if !output.status.success() {
            ui::print_warning("Failed to fetch from origin");
        }
    }

    let worktrees = list_worktrees(repo)?;
    let statuses = get_all_statuses(repo, &worktrees);

    let mut synced = 0;
    let mut skipped_dirty = 0;
    let mut skipped_no_upstream = 0;
    let mut failed = 0;

    for (wt, status) in &statuses {
        // Skip main worktree (usually you don't want to auto-rebase main)
        if wt.is_main_worktree(repo) {
            continue;
        }

        // Skip detached HEAD
        if wt.detached {
            continue;
        }

        // Skip if no upstream
        if status.upstream.is_none() {
            skipped_no_upstream += 1;
            continue;
        }

        // Skip if nothing to sync
        let behind = status.behind.unwrap_or(0);
        if behind == 0 {
            continue;
        }

        let branch_name = wt.name();

        // Skip if dirty (already computed in the status batch)
        if status.is_dirty() {
            if !opts.dry_run {
                ui::print_warning(&format!("{}: skipped (dirty)", branch_name));
            }
            skipped_dirty += 1;
            continue;
        }

        if opts.dry_run {
            ui::print_info(&format!(
                "{}: would rebase ({} commits behind)",
                branch_name, behind
            ));
            synced += 1;
            continue;
        }

        // Perform the rebase
        ui::print_info(&format!("{}: rebasing...", branch_name));

        let output = Command::new("git")
            .current_dir(&wt.path)
            .args(["rebase"])
            .output()
            .context("Failed to run git rebase")?;

        if output.status.success() {
            ui::print_success(&format!("{}: rebased", branch_name));
            synced += 1;
        } else {
            // Abort the failed rebase
            let _ = Command::new("git")
                .current_dir(&wt.path)
                .args(["rebase", "--abort"])
                .output();

            ui::print_warning(&format!("{}: rebase failed, aborted", branch_name));
            failed += 1;
        }
    }

    // Summary
    println!();
    if opts.dry_run {
        ui::print_info(&format!("Would sync {} worktree(s)", synced));
    } else {
        ui::print_info(&format!(
            "Synced: {}, Skipped (dirty): {}, Skipped (no upstream): {}, Failed: {}",
            synced, skipped_dirty, skipped_no_upstream, failed
        ));
    }

    Ok(())
}
