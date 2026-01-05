use crate::config::{config_exists, Config};
use crate::git::{is_git_installed, is_in_git_repo, GitRepo};
use crate::worktree::list_worktrees;
use owo_colors::OwoColorize;
use std::path::Path;
use std::process::Command;

pub fn execute(start_path: Option<&Path>) {
    let mut all_ok = true;

    print_check("Git installed", is_git_installed(), &mut all_ok);

    let cwd = start_path
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    let in_repo = is_in_git_repo(&cwd);
    print_check("Inside Git repository", in_repo, &mut all_ok);

    if !in_repo {
        eprintln!("\n{}: Run this command from inside a Git repository.", "hint".cyan());
        return;
    }

    let repo = match GitRepo::discover(start_path) {
        Ok(r) => r,
        Err(e) => {
            print_fail(&format!("Failed to discover repo: {}", e));
            return;
        }
    };

    eprintln!("  Repository root: {}", repo.root.display());
    eprintln!("  Common dir: {}", repo.common_dir.display());

    let worktrees = list_worktrees(&repo);
    print_check("Can list worktrees", worktrees.is_ok(), &mut all_ok);

    if let Ok(wts) = &worktrees {
        eprintln!("  Found {} worktree(s)", wts.len());

        let prunable: Vec<_> = wts.iter().filter(|wt| wt.prunable).collect();
        if !prunable.is_empty() {
            print_warn(&format!("{} prunable worktree(s) found", prunable.len()));
            eprintln!(
                "  {}: Run `git worktree prune` to clean up.",
                "hint".cyan()
            );
        }
    }

    print_check("Config exists", config_exists(&repo), &mut all_ok);

    if config_exists(&repo) {
        match Config::load(&repo) {
            Ok(config) => {
                eprintln!("  Base branch: {}", config.base);
                eprintln!("  Workspace root: {}", config.root);
            }
            Err(e) => {
                print_fail(&format!("Config parse error: {}", e));
                all_ok = false;
            }
        }
    } else {
        eprintln!("  Using default config (no workty.toml found)");
    }

    let gh_installed = Command::new("gh")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if gh_installed {
        eprintln!("  {} GitHub CLI (gh) available", "✓".green());

        let gh_auth = Command::new("gh")
            .args(["auth", "status"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if gh_auth {
            eprintln!("  {} GitHub CLI authenticated", "✓".green());
        } else {
            eprintln!("  {} GitHub CLI not authenticated", "○".yellow());
            eprintln!("  {}: Run `gh auth login` to enable PR features.", "hint".cyan());
        }
    } else {
        eprintln!("  {} GitHub CLI (gh) not installed (optional)", "○".dimmed());
    }

    eprintln!();
    if all_ok {
        eprintln!("{}", "All checks passed! ✓".green().bold());
    } else {
        eprintln!("{}", "Some checks failed. See hints above.".yellow());
    }
}

fn print_check(name: &str, ok: bool, all_ok: &mut bool) {
    if ok {
        eprintln!("{} {}", "✓".green(), name);
    } else {
        eprintln!("{} {}", "✗".red(), name);
        *all_ok = false;
    }
}

fn print_fail(msg: &str) {
    eprintln!("{} {}", "✗".red(), msg);
}

fn print_warn(msg: &str) {
    eprintln!("{} {}", "!".yellow(), msg);
}
