use crate::git::GitRepo;
use crate::worktree::Worktree;
use anyhow::Result;
use serde::Serialize;
use std::process::Command;

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

pub fn get_worktree_status(repo: &GitRepo, worktree: &Worktree) -> WorktreeStatus {
    let dirty_count = get_dirty_count(&worktree.path);
    let (upstream, ahead, behind) = get_ahead_behind(repo, worktree);

    WorktreeStatus {
        dirty_count,
        upstream,
        ahead,
        behind,
    }
}

fn get_dirty_count(worktree_path: &std::path::Path) -> usize {
    let output = Command::new("git")
        .current_dir(worktree_path)
        .args(["status", "--porcelain"])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            String::from_utf8_lossy(&out.stdout)
                .lines()
                .filter(|line| !line.is_empty())
                .count()
        }
        _ => 0,
    }
}

fn get_ahead_behind(
    _repo: &GitRepo,
    worktree: &Worktree,
) -> (Option<String>, Option<usize>, Option<usize>) {
    if worktree.detached {
        return (None, None, None);
    }

    let upstream = Command::new("git")
        .current_dir(&worktree.path)
        .args(["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"])
        .output();

    let upstream = match upstream {
        Ok(out) if out.status.success() => {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if s.is_empty() { None } else { Some(s) }
        }
        _ => return (None, None, None),
    };

    let output = Command::new("git")
        .current_dir(&worktree.path)
        .args(["rev-list", "--left-right", "--count", "HEAD...@{u}"])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let s = String::from_utf8_lossy(&out.stdout);
            let parts: Vec<&str> = s.trim().split('\t').collect();
            if parts.len() == 2 {
                let ahead = parts[0].parse().ok();
                let behind = parts[1].parse().ok();
                (upstream, ahead, behind)
            } else {
                (upstream, None, None)
            }
        }
        _ => (upstream, None, None),
    }
}

pub fn get_all_statuses(repo: &GitRepo, worktrees: &[Worktree]) -> Vec<(Worktree, WorktreeStatus)> {
    worktrees
        .iter()
        .map(|wt| {
            let status = get_worktree_status(repo, wt);
            (wt.clone(), status)
        })
        .collect()
}

pub fn is_worktree_dirty(worktree: &Worktree) -> bool {
    get_dirty_count(&worktree.path) > 0
}

#[allow(dead_code)]
pub fn check_branch_merged(repo: &GitRepo, branch: &str, base: &str) -> Result<bool> {
    crate::git::is_ancestor(repo, branch, base)
}
