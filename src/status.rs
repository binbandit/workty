use crate::git::GitRepo;
use crate::worktree::Worktree;
use rayon::prelude::*;
use serde::Serialize;

#[derive(Debug, Clone, Default, Serialize)]
pub struct WorktreeStatus {
    pub dirty_count: usize,
    pub upstream: Option<String>,
    pub ahead: Option<usize>,
    pub behind: Option<usize>,
    /// Seconds since last commit (HEAD)
    pub last_commit_time: Option<i64>,
    /// Behind count relative to origin/main or origin/master
    pub behind_main: Option<usize>,
    /// Number of commits with no upstream tracking (unpushed branch)
    pub untracked_commits: Option<usize>,
    /// True if upstream branch has been deleted on remote
    pub upstream_gone: bool,
}

impl WorktreeStatus {
    pub fn is_dirty(&self) -> bool {
        self.dirty_count > 0
    }

    /// Returns true if this branch needs rebasing onto main
    pub fn needs_rebase(&self) -> bool {
        self.behind_main.is_some_and(|b| b > 0)
    }
}

/// Compute the status of every worktree in parallel.
pub fn get_all_statuses(repo: &GitRepo, worktrees: &[Worktree]) -> Vec<(Worktree, WorktreeStatus)> {
    collect_statuses(repo, worktrees, true)
}

/// Like [`get_all_statuses`] but skips the expensive dirty-file check.
pub fn get_all_statuses_fast(
    repo: &GitRepo,
    worktrees: &[Worktree],
) -> Vec<(Worktree, WorktreeStatus)> {
    collect_statuses(repo, worktrees, false)
}

fn collect_statuses(
    repo: &GitRepo,
    worktrees: &[Worktree],
    check_dirty: bool,
) -> Vec<(Worktree, WorktreeStatus)> {
    // Pre-compute the main branch OID once; it's shared by every worktree.
    let main_oid = get_main_branch_oid(repo);

    worktrees
        .par_iter()
        .map(|worktree| {
            let status = compute_status(worktree, main_oid, check_dirty);
            (worktree.clone(), status)
        })
        .collect()
}

fn get_main_branch_oid(repo: &GitRepo) -> Option<git2::Oid> {
    repo.repo
        .find_reference("refs/remotes/origin/main")
        .or_else(|_| repo.repo.find_reference("refs/remotes/origin/master"))
        .ok()?
        .target()
}

fn compute_status(
    worktree: &Worktree,
    main_oid: Option<git2::Oid>,
    check_dirty: bool,
) -> WorktreeStatus {
    let wt_repo = match git2::Repository::open(&worktree.path) {
        Ok(r) => r,
        Err(_) => return WorktreeStatus::default(),
    };

    let dirty_count = if check_dirty {
        get_dirty_count(&wt_repo)
    } else {
        0
    };
    let (upstream, ahead, behind, upstream_gone) = get_ahead_behind(&wt_repo, worktree);
    let last_commit_time = get_last_commit_time(&wt_repo);

    let head_oid = wt_repo.head().ok().and_then(|h| h.target());
    let (behind_main, untracked_commits) = match (main_oid, head_oid) {
        (Some(main_oid), Some(head_oid)) => {
            let (ahead_of_main, behind_of_main) = wt_repo
                .graph_ahead_behind(head_oid, main_oid)
                .unwrap_or((0, 0));

            let untracked = if upstream.is_none() && !worktree.detached {
                Some(ahead_of_main)
            } else {
                None
            };

            (Some(behind_of_main), untracked)
        }
        _ => (None, None),
    };

    WorktreeStatus {
        dirty_count,
        upstream,
        ahead,
        behind,
        last_commit_time,
        behind_main,
        untracked_commits,
        upstream_gone,
    }
}

fn get_dirty_count(repo: &git2::Repository) -> usize {
    let mut opts = git2::StatusOptions::new();
    opts.include_untracked(true)
        .recurse_untracked_dirs(false) // Don't recurse - much faster
        .exclude_submodules(true)
        .no_refresh(true); // Don't refresh index from disk

    match repo.statuses(Some(&mut opts)) {
        Ok(statuses) => statuses.len(),
        Err(_) => 0,
    }
}

fn get_ahead_behind(
    repo: &git2::Repository,
    worktree: &Worktree,
) -> (Option<String>, Option<usize>, Option<usize>, bool) {
    if worktree.detached {
        return (None, None, None, false);
    }

    let head = match repo.head() {
        Ok(h) if h.is_branch() => h,
        _ => return (None, None, None, false),
    };

    let head_refname = match head.name() {
        Some(name) => name,
        None => return (None, None, None, false),
    };

    // Resolve the upstream from config rather than the ref itself: when the
    // remote branch has been deleted (and pruned), the ref is gone but the
    // config remains, which is exactly how we detect a "gone" upstream.
    let upstream_ref = match repo.branch_upstream_name(head_refname) {
        Ok(buf) => match buf.as_str() {
            Some(s) => s.to_string(),
            None => return (None, None, None, false),
        },
        Err(_) => return (None, None, None, false), // No upstream configured
    };

    let upstream_name = upstream_ref
        .strip_prefix("refs/remotes/")
        .unwrap_or(&upstream_ref)
        .to_string();

    let local_oid = match head.target() {
        Some(oid) => oid,
        None => return (Some(upstream_name), None, None, false),
    };

    let upstream_oid = match repo.find_reference(&upstream_ref).ok().and_then(|r| r.target()) {
        Some(oid) => oid,
        // Upstream is configured but its ref no longer exists - it was deleted
        None => return (Some(upstream_name), None, None, true),
    };

    match repo.graph_ahead_behind(local_oid, upstream_oid) {
        Ok((ahead, behind)) => (Some(upstream_name), Some(ahead), Some(behind), false),
        Err(_) => (Some(upstream_name), None, None, false),
    }
}

fn get_last_commit_time(repo: &git2::Repository) -> Option<i64> {
    let commit = repo.head().ok()?.peel_to_commit().ok()?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs() as i64;
    Some(now - commit.time().seconds())
}

pub fn is_worktree_dirty(worktree: &Worktree) -> bool {
    match git2::Repository::open(&worktree.path) {
        Ok(repo) => get_dirty_count(&repo) > 0,
        Err(_) => false,
    }
}
