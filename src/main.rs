mod commands;
mod config;
mod gh;
mod git;
mod shell;
mod status;
mod ui;
mod worktree;

use clap::{Parser, Subcommand};
use clap_complete::Shell;
use std::path::PathBuf;

use crate::commands::{clean, completions, doctor, go, init, list, new, pick, pr, rm};
use crate::git::GitRepo;
use crate::ui::UiOptions;

const ABOUT: &str = "Git worktrees as daily-driver workspaces

workty makes Git worktrees feel like workspaces/tabs. Switch context without
stashing or WIP commits, see everything in flight with a dashboard, and clean
up merged work safely.";

const AFTER_HELP: &str = "EXAMPLES:
    git workty                    Show dashboard of all worktrees
    git workty new feat/login     Create new workspace for feat/login
    git workty go feat/login      Print path to feat/login worktree
    git workty pick               Fuzzy select a worktree (interactive)
    git workty rm feat/login      Remove the feat/login worktree
    git workty clean --merged     Remove all merged worktrees

SHELL INTEGRATION:
    Add to your shell config:
        eval \"$(git workty init zsh)\"

    This provides:
        wcd   - fuzzy select and cd to a worktree
        wnew  - create new worktree and cd into it
        wgo   - go to a worktree by name";

#[derive(Parser)]
#[command(name = "git-workty")]
#[command(author, version, about = ABOUT, after_help = AFTER_HELP)]
#[command(propagate_version = true)]
struct Cli {
    /// Disable colored output
    #[arg(long, global = true, env = "NO_COLOR")]
    no_color: bool,

    /// Use ASCII-only symbols
    #[arg(long, global = true)]
    ascii: bool,

    /// Output in JSON format
    #[arg(long, global = true)]
    json: bool,

    /// Run as if started in <PATH>
    #[arg(short = 'C', global = true, value_name = "PATH")]
    directory: Option<PathBuf>,

    /// Assume yes to prompts
    #[arg(long, short = 'y', global = true)]
    yes: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Show dashboard of all worktrees (default)
    #[command(visible_alias = "ls")]
    List,

    /// Create a new workspace
    #[command(after_help = "EXAMPLES:
    git workty new feat/login
    git workty new hotfix --from main
    git workty new feature --print-path")]
    New {
        /// Branch name for the new workspace
        name: String,

        /// Base branch or commit to create from
        #[arg(long, short = 'f')]
        from: Option<String>,

        /// Custom path for the worktree
        #[arg(long, short = 'p')]
        path: Option<PathBuf>,

        /// Print only the created path to stdout
        #[arg(long)]
        print_path: bool,

        /// Open the worktree in configured editor
        #[arg(long, short = 'o')]
        open: bool,
    },

    /// Print path to a worktree by name
    #[command(after_help = "EXAMPLES:
    cd \"$(git workty go feat/login)\"
    git workty go main")]
    Go {
        /// Worktree name (branch name or directory name)
        name: String,
    },

    /// Interactively select a worktree (fuzzy finder)
    #[command(after_help = "EXAMPLES:
    cd \"$(git workty pick)\"")]
    Pick,

    /// Remove a workspace
    #[command(after_help = "EXAMPLES:
    git workty rm feat/login
    git workty rm feat/login --delete-branch
    git workty rm feat/login --force")]
    Rm {
        /// Worktree name to remove
        name: String,

        /// Remove even if worktree has uncommitted changes
        #[arg(long, short = 'f')]
        force: bool,

        /// Also delete the branch after removing worktree
        #[arg(long, short = 'd')]
        delete_branch: bool,
    },

    /// Remove merged or stale worktrees
    #[command(after_help = "EXAMPLES:
    git workty clean --merged --dry-run
    git workty clean --merged --yes")]
    Clean {
        /// Only remove worktrees whose branch is merged into base
        #[arg(long)]
        merged: bool,

        /// Show what would be removed without removing
        #[arg(long, short = 'n')]
        dry_run: bool,
    },

    /// Print shell integration script
    #[command(after_help = "EXAMPLES:
    eval \"$(git workty init zsh)\"
    git workty init bash >> ~/.bashrc")]
    Init {
        /// Shell to generate script for (bash, zsh, fish, powershell)
        shell: String,

        /// Generate git wrapper that auto-cds
        #[arg(long)]
        wrap_git: bool,

        /// Disable cd helpers (completions only)
        #[arg(long)]
        no_cd: bool,
    },

    /// Diagnose common issues
    Doctor,

    /// Generate shell completions
    #[command(after_help = "EXAMPLES:
    git workty completions zsh > _git-workty
    git workty completions bash > /etc/bash_completion.d/git-workty")]
    Completions {
        /// Shell to generate completions for
        shell: Shell,
    },

    /// Create a worktree for a GitHub PR (requires gh CLI)
    #[command(after_help = "EXAMPLES:
    git workty pr 123
    cd \"$(git workty pr 123 --print-path)\"")]
    Pr {
        /// PR number
        number: u32,

        /// Print only the created path to stdout
        #[arg(long)]
        print_path: bool,

        /// Open the worktree in configured editor
        #[arg(long, short = 'o')]
        open: bool,
    },
}

fn main() {
    let cli = Cli::parse();

    let ui_opts = UiOptions {
        color: !cli.no_color && supports_color(),
        ascii: cli.ascii,
        json: cli.json,
    };

    let result = run(cli, &ui_opts);

    if let Err(e) = result {
        ui::print_error(&format!("{:#}", e), None);
        std::process::exit(1);
    }
}

fn run(cli: Cli, ui_opts: &UiOptions) -> anyhow::Result<()> {
    let start_path = cli.directory.as_deref();

    match cli.command {
        None | Some(Commands::List) => {
            let repo = GitRepo::discover(start_path)?;
            list::execute(&repo, ui_opts)
        }

        Some(Commands::New {
            name,
            from,
            path,
            print_path,
            open,
        }) => {
            let repo = GitRepo::discover(start_path)?;
            new::execute(
                &repo,
                new::NewOptions {
                    name,
                    from,
                    path,
                    print_path,
                    open,
                },
            )
        }

        Some(Commands::Go { name }) => {
            let repo = GitRepo::discover(start_path)?;
            go::execute(&repo, &name)
        }

        Some(Commands::Pick) => {
            let repo = GitRepo::discover(start_path)?;
            pick::execute(&repo, ui_opts)
        }

        Some(Commands::Rm {
            name,
            force,
            delete_branch,
        }) => {
            let repo = GitRepo::discover(start_path)?;
            rm::execute(
                &repo,
                rm::RmOptions {
                    name,
                    force,
                    delete_branch,
                    yes: cli.yes,
                },
            )
        }

        Some(Commands::Clean { merged, dry_run }) => {
            let repo = GitRepo::discover(start_path)?;
            clean::execute(
                &repo,
                clean::CleanOptions {
                    merged,
                    dry_run,
                    yes: cli.yes,
                },
            )
        }

        Some(Commands::Init {
            shell,
            wrap_git,
            no_cd,
        }) => {
            init::execute(init::InitOptions {
                shell,
                wrap_git,
                no_cd,
            });
            Ok(())
        }

        Some(Commands::Doctor) => {
            doctor::execute(start_path);
            Ok(())
        }

        Some(Commands::Completions { shell }) => {
            completions::execute::<Cli>(shell);
            Ok(())
        }

        Some(Commands::Pr {
            number,
            print_path,
            open,
        }) => {
            let repo = GitRepo::discover(start_path)?;
            pr::execute(
                &repo,
                pr::PrOptions {
                    number,
                    print_path,
                    open,
                },
            )
        }
    }
}

fn supports_color() -> bool {
    use is_terminal::IsTerminal;

    if std::env::var("NO_COLOR").is_ok() {
        return false;
    }

    std::io::stdout().is_terminal()
}
