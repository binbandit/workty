use crate::config::Config;
use crate::gh::{checkout_pr, get_pr_branch, is_gh_authenticated, is_gh_installed};
use crate::git::GitRepo;
use crate::ui::{print_error, print_info, print_success};
use crate::worktree::{list_worktrees, slug_from_branch};
use anyhow::{bail, Context, Result};
use std::process::Command;

pub struct PrOptions {
    pub number: u32,
    pub print_path: bool,
    pub open: bool,
}

pub fn execute(repo: &GitRepo, opts: PrOptions) -> Result<()> {
    if !is_gh_installed() {
        print_error(
            "GitHub CLI (gh) is not installed",
            Some("Install it from https://cli.github.com/ to use PR features."),
        );
        std::process::exit(1);
    }

    if !is_gh_authenticated() {
        print_error(
            "GitHub CLI is not authenticated",
            Some("Run `gh auth login` to authenticate."),
        );
        std::process::exit(1);
    }

    let config = Config::load(repo)?;
    let pr_name = format!("pr-{}", opts.number);

    let worktrees = list_worktrees(repo)?;
    if let Some(existing) = worktrees.iter().find(|wt| {
        wt.branch_short.as_deref() == Some(&pr_name)
            || wt.path.file_name().and_then(|s| s.to_str()) == Some(&pr_name)
    }) {
        print_info(&format!(
            "PR #{} already has a worktree at {}",
            opts.number,
            existing.path.display()
        ));
        println!("{}", existing.path.display());
        return Ok(());
    }

    let branch_name = get_pr_branch(opts.number)?;
    print_info(&format!(
        "PR #{} uses branch '{}'",
        opts.number, branch_name
    ));

    let slug = slug_from_branch(&pr_name);
    let worktree_path = config.worktree_path(repo, &slug);

    if worktree_path.exists() {
        bail!(
            "Directory already exists: {}\nUse a different path or remove the existing directory.",
            worktree_path.display()
        );
    }

    if let Some(parent) = worktree_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    let output = Command::new("git")
        .current_dir(&repo.root)
        .args([
            "worktree",
            "add",
            worktree_path.to_str().unwrap(),
            "--detach",
        ])
        .output()
        .context("Failed to create worktree")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("Failed to create worktree: {}", stderr.trim());
    }

    checkout_pr(&worktree_path, opts.number)?;

    if opts.print_path {
        println!("{}", worktree_path.display());
    } else {
        print_success(&format!(
            "Created PR worktree at {}",
            worktree_path.display()
        ));
    }

    if opts.open {
        if let Some(open_cmd) = &config.open_cmd {
            let _ = Command::new(open_cmd).arg(&worktree_path).spawn();
        }
    }

    Ok(())
}
