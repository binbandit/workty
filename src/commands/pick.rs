use crate::git::GitRepo;
use crate::status::get_all_statuses;
use crate::ui::{format_time, UiOptions};
use crate::worktree::list_worktrees;
use anyhow::{bail, Result};
use console::Term;
use dialoguer::FuzzySelect;
use std::io::IsTerminal;

pub fn execute(repo: &GitRepo, _opts: &UiOptions) -> Result<()> {
    if !std::io::stdin().is_terminal() {
        bail!("Cannot run interactive picker in non-TTY. Use `git workty go <name>` instead.");
    }

    let worktrees = list_worktrees(repo)?;
    if worktrees.is_empty() {
        bail!("No worktrees found");
    }

    // Get status for richer display
    let statuses = get_all_statuses(repo, &worktrees);

    // Find max name length for alignment
    let max_name_len = statuses
        .iter()
        .map(|(wt, _)| wt.name().len())
        .max()
        .unwrap_or(10);

    let items: Vec<String> = statuses
        .iter()
        .map(|(wt, status)| {
            let name = format!("{:width$}", wt.name(), width = max_name_len);

            // Dirty indicator
            let dirty = if status.dirty_count > 0 {
                format!("*{}", status.dirty_count)
            } else {
                " ".to_string()
            };

            // Time since last commit
            let time = format_time(status.last_commit_time);

            // Needs rebase indicator
            let rebase = if status.needs_rebase() {
                format!("R{}", status.behind_main.unwrap_or(0))
            } else {
                "  ".to_string()
            };

            format!("{}  {:>3}  {:>4}  {}", name, dirty, time, rebase)
        })
        .collect();

    let selection = FuzzySelect::new()
        .with_prompt("Select worktree")
        .items(&items)
        .default(0)
        .interact_on_opt(&Term::stderr())?;

    match selection {
        Some(idx) => {
            println!("{}", statuses[idx].0.path.display());
            Ok(())
        }
        None => {
            // User cancelled - exit with 130 (128 + SIGINT)
            std::process::exit(130);
        }
    }
}
