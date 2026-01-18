use crate::Cli;
use anyhow::{Context, Result};
use clap::CommandFactory;
use clap_mangen::Man;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Confirm;
use std::fs;

pub fn execute(yes: bool) -> Result<()> {
    let home = dirs::home_dir().context("Could not determine home directory")?;
    let target_dir = home.join(".local/share/man/man1");
    let target_file = target_dir.join("git-workty.1");

    if !yes {
        let theme = ColorfulTheme::default();
        let confirmed = Confirm::with_theme(&theme)
            .with_prompt(format!("Install manpage to {}?", target_file.display()))
            .default(true)
            .interact()?;

        if !confirmed {
            println!("Aborted.");
            return Ok(());
        }
    }

    fs::create_dir_all(&target_dir)
        .with_context(|| format!("Failed to create directory: {}", target_dir.display()))?;

    let cmd = Cli::command();
    let man = Man::new(cmd);

    let mut buffer: Vec<u8> = Default::default();
    man.render(&mut buffer)?;

    fs::write(&target_file, buffer)
        .with_context(|| format!("Failed to write manpage to {}", target_file.display()))?;

    println!("Manpage installed to {}", target_file.display());
    println!("You may need to add ~/.local/share/man to your MANPATH if it's not already there.");

    Ok(())
}
