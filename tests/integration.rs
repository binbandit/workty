use std::process::Command;
use tempfile::TempDir;

fn git(dir: &std::path::Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .expect("Failed to execute git");
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn git_init_repo(dir: &std::path::Path) {
    git(dir, &["init"]);
    git(dir, &["config", "user.email", "test@test.com"]);
    git(dir, &["config", "user.name", "Test User"]);

    std::fs::write(dir.join("README.md"), "# Test Repo\n").unwrap();
    git(dir, &["add", "."]);
    git(dir, &["commit", "-m", "Initial commit"]);
}

fn workty(dir: &std::path::Path, args: &[&str]) -> std::process::Output {
    let binary = env!("CARGO_BIN_EXE_git-workty");
    Command::new(binary)
        .current_dir(dir)
        .args(args)
        .output()
        .expect("Failed to execute git-workty")
}

fn workty_success(dir: &std::path::Path, args: &[&str]) -> String {
    let output = workty(dir, args);
    assert!(
        output.status.success(),
        "Command failed: {:?}\nstdout: {}\nstderr: {}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn test_list_shows_main_worktree() {
    let temp = TempDir::new().unwrap();
    let repo_dir = temp.path();

    git_init_repo(repo_dir);

    let output = workty_success(repo_dir, &["list", "--no-color"]);

    assert!(
        output.contains("master") || output.contains("main"),
        "Output should contain main/master branch: {}",
        output
    );
}

#[test]
fn test_new_creates_worktree() {
    let temp = TempDir::new().unwrap();
    let repo_dir = temp.path();

    git_init_repo(repo_dir);

    let output = workty_success(repo_dir, &["new", "feat/test", "--print-path"]);
    let worktree_path = output.trim();

    assert!(
        std::path::Path::new(worktree_path).exists(),
        "Worktree path should exist: {}",
        worktree_path
    );

    let list_output = workty_success(repo_dir, &["list", "--no-color"]);
    assert!(
        list_output.contains("feat/test"),
        "List should show new worktree: {}",
        list_output
    );
}

#[test]
fn test_go_returns_path() {
    let temp = TempDir::new().unwrap();
    let repo_dir = temp.path();

    git_init_repo(repo_dir);

    let new_output = workty_success(repo_dir, &["new", "test-branch", "--print-path"]);
    let expected_path = new_output.trim();

    let go_output = workty_success(repo_dir, &["go", "test-branch"]);
    let actual_path = go_output.trim();

    assert_eq!(
        expected_path, actual_path,
        "go should return the same path as new"
    );
}

#[test]
fn test_list_json_output() {
    let temp = TempDir::new().unwrap();
    let repo_dir = temp.path();

    git_init_repo(repo_dir);

    let output = workty_success(repo_dir, &["list", "--json"]);

    let parsed: serde_json::Value =
        serde_json::from_str(&output).expect("Output should be valid JSON");

    assert!(parsed.get("repo").is_some(), "JSON should have repo field");
    assert!(
        parsed.get("worktrees").is_some(),
        "JSON should have worktrees field"
    );
}

#[test]
fn test_dirty_detection() {
    let temp = TempDir::new().unwrap();
    let repo_dir = temp.path();

    git_init_repo(repo_dir);

    workty_success(repo_dir, &["new", "dirty-test", "--print-path"]);

    let list_clean = workty_success(repo_dir, &["list", "--json"]);
    let parsed_clean: serde_json::Value = serde_json::from_str(&list_clean).unwrap();

    let worktrees = parsed_clean["worktrees"].as_array().unwrap();
    let dirty_wt = worktrees
        .iter()
        .find(|wt| wt["branch_short"].as_str() == Some("dirty-test"))
        .expect("Should find dirty-test worktree");

    assert_eq!(
        dirty_wt["dirty"]["count"].as_u64(),
        Some(0),
        "Should be clean initially"
    );

    let go_output = workty_success(repo_dir, &["go", "dirty-test"]);
    let wt_path = std::path::Path::new(go_output.trim());
    std::fs::write(wt_path.join("new-file.txt"), "dirty content").unwrap();

    let list_dirty = workty_success(repo_dir, &["list", "--json"]);
    let parsed_dirty: serde_json::Value = serde_json::from_str(&list_dirty).unwrap();

    let worktrees_dirty = parsed_dirty["worktrees"].as_array().unwrap();
    let dirty_wt_after = worktrees_dirty
        .iter()
        .find(|wt| wt["branch_short"].as_str() == Some("dirty-test"))
        .expect("Should find dirty-test worktree");

    assert!(
        dirty_wt_after["dirty"]["count"].as_u64().unwrap() > 0,
        "Should detect dirty state"
    );
}

#[test]
fn test_rm_refuses_dirty_without_force() {
    let temp = TempDir::new().unwrap();
    let repo_dir = temp.path();

    git_init_repo(repo_dir);

    let new_output = workty_success(repo_dir, &["new", "to-remove", "--print-path"]);
    let wt_path = std::path::Path::new(new_output.trim());

    std::fs::write(wt_path.join("dirty.txt"), "uncommitted").unwrap();

    let rm_output = workty(repo_dir, &["rm", "to-remove", "--yes"]);

    assert!(
        !rm_output.status.success(),
        "rm should fail for dirty worktree without --force"
    );

    let stderr = String::from_utf8_lossy(&rm_output.stderr);
    assert!(
        stderr.contains("uncommitted") || stderr.contains("--force"),
        "Error should mention uncommitted changes or --force: {}",
        stderr
    );
}

#[test]
fn test_rm_with_force() {
    let temp = TempDir::new().unwrap();
    let repo_dir = temp.path();

    git_init_repo(repo_dir);

    let new_output = workty_success(repo_dir, &["new", "force-remove", "--print-path"]);
    let wt_path = std::path::Path::new(new_output.trim());

    std::fs::write(wt_path.join("dirty.txt"), "uncommitted").unwrap();

    workty_success(repo_dir, &["rm", "force-remove", "--force", "--yes"]);

    let list_output = workty_success(repo_dir, &["list", "--no-color"]);
    assert!(
        !list_output.contains("force-remove"),
        "Worktree should be removed"
    );
}

#[test]
fn test_clean_dry_run() {
    let temp = TempDir::new().unwrap();
    let repo_dir = temp.path();

    git_init_repo(repo_dir);

    workty_success(repo_dir, &["new", "to-clean", "--print-path"]);

    let clean_output = workty_success(repo_dir, &["clean", "--dry-run"]);

    assert!(
        clean_output.contains("to-clean") || clean_output.contains("Dry run"),
        "Dry run should list candidates: {}",
        clean_output
    );

    let list_output = workty_success(repo_dir, &["list", "--no-color"]);
    assert!(
        list_output.contains("to-clean"),
        "Worktree should still exist after dry run"
    );
}

#[test]
fn test_doctor_runs() {
    let temp = TempDir::new().unwrap();
    let repo_dir = temp.path();

    git_init_repo(repo_dir);

    let output = workty(repo_dir, &["doctor"]);

    assert!(output.status.success(), "doctor should succeed");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Git installed") || stderr.contains("✓"),
        "Doctor should check Git: {}",
        stderr
    );
}

#[test]
fn test_init_generates_shell_script() {
    let temp = TempDir::new().unwrap();

    let output = workty_success(temp.path(), &["init", "zsh"]);

    assert!(output.contains("wcd"), "Init should define wcd function");
    assert!(output.contains("wnew"), "Init should define wnew function");
    assert!(output.contains("wgo"), "Init should define wgo function");
}

#[test]
fn test_completions_generates_output() {
    let temp = TempDir::new().unwrap();

    let output = workty_success(temp.path(), &["completions", "zsh"]);

    assert!(
        output.contains("git-workty") || output.contains("compdef"),
        "Should generate completion script"
    );
}

#[test]
fn test_help_contains_examples() {
    let temp = TempDir::new().unwrap();

    let output = workty_success(temp.path(), &["--help"]);

    assert!(
        output.contains("EXAMPLES"),
        "Help should contain examples: {}",
        output
    );
    assert!(
        output.contains("git workty new"),
        "Help should show new command example"
    );
}

#[test]
fn test_new_subcommand_help() {
    let temp = TempDir::new().unwrap();

    let output = workty_success(temp.path(), &["new", "--help"]);

    assert!(
        output.contains("--from"),
        "new help should show --from flag"
    );
    assert!(
        output.contains("--print-path"),
        "new help should show --print-path flag"
    );
}
