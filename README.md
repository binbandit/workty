# git-workty

I got mass-context-switching burnout. Every `git stash` felt like shoving clothes under the bed before guests arrive. WIP commits? "WIP: stuff" cluttering my history. Worktrees fixed it, but the raw commands are clunky.

So I built this. Now I just `wnew feat/login`, do the work, `wcd` back to main when someone pings me about a bug, fix it, and `wcd` right back. No stashing. No mental overhead.

```
$ git workty
▶ feat/login       ● 3   ↑2↓0   ~/.workty/myrepo/feat-login
  main             ✓     ↑0↓0   ~/src/myrepo
  hotfix-auth      ● 1   -      ~/.workty/myrepo/hotfix-auth
```

## Install

```bash
cargo install git-workty
```

Then add shell integration (so `wcd`, `wnew`, `wgo` actually change your directory):

```bash
# zsh
eval "$(git workty init zsh)"

# bash  
eval "$(git workty init bash)"

# fish
git workty init fish | source
```

### Manpages

To generate and install the manpage automatically:

```bash
git workty install-man
```

This will write `git-workty.1` to `~/.local/share/man/man1`. You may need to add `~/.local/share/man` to your `MANPATH` environment variable if your system doesn't pick it up automatically.

## Usage

The whole point is to make worktrees feel like browser tabs:

```bash
wnew feat/login     # new worktree + cd into it
wcd                 # fuzzy-pick a worktree + cd
wgo main            # jump to "main" worktree

git workty          # see everything at a glance
git workty clean --merged   # tidy up finished work
```

### All commands

| Command | What it does |
|---------|--------------|
| `git workty` | Dashboard showing all worktrees |
| `git workty new <branch>` | Create worktree (and branch if needed) |
| `git workty go <name>` | Print path to worktree |
| `git workty pick` | Fuzzy selector |
| `git workty rm <name>` | Remove worktree (prompts if dirty) |
| `git workty clean --merged` | Remove worktrees with merged branches |
| `git workty pr <num>` | Checkout a GitHub PR (needs `gh`) |
| `git workty doctor` | Diagnose issues |

## Config

Optional. Lives in `.git/workty.toml`:

```toml
base = "main"                    # default branch for new worktrees
root = "~/.workty/{repo}-{id}"   # where worktrees go
open_cmd = "code"                # editor for --open flag
```

## Why not just...

**Why not `git stash`?** — Stashes get lost. I've got 47 stashes in one repo right now. No idea what's in them.

**Why not WIP commits?** — They clutter history and I forget to squash them.

**Why not just raw `git worktree`?** — The commands are verbose and I kept forgetting the syntax. This is just a nice wrapper.

## Safety

Won't delete dirty worktrees unless you `--force`. Prompts before destructive stuff unless you `--yes`. Every error tells you what to do next.

## License

MIT or Apache-2.0, your choice.
