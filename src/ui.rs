use crate::git::GitRepo;
use crate::status::WorktreeStatus;
use crate::worktree::Worktree;
use owo_colors::{OwoColorize, Style};
use serde::Serialize;
use std::io::{self, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

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

/// Global switch for the print_* helpers, set once at startup.
static COLOR_ENABLED: AtomicBool = AtomicBool::new(true);

pub fn set_color_enabled(enabled: bool) {
    COLOR_ENABLED.store(enabled, Ordering::Relaxed);
}

fn color_enabled() -> bool {
    COLOR_ENABLED.load(Ordering::Relaxed)
}

/// Returns `style` when color is enabled, otherwise a no-op style.
fn style_if(color: bool, style: Style) -> Style {
    if color { style } else { Style::new() }
}

pub struct Icons {
    pub current: &'static str,
    pub dirty: &'static str,
    pub clean: &'static str,
    pub arrow_up: &'static str,
    pub arrow_down: &'static str,
    pub rebase: &'static str,
}

impl Icons {
    pub fn unicode() -> Self {
        Self {
            current: "▶",
            dirty: "●",
            clean: "✓",
            arrow_up: "↑",
            arrow_down: "↓",
            rebase: "⟳",
        }
    }

    pub fn ascii() -> Self {
        Self {
            current: ">",
            dirty: "*",
            clean: "-",
            arrow_up: "^",
            arrow_down: "v",
            rebase: "R",
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
        .unwrap_or(10)
        .max(6); // minimum width for "BRANCH" header

    let header = format!(
        "  {:width$}  {:>6}  {:>6}  {:>5}  {:>6}  PATH",
        "BRANCH",
        "DIRTY",
        "SYNC",
        "AGE",
        "REBASE",
        width = max_name_len
    );
    println!(
        "{}",
        header.style(style_if(opts.color, Style::new().dimmed()))
    );

    for (wt, status) in worktrees {
        let is_current = wt.path == current_path;

        // Pad every column as plain text first, then style it; formatting a
        // string that already contains ANSI codes would count the codes as
        // width and misalign the table.
        let marker = if is_current { icons.current } else { " " };
        let name = format!("{:width$}", wt.name(), width = max_name_len);
        let dirty = format!("{:>6}", format_dirty(status, &icons));
        let sync = format!("{:>6}", format_sync(status, &icons));
        let age = format!("{:>5}", format_time(status.last_commit_time));
        let rebase = format!("{:>6}", format_rebase(status, &icons));

        let current_style = style_if(is_current && opts.color, Style::new().green().bold());
        let name_style = if is_current {
            current_style
        } else {
            style_if(status.is_dirty() && opts.color, Style::new().yellow())
        };
        let dirty_style = style_if(
            opts.color,
            if status.is_dirty() {
                Style::new().yellow()
            } else {
                Style::new().green()
            },
        );
        let rebase_style = style_if(opts.color && status.needs_rebase(), Style::new().red());
        let dim = style_if(opts.color, Style::new().dimmed());

        println!(
            "{} {}  {}  {}  {}  {}  {}",
            marker.style(current_style),
            name.style(name_style),
            dirty.style(dirty_style),
            sync,
            age.style(dim),
            rebase.style(rebase_style),
            shorten_path(&wt.path).style(dim)
        );
    }
}

fn format_dirty(status: &WorktreeStatus, icons: &Icons) -> String {
    if status.is_dirty() {
        format!("{} {:>3}", icons.dirty, status.dirty_count)
    } else {
        format!("{} {:>3}", icons.clean, "-")
    }
}

fn format_sync(status: &WorktreeStatus, icons: &Icons) -> String {
    match (status.ahead, status.behind) {
        (Some(a), Some(b)) => {
            format!("{}{} {}{}", icons.arrow_up, a, icons.arrow_down, b)
        }
        _ => "-".to_string(),
    }
}

pub fn format_time(seconds: Option<i64>) -> String {
    match seconds {
        Some(s) if s < 60 => "now".to_string(),
        Some(s) if s < 3600 => format!("{}m", s / 60),
        Some(s) if s < 86400 => format!("{}h", s / 3600),
        Some(s) if s < 604800 => format!("{}d", s / 86400),
        Some(s) if s < 2592000 => format!("{}w", s / 604800),
        Some(s) => format!("{}mo", s / 2592000),
        None => "-".to_string(),
    }
}

fn format_rebase(status: &WorktreeStatus, icons: &Icons) -> String {
    match status.behind_main {
        Some(n) if n > 0 => format!("{} {:>3}", icons.rebase, n),
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
struct JsonOutput {
    repo: RepoInfo,
    current: String,
    worktrees: Vec<JsonWorktree>,
}

#[derive(Serialize)]
struct RepoInfo {
    root: String,
    common_dir: String,
}

#[derive(Serialize)]
struct JsonWorktree {
    path: String,
    branch: Option<String>,
    branch_short: Option<String>,
    head: String,
    detached: bool,
    locked: bool,
    dirty_count: usize,
    upstream: Option<String>,
    ahead: Option<usize>,
    behind: Option<usize>,
    last_commit_seconds: Option<i64>,
    behind_main: Option<usize>,
}

fn print_worktree_list_json(
    repo: &GitRepo,
    worktrees: &[(Worktree, WorktreeStatus)],
    current_path: &Path,
) {
    let json_worktrees: Vec<JsonWorktree> = worktrees
        .iter()
        .map(|(wt, status)| JsonWorktree {
            path: wt.path.to_string_lossy().into_owned(),
            branch: wt.branch.clone(),
            branch_short: wt.branch_short.clone(),
            head: wt.head.clone(),
            detached: wt.detached,
            locked: wt.locked,
            dirty_count: status.dirty_count,
            upstream: status.upstream.clone(),
            ahead: status.ahead,
            behind: status.behind,
            last_commit_seconds: status.last_commit_time,
            behind_main: status.behind_main,
        })
        .collect();

    let output = JsonOutput {
        repo: RepoInfo {
            root: repo.root.to_string_lossy().into_owned(),
            common_dir: repo.common_dir.to_string_lossy().into_owned(),
        },
        current: current_path.to_string_lossy().into_owned(),
        worktrees: json_worktrees,
    };

    let json = serde_json::to_string_pretty(&output).unwrap_or_else(|_| "{}".to_string());
    println!("{}", json);
}

pub fn print_error(msg: &str, hint: Option<&str>) {
    let color = style_if(color_enabled(), Style::new().red().bold());
    let hint_color = style_if(color_enabled(), Style::new().cyan());

    let stderr = io::stderr();
    let mut handle = stderr.lock();

    let _ = writeln!(handle, "{}: {}", "error".style(color), msg);
    if let Some(h) = hint {
        let _ = writeln!(handle, "{}: {}", "hint".style(hint_color), h);
    }
}

pub fn print_success(msg: &str) {
    let style = style_if(color_enabled(), Style::new().green().bold());
    eprintln!("{}: {}", "success".style(style), msg);
}

pub fn print_warning(msg: &str) {
    let style = style_if(color_enabled(), Style::new().yellow().bold());
    eprintln!("{}: {}", "warning".style(style), msg);
}

pub fn print_info(msg: &str) {
    eprintln!("{}", msg);
}
