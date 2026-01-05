use crate::config::Config;
use crate::git::{branch_exists, GitRepo};
use crate::ui::{print_info, print_success};
use crate::worktree::{list_worktrees, slug_from_branch};
use anyhow::{bail, Context, Result};
use std::path::PathBuf;
use std::process::Command;

pub struct NewOptions {
    pub name: String,
    pub from: Option<String>,
    pub path: Option<PathBuf>,
    pub print_path: bool,
    pub open: bool,
}

pub fn execute(repo: &GitRepo, opts: NewOptions) -> Result<()> {
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

    let base = opts.from.unwrap_or_else(|| config.base.clone());

    if let Some(parent) = worktree_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    let branch_already_exists = branch_exists(repo, branch_name);

    if branch_already_exists {
        print_info(&format!(
            "Using existing branch '{}'",
            branch_name
        ));

        let output = Command::new("git")
            .current_dir(&repo.root)
            .args([
                "worktree",
                "add",
                worktree_path.to_str().unwrap(),
                branch_name,
            ])
            .output()
            .context("Failed to create worktree")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("Failed to create worktree: {}", stderr.trim());
        }
    } else {
        print_info(&format!(
            "Creating new branch '{}' from '{}'",
            branch_name, base
        ));

        let output = Command::new("git")
            .current_dir(&repo.root)
            .args([
                "worktree",
                "add",
                "-b",
                branch_name,
                worktree_path.to_str().unwrap(),
                &base,
            ])
            .output()
            .context("Failed to create worktree")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("Failed to create worktree: {}", stderr.trim());
        }
    }

    if opts.print_path {
        println!("{}", worktree_path.display());
    } else {
        print_success(&format!(
            "Created worktree at {}",
            worktree_path.display()
        ));
    }

    if opts.open {
        if let Some(open_cmd) = &config.open_cmd {
            let _ = Command::new(open_cmd)
                .arg(&worktree_path)
                .spawn();
        }
    }

    Ok(())
}
