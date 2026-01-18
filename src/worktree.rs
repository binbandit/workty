use crate::git::GitRepo;
use anyhow::{Context, Result};
use serde::Serialize;
use std::path::PathBuf;

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
        self.path == repo.root
    }
}

pub fn parse_worktree_list(output: &str) -> Vec<Worktree> {
    let mut worktrees = Vec::new();
    let mut current: Option<WorktreeBuilder> = None;

    for line in output.lines() {
        if line.is_empty() {
            if let Some(builder) = current.take() {
                if let Some(worktree) = builder.build() {
                    worktrees.push(worktree);
                }
            }
            continue;
        }

        if let Some((key, value)) = line.split_once(' ') {
            let builder = current.get_or_insert_with(WorktreeBuilder::default);
            match key {
                "worktree" => builder.path = Some(PathBuf::from(value)),
                "HEAD" => builder.head = Some(value.to_string()),
                "branch" => builder.branch = Some(value.to_string()),
                "detached" => builder.detached = true,
                "locked" => builder.locked = true,
                "prunable" => builder.prunable = true,
                _ => {}
            }
        } else {
            let builder = current.get_or_insert_with(WorktreeBuilder::default);
            match line {
                "detached" => builder.detached = true,
                "locked" => builder.locked = true,
                "prunable" => builder.prunable = true,
                "bare" => builder.bare = true,
                _ => {}
            }
        }
    }

    if let Some(builder) = current {
        if let Some(worktree) = builder.build() {
            worktrees.push(worktree);
        }
    }

    worktrees
}

#[derive(Default)]
struct WorktreeBuilder {
    path: Option<PathBuf>,
    head: Option<String>,
    branch: Option<String>,
    detached: bool,
    locked: bool,
    prunable: bool,
    bare: bool,
}

impl WorktreeBuilder {
    fn build(self) -> Option<Worktree> {
        let path = self.path?;
        let head = self.head.unwrap_or_default();

        if self.bare {
            return None;
        }

        let branch_short = self
            .branch
            .as_ref()
            .map(|b| b.strip_prefix("refs/heads/").unwrap_or(b).to_string());

        Some(Worktree {
            path,
            head,
            branch: self.branch,
            branch_short,
            detached: self.detached,
            locked: self.locked,
            prunable: self.prunable,
        })
    }
}

pub fn list_worktrees(repo: &GitRepo) -> Result<Vec<Worktree>> {
    let output = repo
        .run_git(&["worktree", "list", "--porcelain"])
        .context("Failed to list worktrees")?;
    Ok(parse_worktree_list(&output))
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
    fn test_parse_worktree_list_normal() {
        let output = r#"worktree /home/user/project
HEAD abc123def456
branch refs/heads/main

worktree /home/user/.workty/project/feat-login
HEAD def789abc012
branch refs/heads/feat/login

"#;
        let worktrees = parse_worktree_list(output);
        assert_eq!(worktrees.len(), 2);
        assert_eq!(worktrees[0].branch_short.as_deref(), Some("main"));
        assert_eq!(worktrees[1].branch_short.as_deref(), Some("feat/login"));
        assert!(!worktrees[0].detached);
    }

    #[test]
    fn test_parse_worktree_list_detached() {
        let output = r#"worktree /home/user/project
HEAD abc123def456
detached

"#;
        let worktrees = parse_worktree_list(output);
        assert_eq!(worktrees.len(), 1);
        assert!(worktrees[0].detached);
        assert!(worktrees[0].branch.is_none());
    }

    #[test]
    fn test_parse_worktree_list_locked() {
        let output = r#"worktree /home/user/project
HEAD abc123def456
branch refs/heads/main
locked reason here

"#;
        let worktrees = parse_worktree_list(output);
        assert_eq!(worktrees.len(), 1);
        assert!(worktrees[0].locked);
    }

    #[test]
    fn test_parse_worktree_list_bare() {
        let output = r#"worktree /home/user/project.git
bare

worktree /home/user/project
HEAD abc123
branch refs/heads/main

"#;
        let worktrees = parse_worktree_list(output);
        assert_eq!(worktrees.len(), 1);
        assert_eq!(worktrees[0].branch_short.as_deref(), Some("main"));
    }

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
