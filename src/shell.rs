pub fn generate_init(shell: &str, wrap_git: bool, no_cd: bool) -> String {
    match shell {
        "bash" => generate_bash(wrap_git, no_cd),
        "zsh" => generate_zsh(wrap_git, no_cd),
        "fish" => generate_fish(wrap_git, no_cd),
        "powershell" | "pwsh" => generate_powershell(wrap_git, no_cd),
        _ => format!("# Unsupported shell: {}\n", shell),
    }
}

fn generate_bash(wrap_git: bool, no_cd: bool) -> String {
    let mut output = String::new();

    output.push_str("# git-workty shell integration for bash\n\n");

    if !no_cd {
        output.push_str(r#"# wcd - fuzzy select and cd to a worktree
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

"#);
    }

    if wrap_git {
        output.push_str(r#"# git wrapper that auto-cds for workty commands
git() {
    if [ "$1" = "workty" ]; then
        case "$2" in
            go)
                local dir
                dir="$(command git workty go "${@:3}" 2>/dev/null)"
                if [ -n "$dir" ] && [ -d "$dir" ]; then
                    cd "$dir"
                else
                    command git "$@"
                fi
                ;;
            pick)
                local dir
                dir="$(command git workty pick 2>/dev/null)"
                if [ -n "$dir" ] && [ -d "$dir" ]; then
                    cd "$dir"
                else
                    command git "$@"
                fi
                ;;
            new)
                local dir
                dir="$(command git workty new "${@:3}" --print-path)"
                if [ -n "$dir" ] && [ -d "$dir" ]; then
                    cd "$dir"
                else
                    command git "$@"
                fi
                ;;
            *)
                command git "$@"
                ;;
        esac
    else
        command git "$@"
    fi
}

"#);
    }

    output
}

fn generate_zsh(wrap_git: bool, no_cd: bool) -> String {
    let mut output = String::new();

    output.push_str("# git-workty shell integration for zsh\n\n");

    if !no_cd {
        output.push_str(r#"# wcd - fuzzy select and cd to a worktree
wcd() {
    local dir
    dir="$(git workty pick 2>/dev/null)"
    if [[ -n "$dir" ]] && [[ -d "$dir" ]]; then
        cd "$dir"
    fi
}

# wnew - create new worktree and cd into it
wnew() {
    if [[ -z "$1" ]]; then
        echo "Usage: wnew <branch-name>" >&2
        return 1
    fi
    local dir
    dir="$(git workty new "$@" --print-path 2>/dev/null)"
    if [[ -n "$dir" ]] && [[ -d "$dir" ]]; then
        cd "$dir"
    fi
}

# wgo - go to a worktree by name
wgo() {
    if [[ -z "$1" ]]; then
        echo "Usage: wgo <worktree-name>" >&2
        return 1
    fi
    local dir
    dir="$(git workty go "$1" 2>/dev/null)"
    if [[ -n "$dir" ]] && [[ -d "$dir" ]]; then
        cd "$dir"
    else
        echo "Worktree not found: $1" >&2
        return 1
    fi
}

"#);
    }

    if wrap_git {
        output.push_str(r#"# git wrapper that auto-cds for workty commands
git() {
    if [[ "$1" == "workty" ]]; then
        case "$2" in
            go)
                local dir
                dir="$(command git workty go "${@:3}" 2>/dev/null)"
                if [[ -n "$dir" ]] && [[ -d "$dir" ]]; then
                    cd "$dir"
                else
                    command git "$@"
                fi
                ;;
            pick)
                local dir
                dir="$(command git workty pick 2>/dev/null)"
                if [[ -n "$dir" ]] && [[ -d "$dir" ]]; then
                    cd "$dir"
                else
                    command git "$@"
                fi
                ;;
            new)
                local dir
                dir="$(command git workty new "${@:3}" --print-path)"
                if [[ -n "$dir" ]] && [[ -d "$dir" ]]; then
                    cd "$dir"
                else
                    command git "$@"
                fi
                ;;
            *)
                command git "$@"
                ;;
        esac
    else
        command git "$@"
    fi
}

"#);
    }

    output
}

fn generate_fish(wrap_git: bool, no_cd: bool) -> String {
    let mut output = String::new();

    output.push_str("# git-workty shell integration for fish\n\n");

    if !no_cd {
        output.push_str(r#"# wcd - fuzzy select and cd to a worktree
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

"#);
    }

    if wrap_git {
        output.push_str(r#"# git wrapper that auto-cds for workty commands
function git --wraps git
    if test "$argv[1]" = "workty"
        switch $argv[2]
            case go
                set -l dir (command git workty go $argv[3..] 2>/dev/null)
                if test -n "$dir" -a -d "$dir"
                    cd "$dir"
                else
                    command git $argv
                end
            case pick
                set -l dir (command git workty pick 2>/dev/null)
                if test -n "$dir" -a -d "$dir"
                    cd "$dir"
                else
                    command git $argv
                end
            case new
                set -l dir (command git workty new $argv[3..] --print-path)
                if test -n "$dir" -a -d "$dir"
                    cd "$dir"
                else
                    command git $argv
                end
            case '*'
                command git $argv
        end
    else
        command git $argv
    end
end

"#);
    }

    output
}

fn generate_powershell(wrap_git: bool, no_cd: bool) -> String {
    let mut output = String::new();

    output.push_str("# git-workty shell integration for PowerShell\n\n");

    if !no_cd {
        output.push_str(r#"# wcd - fuzzy select and cd to a worktree
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

"#);
    }

    if wrap_git {
        output.push_str(r#"# Note: Git wrapper for PowerShell requires more complex setup.
# Consider using the wcd, wnew, and wgo functions directly.

"#);
    }

    output
}

#[allow(dead_code)]
pub fn supported_shells() -> &'static [&'static str] {
    &["bash", "zsh", "fish", "powershell"]
}
