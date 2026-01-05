use crate::git::GitRepo;
use crate::status::get_all_statuses;
use crate::ui::{print_error, shorten_path, Icons, UiOptions};
use crate::worktree::list_worktrees;
use anyhow::Result;
use dialoguer::FuzzySelect;
use is_terminal::IsTerminal;

pub fn execute(repo: &GitRepo, opts: &UiOptions) -> Result<()> {
    if !std::io::stdin().is_terminal() {
        print_error(
            "Cannot run interactive picker in non-TTY environment",
            Some("Use `git workty go <name>` for non-interactive selection."),
        );
        std::process::exit(1);
    }

    let worktrees = list_worktrees(repo)?;
    if worktrees.is_empty() {
        print_error("No worktrees found", None);
        std::process::exit(1);
    }

    let statuses = get_all_statuses(repo, &worktrees);
    let icons = Icons::from_options(opts);

    let items: Vec<String> = statuses
        .iter()
        .map(|(wt, status)| {
            let dirty = if status.is_dirty() {
                format!("{}{}", icons.dirty, status.dirty_count)
            } else {
                icons.clean.to_string()
            };

            let sync = match (status.ahead, status.behind) {
                (Some(a), Some(b)) => format!("{}{}{}{}", icons.arrow_up, a, icons.arrow_down, b),
                _ => "-".to_string(),
            };

            format!(
                "{:<20}  {:>4}  {:>8}   {}",
                wt.name(),
                dirty,
                sync,
                shorten_path(&wt.path)
            )
        })
        .collect();

    let selection = FuzzySelect::new()
        .with_prompt("Select worktree")
        .items(&items)
        .default(0)
        .interact_opt()?;

    match selection {
        Some(idx) => {
            println!("{}", statuses[idx].0.path.display());
            Ok(())
        }
        None => {
            std::process::exit(130);
        }
    }
}
