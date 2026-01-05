use crate::git::GitRepo;
use crate::status::WorktreeStatus;
use crate::worktree::Worktree;
use owo_colors::OwoColorize;
use serde::Serialize;
use std::io::{self, Write};
use std::path::Path;

#[derive(Debug, Clone, Copy)]
pub struct UiOptions {
    pub color: bool,
    pub ascii: bool,
    pub json: bool,
}

impl Default for UiOptions {
    fn default() -> Self {
        Self {
            color: true,
            ascii: false,
            json: false,
        }
    }
}

pub struct Icons {
    pub current: &'static str,
    pub dirty: &'static str,
    pub clean: &'static str,
    pub arrow_up: &'static str,
    pub arrow_down: &'static str,
}

impl Icons {
    pub fn unicode() -> Self {
        Self {
            current: "▶",
            dirty: "●",
            clean: "✓",
            arrow_up: "↑",
            arrow_down: "↓",
        }
    }

    pub fn ascii() -> Self {
        Self {
            current: ">",
            dirty: "*",
            clean: "-",
            arrow_up: "^",
            arrow_down: "v",
        }
    }

    pub fn from_options(opts: &UiOptions) -> Self {
        if opts.ascii {
            Self::ascii()
        } else {
            Self::unicode()
        }
    }
}

pub fn print_worktree_list(
    repo: &GitRepo,
    worktrees: &[(Worktree, WorktreeStatus)],
    current_path: &Path,
    opts: &UiOptions,
) {
    if opts.json {
        print_worktree_list_json(repo, worktrees, current_path);
        return;
    }

    let icons = Icons::from_options(opts);

    let max_name_len = worktrees
        .iter()
        .map(|(wt, _)| wt.name().len())
        .max()
        .unwrap_or(10);

    for (wt, status) in worktrees {
        let is_current = wt.path == current_path;

        let marker = if is_current { icons.current } else { " " };

        let name = wt.name();
        let name_padded = format!("{:width$}", name, width = max_name_len);

        let dirty_str = format_dirty(status, &icons, opts);
        let sync_str = format_sync(status, &icons);
        let path_str = shorten_path(&wt.path);

        if opts.color {
            let name_colored = if is_current {
                name_padded.green().bold().to_string()
            } else if status.is_dirty() {
                name_padded.yellow().to_string()
            } else {
                name_padded.to_string()
            };

            let marker_colored = if is_current {
                marker.green().bold().to_string()
            } else {
                marker.to_string()
            };

            println!(
                "{} {}  {}  {:>6}  {}",
                marker_colored, name_colored, dirty_str, sync_str, path_str.dimmed()
            );
        } else {
            println!(
                "{} {}  {}  {:>6}  {}",
                marker, name_padded, dirty_str, sync_str, path_str
            );
        }
    }
}

fn format_dirty(status: &WorktreeStatus, icons: &Icons, opts: &UiOptions) -> String {
    if status.dirty_count > 0 {
        let s = format!("{}{:>3}", icons.dirty, status.dirty_count);
        if opts.color {
            s.yellow().to_string()
        } else {
            s
        }
    } else {
        let s = format!("{:>4}", icons.clean);
        if opts.color {
            s.green().to_string()
        } else {
            s
        }
    }
}

fn format_sync(status: &WorktreeStatus, icons: &Icons) -> String {
    match (status.ahead, status.behind) {
        (Some(a), Some(b)) if a > 0 || b > 0 => {
            format!("{}{}{}{}", icons.arrow_up, a, icons.arrow_down, b)
        }
        (Some(0), Some(0)) => format!("{}0{}0", icons.arrow_up, icons.arrow_down),
        _ => "-".to_string(),
    }
}

pub fn shorten_path(path: &Path) -> String {
    if let Some(home) = dirs::home_dir() {
        if let Ok(stripped) = path.strip_prefix(&home) {
            return format!("~/{}", stripped.display());
        }
    }
    path.display().to_string()
}

#[derive(Serialize)]
struct JsonOutput<'a> {
    repo: RepoInfo<'a>,
    current: &'a str,
    worktrees: Vec<JsonWorktree<'a>>,
}

#[derive(Serialize)]
struct RepoInfo<'a> {
    root: &'a str,
    common_dir: &'a str,
}

#[derive(Serialize)]
struct JsonWorktree<'a> {
    path: &'a str,
    branch: Option<&'a str>,
    branch_short: Option<&'a str>,
    head: &'a str,
    detached: bool,
    locked: bool,
    dirty: DirtyInfo,
    upstream: Option<&'a str>,
    ahead: Option<usize>,
    behind: Option<usize>,
}

#[derive(Serialize)]
struct DirtyInfo {
    count: usize,
}

fn print_worktree_list_json(
    repo: &GitRepo,
    worktrees: &[(Worktree, WorktreeStatus)],
    current_path: &Path,
) {
    let root_str = repo.root.to_string_lossy();
    let common_str = repo.common_dir.to_string_lossy();
    let current_str = current_path.to_string_lossy();

    let json_worktrees: Vec<JsonWorktree> = worktrees
        .iter()
        .map(|(wt, status)| JsonWorktree {
            path: Box::leak(wt.path.to_string_lossy().into_owned().into_boxed_str()),
            branch: wt.branch.as_deref(),
            branch_short: wt.branch_short.as_deref(),
            head: &wt.head,
            detached: wt.detached,
            locked: wt.locked,
            dirty: DirtyInfo {
                count: status.dirty_count,
            },
            upstream: status.upstream.as_deref(),
            ahead: status.ahead,
            behind: status.behind,
        })
        .collect();

    let output = JsonOutput {
        repo: RepoInfo {
            root: Box::leak(root_str.into_owned().into_boxed_str()),
            common_dir: Box::leak(common_str.into_owned().into_boxed_str()),
        },
        current: Box::leak(current_str.into_owned().into_boxed_str()),
        worktrees: json_worktrees,
    };

    let json = serde_json::to_string_pretty(&output).unwrap_or_else(|_| "{}".to_string());
    println!("{}", json);
}

pub fn print_error(msg: &str, hint: Option<&str>) {
    let stderr = io::stderr();
    let mut handle = stderr.lock();

    let _ = writeln!(handle, "{}: {}", "error".red().bold(), msg);
    if let Some(h) = hint {
        let _ = writeln!(handle, "{}: {}", "hint".cyan(), h);
    }
}

pub fn print_success(msg: &str) {
    eprintln!("{}: {}", "success".green().bold(), msg);
}

pub fn print_warning(msg: &str) {
    eprintln!("{}: {}", "warning".yellow().bold(), msg);
}

pub fn print_info(msg: &str) {
    eprintln!("{}", msg);
}
