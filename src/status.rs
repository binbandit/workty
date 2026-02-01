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
}

impl WorktreeStatus {
    pub fn is_dirty(&self) -> bool {
        self.dirty_count > 0
    }

    #[allow(dead_code)]
    pub fn has_upstream(&self) -> bool {
        self.upstream.is_some()
    }
}

pub fn get_worktree_status(_repo: &GitRepo, worktree: &Worktree) -> WorktreeStatus {
    // Open the worktree repo once and reuse it for all status queries
    let wt_repo = match git2::Repository::open(&worktree.path) {
        Ok(r) => r,
        Err(_) => {
            return WorktreeStatus::default();
        }
    };

    let dirty_count = get_dirty_count(&wt_repo);
    let (upstream, ahead, behind) = get_ahead_behind(&wt_repo, worktree);

    WorktreeStatus {
        dirty_count,
        upstream,
        ahead,
        behind,
    }
}

fn get_dirty_count(repo: &git2::Repository) -> usize {
    // Use git2's status API - much faster than spawning a process
    let mut opts = git2::StatusOptions::new();
    opts.include_untracked(true)
        .recurse_untracked_dirs(true)
        .exclude_submodules(true);

    match repo.statuses(Some(&mut opts)) {
        Ok(statuses) => statuses.len(),
        Err(_) => 0,
    }
}

fn get_ahead_behind(
    repo: &git2::Repository,
    worktree: &Worktree,
) -> (Option<String>, Option<usize>, Option<usize>) {
    if worktree.detached {
        return (None, None, None);
    }

    // Get the current branch
    let head = match repo.head() {
        Ok(h) => h,
        Err(_) => return (None, None, None),
    };

    if !head.is_branch() {
        return (None, None, None);
    }

    let branch_name = match head.shorthand() {
        Some(name) => name,
        None => return (None, None, None),
    };

    // Find the local branch and its upstream
    let branch = match repo.find_branch(branch_name, git2::BranchType::Local) {
        Ok(b) => b,
        Err(_) => return (None, None, None),
    };

    let upstream_branch = match branch.upstream() {
        Ok(u) => u,
        Err(_) => return (None, None, None), // No upstream configured
    };

    let upstream_name = upstream_branch.name().ok().flatten().map(|s| s.to_string());

    // Get the OIDs for both branches
    let local_oid = match head.target() {
        Some(oid) => oid,
        None => return (upstream_name, None, None),
    };

    let upstream_oid = match upstream_branch.get().target() {
        Some(oid) => oid,
        None => return (upstream_name, None, None),
    };

    // Use git2's graph_ahead_behind - this is the key performance improvement
    match repo.graph_ahead_behind(local_oid, upstream_oid) {
        Ok((ahead, behind)) => (upstream_name, Some(ahead), Some(behind)),
        Err(_) => (upstream_name, None, None),
    }
}

pub fn get_all_statuses(repo: &GitRepo, worktrees: &[Worktree]) -> Vec<(Worktree, WorktreeStatus)> {
    worktrees
        .par_iter()
        .map(|worktree| {
            let status = get_worktree_status(repo, worktree);
            (worktree.clone(), status)
        })
        .collect()
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
