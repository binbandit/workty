use anyhow::{bail, Context, Result};
use std::process::Command;

pub fn is_gh_installed() -> bool {
    Command::new("gh")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn is_gh_authenticated() -> bool {
    Command::new("gh")
        .args(["auth", "status"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn checkout_pr(worktree_path: &std::path::Path, pr_number: u32) -> Result<()> {
    let output = Command::new("gh")
        .current_dir(worktree_path)
        .args(["pr", "checkout", &pr_number.to_string()])
        .output()
        .context("Failed to execute gh pr checkout")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("gh pr checkout failed: {}", stderr.trim());
    }

    Ok(())
}

pub fn get_pr_branch(pr_number: u32) -> Result<String> {
    let output = Command::new("gh")
        .args([
            "pr",
            "view",
            &pr_number.to_string(),
            "--json",
            "headRefName",
            "-q",
            ".headRefName",
        ])
        .output()
        .context("Failed to get PR branch name")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("Failed to get PR info: {}", stderr.trim());
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
