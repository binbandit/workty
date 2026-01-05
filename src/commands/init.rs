use crate::shell::generate_init;

pub struct InitOptions {
    pub shell: String,
    pub wrap_git: bool,
    pub no_cd: bool,
}

pub fn execute(opts: InitOptions) {
    let output = generate_init(&opts.shell, opts.wrap_git, opts.no_cd);
    print!("{}", output);
}
