use crate::git::GitRepo;
use anyhow::{Context, Result};
use rayon::prelude::*;
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub struct Worktree {
    pub path: PathBuf,
    pub head: String,
    pub branch: Option<String>,
    pub branch_short: Option<String>,
    pub detached: bool,
    pub locked: bool,
    pub prunable: bool,
}

impl Worktree {
    pub fn name(&self) -> &str {
        self.branch_short
            .as_deref()
            .or_else(|| self.path.file_name().and_then(|s| s.to_str()))
            .unwrap_or("unknown")
    }

    pub fn is_main_worktree(&self, repo: &GitRepo) -> bool {
        // The main worktree is the parent of the shared .git directory.
        repo.common_dir
            .parent()
            .is_some_and(|main_root| same_path(&self.path, main_root))
    }
}

pub fn same_path(p1: &Path, p2: &Path) -> bool {
    match (p1.canonicalize(), p2.canonicalize()) {
        (Ok(c1), Ok(c2)) => c1 == c2,
        _ => false, // If either fails to canonicalize, they're not the same
    }
}

pub fn list_worktrees(repo: &GitRepo) -> Result<Vec<Worktree>> {
    let git_repo = &repo.repo;

    // 1. Linked worktrees: gather cheap metadata under the main repo handle,
    // then open each worktree repository (the slow part) in parallel.
    let worktree_names = git_repo.worktrees().context("Failed to list worktrees")?;
    let mut entries = Vec::new();
    for name in worktree_names.iter().filter_map(|n| n.ok().flatten()) {
        let wt = git_repo.find_worktree(name)?;
        let prunable = wt.is_prunable(None).unwrap_or(false);
        let locked = matches!(wt.is_locked(), Ok(git2::WorktreeLockStatus::Locked(_)));
        entries.push((wt.path().to_path_buf(), locked, prunable));
    }

    let mut worktrees: Vec<Worktree> = entries
        .into_par_iter()
        .map(|(path, locked, prunable)| {
            let broken = Worktree {
                path: path.clone(),
                head: String::new(),
                branch: None,
                branch_short: None,
                detached: false,
                locked,
                prunable: true,
            };

            // A prunable worktree usually can't be opened; a failed open is
            // treated the same way.
            if prunable {
                return broken;
            }
            match git2::Repository::open(&path) {
                Ok(wt_repo) => {
                    let (head, branch, branch_short, detached) = get_repo_head_info(&wt_repo);
                    Worktree {
                        path,
                        head,
                        branch,
                        branch_short,
                        detached,
                        locked,
                        prunable: false,
                    }
                }
                Err(_) => broken,
            }
        })
        .collect();

    // 2. Main Worktree (the parent of the shared .git dir; never in the linked list)
    let common_dir = git_repo.commondir();
    let main_path = common_dir.parent().unwrap_or(common_dir);

    if let Ok(main_repo) = git2::Repository::open(main_path) {
        if !worktrees.iter().any(|w| same_path(&w.path, main_path)) {
            let (head, branch, branch_short, detached) = get_repo_head_info(&main_repo);
            worktrees.push(Worktree {
                path: main_path.to_path_buf(),
                head,
                branch,
                branch_short,
                detached,
                locked: false,
                prunable: false,
            });
        }
    }

    Ok(worktrees)
}

fn get_repo_head_info(repo: &git2::Repository) -> (String, Option<String>, Option<String>, bool) {
    let head_ref = repo.head();
    match head_ref {
        Ok(r) => {
            let head_oid = r.target().map(|o| o.to_string()).unwrap_or_default();
            let detached = repo.head_detached().unwrap_or(false);
            let name = r.name().ok().map(|s| s.to_string());

            if detached {
                (head_oid, None, None, true)
            } else {
                let shorthand = r.shorthand().ok().map(|s| s.to_string());
                (head_oid, name, shorthand, false)
            }
        }
        Err(_) => (String::new(), None, None, false), // empty repo?
    }
}

pub fn find_worktree<'a>(worktrees: &'a [Worktree], name: &str) -> Option<&'a Worktree> {
    worktrees.iter().find(|worktree| {
        worktree.branch_short.as_deref() == Some(name)
            || worktree.path.file_name().and_then(|s| s.to_str()) == Some(name)
    })
}

pub fn slug_from_branch(branch: &str) -> String {
    branch
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slug_from_branch() {
        assert_eq!(slug_from_branch("feat/login"), "feat-login");
        assert_eq!(slug_from_branch("fix/bug-123"), "fix-bug-123");
        assert_eq!(
            slug_from_branch("feature/add user auth"),
            "feature-add-user-auth"
        );
    }
}
