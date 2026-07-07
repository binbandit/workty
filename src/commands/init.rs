use crate::shell::{ShellKind, generate_init};

#[derive(Debug, clap::Args)]
pub struct InitArgs {
    /// Shell to generate script for
    #[arg(value_enum)]
    pub shell: ShellKind,

    /// Generate git wrapper that auto-cds
    #[arg(long)]
    pub wrap_git: bool,

    /// Disable cd helpers (completions only)
    #[arg(long)]
    pub no_cd: bool,
}

pub fn execute(args: InitArgs) {
    print!("{}", generate_init(args.shell, args.wrap_git, args.no_cd));
}
