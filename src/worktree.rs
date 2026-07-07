use crate::git::GitRepo;
use anyhow::{Context, Result};
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
    let mut worktrees = Vec::new();

    // 1. Linked Worktrees
    let worktree_names = git_repo.worktrees().context("Failed to list worktrees")?;
    for name in worktree_names.iter().filter_map(|n| n.ok().flatten()) {
        // git2::Worktree structure
        let wt = git_repo.find_worktree(name)?;
        let path = wt.path().to_path_buf();

        let should_prune = wt.is_prunable(None).unwrap_or(false);
        let is_locked = matches!(wt.is_locked(), Ok(git2::WorktreeLockStatus::Locked(_)));

        // If prunable, we might not be able to open it
        if should_prune {
            worktrees.push(Worktree {
                path,
                head: String::new(),
                branch: None,
                branch_short: None,
                detached: false,
                locked: is_locked,
                prunable: true,
            });
            continue;
        }

        // Try to open repo to get head info
        // Note: Repository::open on a worktree path opens the worktree context
        match git2::Repository::open(&path) {
            Ok(wt_repo) => {
                let (head, branch, branch_short, detached) = get_repo_head_info(&wt_repo);
                worktrees.push(Worktree {
                    path,
                    head,
                    branch,
                    branch_short,
                    detached,
                    locked: is_locked,
                    prunable: false,
                });
            }
            Err(_) => {
                // Could not open, maybe permissions or broken
                worktrees.push(Worktree {
                    path,
                    head: String::new(),
                    branch: None,
                    branch_short: None,
                    detached: false,
                    locked: is_locked,
                    prunable: true, // Treat as broken
                });
            }
        }
    }

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
