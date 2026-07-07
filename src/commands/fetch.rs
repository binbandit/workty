use crate::git::GitRepo;
use crate::ui;
use anyhow::{Context, Result};
use std::process::Command;

#[derive(Debug, clap::Args)]
pub struct FetchArgs {
    /// Fetch from all remotes, not just origin
    #[arg(long, short = 'a')]
    pub all: bool,
}

pub fn execute(repo: &GitRepo, all: bool) -> Result<()> {
    ui::print_info("Fetching from remotes...");

    let remote_names: Vec<String> = if all {
        let remotes = repo.repo.remotes().context("Failed to list remotes")?;
        remotes.iter().flatten().map(|s| s.to_string()).collect()
    } else {
        // Just fetch origin by default
        vec!["origin".to_string()]
    };

    for remote in &remote_names {
        ui::print_info(&format!("  Fetching {}...", remote));

        let output = Command::new("git")
            .current_dir(&repo.root)
            .args(["fetch", "--prune", remote])
            .output()
            .context("Failed to run git fetch")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            ui::print_warning(&format!("Failed to fetch {}: {}", remote, stderr.trim()));
        }
    }

    ui::print_success(&format!(
        "Fetched {} remote{}",
        remote_names.len(),
        if remote_names.len() == 1 { "" } else { "s" }
    ));

    Ok(())
}
