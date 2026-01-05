use clap::CommandFactory;
use clap_complete::{generate, Shell};
use std::io;

pub fn execute<C: CommandFactory>(shell: Shell) {
    let mut cmd = C::command();
    generate(shell, &mut cmd, "git-workty", &mut io::stdout());
}
