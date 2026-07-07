use crate::config::Config;
use crate::git::GitRepo;
use crate::ui::{print_info, print_success};
use crate::worktree::{list_worktrees, slug_from_branch};
use anyhow::{Context, Result, bail};
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[derive(Debug, clap::Args)]
pub struct NewArgs {
    /// Branch name for the new workspace
    pub name: String,

    /// Base branch or commit to create from
    #[arg(long, short = 'f')]
    pub from: Option<String>,

    /// Custom path for the worktree
    #[arg(long, short = 'p')]
    pub path: Option<PathBuf>,

    /// Print only the created path to stdout
    #[arg(long)]
    pub print_path: bool,

    /// Open the worktree in configured editor
    #[arg(long, short = 'o')]
    pub open: bool,

    /// Skip fetching from remote before creating
    #[arg(long)]
    pub no_fetch: bool,

    /// Skip pushing to set upstream after creating
    #[arg(long)]
    pub no_push: bool,
}

pub fn execute(repo: &GitRepo, opts: NewArgs) -> Result<()> {
    let config = Config::load(repo)?;

    let branch_name = &opts.name;
    let slug = slug_from_branch(branch_name);

    let worktree_path = opts
        .path
        .unwrap_or_else(|| config.worktree_path(repo, &slug));

    if worktree_path.exists() {
        bail!(
            "Directory already exists: {}\nUse --path to specify a different location.",
            worktree_path.display()
        );
    }

    let existing = list_worktrees(repo)?;
    if let Some(existing_wt) = existing
        .iter()
        .find(|wt| wt.branch_short.as_deref() == Some(branch_name))
    {
        bail!(
            "Branch '{}' is already checked out at: {}\nUse `git workty go {}` to switch to it.",
            branch_name,
            existing_wt.path.display(),
            branch_name
        );
    }

    let mut base = opts.from.unwrap_or_else(|| config.base.clone());

    if let Some(parent) = worktree_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    let branch_already_exists = repo.branch_exists(branch_name);

    if branch_already_exists {
        print_info(&format!("Using existing branch '{}'", branch_name));

        let path_str = worktree_path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Path contains invalid UTF-8: {:?}", worktree_path))?;

        let output = Command::new("git")
            .current_dir(&repo.root)
            .args(["worktree", "add", path_str, branch_name])
            .output()
            .context("Failed to create worktree")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("Failed to create worktree: {}", stderr.trim());
        }
    } else {
        // Try to fetch upstream of base to ensure we are up to date
        if !opts.no_fetch && config.auto_fetch {
            if let Some(upstream) = base_upstream(repo, &base) {
                print_info(&format!("Fetching {} to ensure fresh start...", upstream));

                // upstream is likely "origin/main", we want to split to "origin" "main"
                if let Some((remote, branch)) = upstream.split_once('/') {
                    let _ = Command::new("git")
                        .current_dir(&repo.root)
                        .args(["fetch", remote, branch])
                        .output();

                    // Update base to use the upstream ref (e.g. origin/main)
                    // so we branch off the latest remote commit
                    base = upstream;
                }
            }
        }

        print_info(&format!(
            "Creating new branch '{}' from '{}'",
            branch_name, base
        ));

        let path_str = worktree_path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Path contains invalid UTF-8: {:?}", worktree_path))?;

        // Create the branch up front so the upstream push can run while the
        // worktree checkout does; those are the two slow steps (network and
        // disk), and neither depends on the other.
        let output = Command::new("git")
            .current_dir(&repo.root)
            .args(["branch", branch_name, &base])
            .output()
            .context("Failed to create branch")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("Failed to create branch: {}", stderr.trim());
        }

        let mut push = None;
        if !opts.no_push && config.auto_push && repo.origin_url().is_some() {
            print_info("Setting upstream...");
            push = Command::new("git")
                .current_dir(&repo.root)
                .args(["push", "-u", "origin", branch_name])
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn()
                .ok();
            if push.is_none() {
                print_info("Note: Could not run git push to set upstream");
            }
        }

        let output = Command::new("git")
            .current_dir(&repo.root)
            .args(["worktree", "add", path_str, branch_name])
            .output()
            .context("Failed to create worktree")?;

        if !output.status.success() {
            // Reap the in-flight push, then undo the branch we just created
            if let Some(child) = push {
                let _ = child.wait_with_output();
            }
            let _ = Command::new("git")
                .current_dir(&repo.root)
                .args(["branch", "-D", branch_name])
                .output();
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("Failed to create worktree: {}", stderr.trim());
        }

        if let Some(child) = push {
            match child.wait_with_output() {
                Ok(p) if p.status.success() => {
                    print_success("Upstream set successfully");
                }
                Ok(p) => {
                    let stderr = String::from_utf8_lossy(&p.stderr);
                    print_info(&format!("Note: Could not set upstream: {}", stderr.trim()));
                }
                Err(_) => {
                    print_info("Note: Could not run git push to set upstream");
                }
            }
        }
    }

    if opts.print_path {
        println!("{}", worktree_path.display());
    } else {
        print_success(&format!("Created worktree at {}", worktree_path.display()));
    }

    if opts.open {
        if let Some(open_cmd) = &config.open_cmd {
            let _ = Command::new(open_cmd).arg(&worktree_path).spawn();
        }
    }

    Ok(())
}

/// Short upstream name of a local branch (e.g. "origin/main"), if configured.
fn base_upstream(repo: &GitRepo, base: &str) -> Option<String> {
    let refname = format!("refs/heads/{}", base);
    let buf = repo.repo.branch_upstream_name(&refname).ok()?;
    let name = buf.as_str().ok()?;
    Some(
        name.strip_prefix("refs/remotes/")
            .unwrap_or(name)
            .to_string(),
    )
}
