use crate::git::GitRepo;
use crate::status::is_worktree_dirty;
use crate::ui::{print_success, print_warning};
use crate::worktree::{find_worktree, list_worktrees, same_path};
use anyhow::{Context, Result, bail};
use dialoguer::Confirm;
use std::io::IsTerminal;
use std::process::Command;

#[derive(Debug, clap::Args)]
pub struct RmArgs {
    /// Worktree name to remove
    pub name: String,

    /// Remove even if worktree has uncommitted changes
    #[arg(long, short = 'f')]
    pub force: bool,

    /// Also delete the branch after removing worktree
    #[arg(long, short = 'd')]
    pub delete_branch: bool,
}

pub fn execute(repo: &GitRepo, opts: RmArgs, yes: bool) -> Result<()> {
    let worktrees = list_worktrees(repo)?;

    let wt = find_worktree(&worktrees, &opts.name).ok_or_else(|| {
        anyhow::anyhow!(
            "Worktree '{}' not found. Use `git workty list` to see available worktrees.",
            opts.name
        )
    })?;

    let current_path = std::env::current_dir().context("Failed to get current directory")?;
    if same_path(&wt.path, &current_path) {
        bail!("Cannot remove the current worktree. Change to a different worktree first.");
    }

    if wt.is_main_worktree(repo) {
        bail!("Cannot remove the main worktree (original repository clone)");
    }

    let is_dirty = is_worktree_dirty(wt);
    if is_dirty && !opts.force {
        bail!(
            "Worktree '{}' has uncommitted changes. Use --force to remove anyway.",
            opts.name
        );
    }

    if is_dirty {
        print_warning(&format!(
            "Worktree '{}' has uncommitted changes (--force specified)",
            opts.name
        ));
    }

    if !yes && std::io::stdin().is_terminal() {
        let confirm = Confirm::new()
            .with_prompt(format!(
                "Remove worktree '{}'{}?",
                opts.name,
                if opts.delete_branch {
                    " and its branch"
                } else {
                    ""
                }
            ))
            .default(false)
            .interact()?;

        if !confirm {
            bail!("Aborted");
        }
    }

    let branch_name = wt.branch_short.clone();
    let wt_path = wt.path.clone();
    let path_str = wt_path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("Path contains invalid UTF-8: {:?}", wt_path))?;

    let mut args = vec!["worktree", "remove"];
    if opts.force {
        args.push("--force");
    }
    args.push(path_str);

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
