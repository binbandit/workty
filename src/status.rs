use crate::git::GitRepo;
use crate::worktree::Worktree;
use anyhow::Result;
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

    #[allow(dead_code)]
    pub fn has_upstream(&self) -> bool {
        self.upstream.is_some()
    }

    /// Returns true if this branch needs rebasing onto main
    pub fn needs_rebase(&self) -> bool {
        self.behind_main.map(|b| b > 0).unwrap_or(false)
    }

    /// Returns true if there are commits that haven't been pushed
    pub fn has_unpushed(&self) -> bool {
        // Commits ahead of upstream
        if self.ahead.map(|a| a > 0).unwrap_or(false) {
            return true;
        }
        // Branch with no upstream but has commits
        if self.upstream.is_none() && self.untracked_commits.map(|c| c > 0).unwrap_or(false) {
            return true;
        }
        false
    }

    /// Returns the number of unpushed commits
    pub fn unpushed_count(&self) -> usize {
        if let Some(ahead) = self.ahead {
            if ahead > 0 {
                return ahead;
            }
        }
        self.untracked_commits.unwrap_or(0)
    }
}

pub fn get_worktree_status(repo: &GitRepo, worktree: &Worktree) -> WorktreeStatus {
    // Open the worktree repo once and reuse it for all status queries
    let wt_repo = match git2::Repository::open(&worktree.path) {
        Ok(r) => r,
        Err(_) => {
            return WorktreeStatus::default();
        }
    };

    let dirty_count = get_dirty_count(&wt_repo);
    let (upstream, ahead, behind, upstream_gone) = get_ahead_behind(&wt_repo, worktree);
    let last_commit_time = get_last_commit_time(&wt_repo);
    let behind_main = get_behind_main(&wt_repo, repo);
    let untracked_commits = if upstream.is_none() && !worktree.detached {
        get_untracked_commit_count(&wt_repo, repo)
    } else {
        None
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
    // Use git2's status API with optimizations for speed
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
    let head = repo.head().ok()?;
    let commit = head.peel_to_commit().ok()?;
    let time = commit.time();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs() as i64;
    Some(now - time.seconds())
}

fn get_behind_main(wt_repo: &git2::Repository, main_repo: &GitRepo) -> Option<usize> {
    // Get HEAD of this worktree
    let head = wt_repo.head().ok()?;
    let head_oid = head.target()?;

    // Find origin/main or origin/master in the main repo
    let main_repo_lock = main_repo.repo.lock().ok()?;

    let main_oid = main_repo_lock
        .find_reference("refs/remotes/origin/main")
        .or_else(|_| main_repo_lock.find_reference("refs/remotes/origin/master"))
        .ok()?
        .target()?;

    // Calculate how far behind main this branch is
    // We need to use the worktree repo for the graph calculation
    // but the OIDs should be the same across repos sharing the same object store
    match wt_repo.graph_ahead_behind(head_oid, main_oid) {
        Ok((_ahead, behind)) => Some(behind),
        Err(_) => None,
    }
}

fn get_untracked_commit_count(wt_repo: &git2::Repository, main_repo: &GitRepo) -> Option<usize> {
    // Count commits on this branch that aren't on origin/main or origin/master
    // This is for branches with no upstream set
    let head = wt_repo.head().ok()?;
    let head_oid = head.target()?;

    let main_repo_lock = main_repo.repo.lock().ok()?;

    let main_oid = main_repo_lock
        .find_reference("refs/remotes/origin/main")
        .or_else(|_| main_repo_lock.find_reference("refs/remotes/origin/master"))
        .ok()?
        .target()?;

    match wt_repo.graph_ahead_behind(head_oid, main_oid) {
        Ok((ahead, _behind)) => Some(ahead),
        Err(_) => None,
    }
}

pub fn get_all_statuses(repo: &GitRepo, worktrees: &[Worktree]) -> Vec<(Worktree, WorktreeStatus)> {
    // Pre-compute main branch OID once for all worktrees
    let main_oid = get_main_branch_oid(repo);

    worktrees
        .par_iter()
        .map(|worktree| {
            let status = get_worktree_status_full(worktree, main_oid);
            (worktree.clone(), status)
        })
        .collect()
}

/// Fast version that skips the expensive dirty file check
pub fn get_all_statuses_fast(
    repo: &GitRepo,
    worktrees: &[Worktree],
) -> Vec<(Worktree, WorktreeStatus)> {
    let main_oid = get_main_branch_oid(repo);

    worktrees
        .par_iter()
        .map(|worktree| {
            let status = get_worktree_status_minimal(worktree, main_oid);
            (worktree.clone(), status)
        })
        .collect()
}

fn get_main_branch_oid(repo: &GitRepo) -> Option<git2::Oid> {
    let repo_lock = repo.repo.lock().ok()?;
    let reference = repo_lock
        .find_reference("refs/remotes/origin/main")
        .or_else(|_| repo_lock.find_reference("refs/remotes/origin/master"))
        .ok()?;
    reference.target()
}

fn get_worktree_status_full(worktree: &Worktree, main_oid: Option<git2::Oid>) -> WorktreeStatus {
    // Open the worktree repo once and reuse it for all status queries
    let wt_repo = match git2::Repository::open(&worktree.path) {
        Ok(r) => r,
        Err(_) => {
            return WorktreeStatus::default();
        }
    };

    let dirty_count = get_dirty_count(&wt_repo);
    let (upstream, ahead, behind, upstream_gone) = get_ahead_behind(&wt_repo, worktree);
    let last_commit_time = get_last_commit_time(&wt_repo);

    // Use pre-computed main_oid for faster calculation
    let (behind_main, untracked_commits) = if let Some(main_oid) = main_oid {
        let head_oid = wt_repo.head().ok().and_then(|h| h.target());
        if let Some(head_oid) = head_oid {
            let (ahead_of_main, behind_of_main) = wt_repo
                .graph_ahead_behind(head_oid, main_oid)
                .unwrap_or((0, 0));

            let untracked = if upstream.is_none() && !worktree.detached {
                Some(ahead_of_main)
            } else {
                None
            };

            (Some(behind_of_main), untracked)
        } else {
            (None, None)
        }
    } else {
        (None, None)
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

/// Minimal status - skips expensive dirty check for fast dashboard
fn get_worktree_status_minimal(worktree: &Worktree, main_oid: Option<git2::Oid>) -> WorktreeStatus {
    let wt_repo = match git2::Repository::open(&worktree.path) {
        Ok(r) => r,
        Err(_) => {
            return WorktreeStatus::default();
        }
    };

    // Skip dirty check - it's the expensive part
    let (upstream, ahead, behind, upstream_gone) = get_ahead_behind(&wt_repo, worktree);
    let last_commit_time = get_last_commit_time(&wt_repo);

    let (behind_main, untracked_commits) = if let Some(main_oid) = main_oid {
        let head_oid = wt_repo.head().ok().and_then(|h| h.target());
        if let Some(head_oid) = head_oid {
            let (ahead_of_main, behind_of_main) = wt_repo
                .graph_ahead_behind(head_oid, main_oid)
                .unwrap_or((0, 0));

            let untracked = if upstream.is_none() && !worktree.detached {
                Some(ahead_of_main)
            } else {
                None
            };

            (Some(behind_of_main), untracked)
        } else {
            (None, None)
        }
    } else {
        (None, None)
    };

    WorktreeStatus {
        dirty_count: 0, // Skip dirty check in fast mode
        upstream,
        ahead,
        behind,
        last_commit_time,
        behind_main,
        untracked_commits,
        upstream_gone,
    }
}

pub fn is_worktree_dirty(worktree: &Worktree) -> bool {
    match git2::Repository::open(&worktree.path) {
        Ok(repo) => get_dirty_count(&repo) > 0,
        Err(_) => false,
    }
}

#[allow(dead_code)]
pub fn check_branch_merged(repo: &GitRepo, branch: &str, base: &str) -> Result<bool> {
    repo.is_merged(branch, base)
}
