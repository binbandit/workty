use crate::git::GitRepo;

use crate::ui::{print_error, UiOptions};
use crate::worktree::list_worktrees;
use anyhow::Result;
use dialoguer::FuzzySelect;
use is_terminal::IsTerminal;

pub fn execute(repo: &GitRepo, _opts: &UiOptions) -> Result<()> {
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

    let items: Vec<String> = worktrees
        .iter()
        .map(|worktree| worktree.name().to_string())
        .collect();

    let selection = FuzzySelect::new()
        .with_prompt("Select worktree")
        .items(&items)
        .default(0)
        .interact_opt()?;

    match selection {
        Some(idx) => {
            println!("{}", worktrees[idx].path.display());
            Ok(())
        }
        None => {
            std::process::exit(130);
        }
    }
}
