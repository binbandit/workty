use crate::git::GitRepo;
use crate::status::is_worktree_dirty;
use crate::ui::{print_error, print_success, print_warning};
use crate::worktree::{find_worktree, list_worktrees};
use anyhow::{bail, Context, Result};
use dialoguer::Confirm;
use is_terminal::IsTerminal;
use std::process::Command;

pub struct RmOptions {
    pub name: String,
    pub force: bool,
    pub delete_branch: bool,
    pub yes: bool,
}

pub fn execute(repo: &GitRepo, opts: RmOptions) -> Result<()> {
    let worktrees = list_worktrees(repo)?;

    let wt = find_worktree(&worktrees, &opts.name).ok_or_else(|| {
        anyhow::anyhow!(
            "Worktree '{}' not found. Use `git workty list` to see available worktrees.",
            opts.name
        )
    })?;

    let current_path = std::env::current_dir().unwrap_or_default();
    if wt.path == current_path {
        print_error(
            "Cannot remove the current worktree",
            Some("Change to a different worktree first with `wcd` or `git workty go`."),
        );
        std::process::exit(1);
    }

    if wt.is_main_worktree(repo) {
        print_error(
            "Cannot remove the main worktree",
            Some("The main worktree is the original repository clone."),
        );
        std::process::exit(1);
    }

    let is_dirty = is_worktree_dirty(wt);
    if is_dirty && !opts.force {
        print_error(
            &format!(
                "Worktree '{}' has uncommitted changes",
                opts.name
            ),
            Some("Use --force to remove anyway, or commit/stash changes first."),
        );
        std::process::exit(1);
    }

    if is_dirty {
        print_warning(&format!(
            "Worktree '{}' has uncommitted changes (--force specified)",
            opts.name
        ));
    }

    if !opts.yes && std::io::stdin().is_terminal() {
        let confirm = Confirm::new()
            .with_prompt(format!(
                "Remove worktree '{}'{}?",
                opts.name,
                if opts.delete_branch { " and its branch" } else { "" }
            ))
            .default(false)
            .interact()?;

        if !confirm {
            eprintln!("Aborted.");
            std::process::exit(1);
        }
    }

    let branch_name = wt.branch_short.clone();
    let wt_path = wt.path.clone();

    let mut args = vec!["worktree", "remove"];
    if opts.force {
        args.push("--force");
    }
    args.push(wt_path.to_str().unwrap());

    let output = Command::new("git")
        .current_dir(&repo.root)
        .args(&args)
        .output()
        .context("Failed to remove worktree")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("Failed to remove worktree: {}", stderr.trim());
    }

    print_success(&format!("Removed worktree '{}'", opts.name));

    if opts.delete_branch {
        if let Some(branch) = branch_name {
            let output = Command::new("git")
                .current_dir(&repo.root)
                .args(["branch", "-d", &branch])
                .output();

            match output {
                Ok(o) if o.status.success() => {
                    print_success(&format!("Deleted branch '{}'", branch));
                }
                Ok(o) => {
                    let stderr = String::from_utf8_lossy(&o.stderr);
                    print_warning(&format!(
                        "Could not delete branch '{}': {}",
                        branch,
                        stderr.trim()
                    ));
                    eprintln!("Hint: Use `git branch -D {}` to force delete.", branch);
                }
                Err(e) => {
                    print_warning(&format!("Could not delete branch '{}': {}", branch, e));
                }
            }
        }
    }

    Ok(())
}
