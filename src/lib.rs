pub mod commands;
pub mod config;
pub mod gh;
pub mod git;
pub mod shell;
pub mod status;
pub mod ui;
pub mod worktree;

use clap::{Parser, Subcommand};
use clap_complete::Shell;
use std::path::PathBuf;

use crate::commands::{
    clean, completions, doctor, fetch, go, init, install_man, list, new, pick, pr, rm, sync,
};
use crate::git::GitRepo;
use crate::ui::UiOptions;

pub const ABOUT: &str = "Git worktrees as daily-driver workspaces

workty makes Git worktrees feel like workspaces/tabs. Switch context without
stashing or WIP commits, see everything in flight with a dashboard, and clean
up merged work safely.";

pub const AFTER_HELP: &str = "EXAMPLES:
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
#[command(name = "git-workty", bin_name = "git workty")]
#[command(author, version, about = ABOUT, after_help = AFTER_HELP)]
#[command(propagate_version = true)]
pub struct Cli {
    /// Disable colored output (also respects the NO_COLOR env var)
    #[arg(long, global = true)]
    pub no_color: bool,

    /// Use ASCII-only symbols
    #[arg(long, global = true)]
    pub ascii: bool,

    /// Output in JSON format
    #[arg(long, global = true)]
    pub json: bool,

    /// Run as if started in <PATH>
    #[arg(short = 'C', global = true, value_name = "PATH")]
    pub directory: Option<PathBuf>,

    /// Assume yes to prompts
    #[arg(long, short = 'y', global = true)]
    pub yes: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Show dashboard of all worktrees (default)
    #[command(visible_alias = "ls")]
    List(list::ListArgs),

    /// Create a new workspace
    #[command(after_help = "EXAMPLES:
    git workty new feat/login
    git workty new hotfix --from main
    git workty new feature --no-fetch --no-push")]
    New(new::NewArgs),

    /// Print path to a worktree by name
    #[command(after_help = "EXAMPLES:
    cd \"$(git workty go feat/login)\"
    git workty go main")]
    Go(go::GoArgs),

    /// Interactively select a worktree (fuzzy finder)
    #[command(after_help = "EXAMPLES:
    cd \"$(git workty pick)\"")]
    Pick,

    /// Remove a workspace
    #[command(after_help = "EXAMPLES:
    git workty rm feat/login
    git workty rm feat/login --delete-branch
    git workty rm feat/login --force")]
    Rm(rm::RmArgs),

    /// Remove merged or stale worktrees
    #[command(after_help = "EXAMPLES:
    git workty clean --merged --dry-run
    git workty clean --gone --yes
    git workty clean --stale 30")]
    Clean(clean::CleanArgs),

    /// Print shell integration script
    #[command(after_help = "EXAMPLES:
    eval \"$(git workty init zsh)\"
    git workty init bash >> ~/.bashrc")]
    Init(init::InitArgs),

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
    Pr(pr::PrArgs),

    /// Fetch from remotes (updates tracking info for all worktrees)
    #[command(after_help = "EXAMPLES:
    git workty fetch
    git workty fetch --all")]
    Fetch(fetch::FetchArgs),

    /// Rebase all clean worktrees onto their upstream
    #[command(after_help = "EXAMPLES:
    git workty sync --dry-run
    git workty sync --fetch")]
    Sync(sync::SyncArgs),

    /// Install manpage to ~/.local/share/man/man1
    InstallMan,
}

pub fn run_cli() {
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
    let repo = || GitRepo::discover(cli.directory.as_deref());

    match cli.command {
        None => list::execute(&repo()?, ui_opts, list::ListArgs::default()),
        Some(Commands::List(args)) => list::execute(&repo()?, ui_opts, args),
        Some(Commands::New(args)) => new::execute(&repo()?, args),
        Some(Commands::Go(args)) => go::execute(&repo()?, &args.name),
        Some(Commands::Pick) => pick::execute(&repo()?, ui_opts),
        Some(Commands::Rm(args)) => rm::execute(&repo()?, args, cli.yes),
        Some(Commands::Clean(args)) => clean::execute(&repo()?, args, cli.yes),
        Some(Commands::Init(args)) => {
            init::execute(args);
            Ok(())
        }
        Some(Commands::Doctor) => {
            doctor::execute(cli.directory.as_deref());
            Ok(())
        }
        Some(Commands::Completions { shell }) => {
            completions::execute::<Cli>(shell);
            Ok(())
        }
        Some(Commands::Pr(args)) => pr::execute(&repo()?, args),
        Some(Commands::Fetch(args)) => fetch::execute(&repo()?, args.all),
        Some(Commands::Sync(args)) => sync::execute(&repo()?, args),
        Some(Commands::InstallMan) => install_man::execute(cli.yes),
    }
}

fn supports_color() -> bool {
    use std::io::IsTerminal;

    // Per the NO_COLOR spec, any non-empty value disables color.
    if std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty()) {
        return false;
    }

    std::io::stdout().is_terminal()
}
