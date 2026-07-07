#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum ShellKind {
    Bash,
    Zsh,
    Fish,
    #[value(alias = "pwsh")]
    Powershell,
}

impl ShellKind {
    fn name(self) -> &'static str {
        match self {
            ShellKind::Bash => "bash",
            ShellKind::Zsh => "zsh",
            ShellKind::Fish => "fish",
            ShellKind::Powershell => "PowerShell",
        }
    }
}

pub fn generate_init(shell: ShellKind, wrap_git: bool, no_cd: bool) -> String {
    let mut output = format!("# git-workty shell integration for {}\n\n", shell.name());

    if !no_cd {
        output.push_str(match shell {
            // The same script is valid in both bash and zsh
            ShellKind::Bash | ShellKind::Zsh => POSIX_HELPERS,
            ShellKind::Fish => FISH_HELPERS,
            ShellKind::Powershell => POWERSHELL_HELPERS,
        });
    }

    if wrap_git {
        output.push_str(match shell {
            ShellKind::Bash | ShellKind::Zsh => POSIX_GIT_WRAPPER,
            ShellKind::Fish => FISH_GIT_WRAPPER,
            ShellKind::Powershell => POWERSHELL_GIT_WRAPPER,
        });
    }

    output
}

const POSIX_HELPERS: &str = r#"# wcd - fuzzy select and cd to a worktree
wcd() {
    local dir
    dir="$(git workty pick 2>/dev/null)"
    if [ -n "$dir" ] && [ -d "$dir" ]; then
        cd "$dir" || return 1
    fi
}

# wnew - create new worktree and cd into it
wnew() {
    if [ -z "$1" ]; then
        echo "Usage: wnew <branch-name>" >&2
        return 1
    fi
    local dir
    dir="$(git workty new "$@" --print-path 2>/dev/null)"
    if [ -n "$dir" ] && [ -d "$dir" ]; then
        cd "$dir" || return 1
    fi
}

# wgo - go to a worktree by name
wgo() {
    if [ -z "$1" ]; then
        echo "Usage: wgo <worktree-name>" >&2
        return 1
    fi
    local dir
    dir="$(git workty go "$1" 2>/dev/null)"
    if [ -n "$dir" ] && [ -d "$dir" ]; then
        cd "$dir" || return 1
    else
        echo "Worktree not found: $1" >&2
        return 1
    fi
}

"#;

const POSIX_GIT_WRAPPER: &str = r#"# git wrapper that auto-cds for workty commands
git() {
    if [ "$1" != "workty" ]; then
        command git "$@"
        return
    fi
    local dir
    case "$2" in
        go|pick)
            dir="$(command git workty "${@:2}" 2>/dev/null)"
            ;;
        new)
            dir="$(command git workty "${@:2}" --print-path)"
            ;;
        *)
            command git "$@"
            return
            ;;
    esac
    if [ -n "$dir" ] && [ -d "$dir" ]; then
        cd "$dir"
    else
        command git "$@"
    fi
}

"#;

const FISH_HELPERS: &str = r#"# wcd - fuzzy select and cd to a worktree
function wcd
    set -l dir (git workty pick 2>/dev/null)
    if test -n "$dir" -a -d "$dir"
        cd "$dir"
    end
end

# wnew - create new worktree and cd into it
function wnew
    if test (count $argv) -eq 0
        echo "Usage: wnew <branch-name>" >&2
        return 1
    end
    set -l dir (git workty new $argv --print-path 2>/dev/null)
    if test -n "$dir" -a -d "$dir"
        cd "$dir"
    end
end

# wgo - go to a worktree by name
function wgo
    if test (count $argv) -eq 0
        echo "Usage: wgo <worktree-name>" >&2
        return 1
    end
    set -l dir (git workty go $argv[1] 2>/dev/null)
    if test -n "$dir" -a -d "$dir"
        cd "$dir"
    else
        echo "Worktree not found: $argv[1]" >&2
        return 1
    end
end

"#;

const FISH_GIT_WRAPPER: &str = r#"# git wrapper that auto-cds for workty commands
function git --wraps git
    if test "$argv[1]" != workty
        command git $argv
        return
    end
    switch $argv[2]
        case go pick
            set -l dir (command git workty $argv[2..] 2>/dev/null)
            if test -n "$dir" -a -d "$dir"
                cd "$dir"
            else
                command git $argv
            end
        case new
            set -l dir (command git workty $argv[2..] --print-path)
            if test -n "$dir" -a -d "$dir"
                cd "$dir"
            else
                command git $argv
            end
        case '*'
            command git $argv
    end
end

"#;

const POWERSHELL_HELPERS: &str = r#"# wcd - fuzzy select and cd to a worktree
function wcd {
    $dir = git workty pick 2>$null
    if ($dir -and (Test-Path $dir)) {
        Set-Location $dir
    }
}

# wnew - create new worktree and cd into it
function wnew {
    param([Parameter(Mandatory=$true)][string]$Name)
    $dir = git workty new $Name --print-path 2>$null
    if ($dir -and (Test-Path $dir)) {
        Set-Location $dir
    }
}

# wgo - go to a worktree by name
function wgo {
    param([Parameter(Mandatory=$true)][string]$Name)
    $dir = git workty go $Name 2>$null
    if ($dir -and (Test-Path $dir)) {
        Set-Location $dir
    } else {
        Write-Error "Worktree not found: $Name"
    }
}

"#;

const POWERSHELL_GIT_WRAPPER: &str = r#"# Note: Git wrapper for PowerShell requires more complex setup.
# Consider using the wcd, wnew, and wgo functions directly.

"#;
