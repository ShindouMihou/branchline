use super::super::{
    command_details,
    commands::{self, CommandStep},
};
use super::*;
use std::process::Command;

fn repository() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    assert!(Command::new("git")
        .args(["init", "--quiet", "--initial-branch=main"])
        .current_dir(root.path())
        .status()
        .unwrap()
        .success());
    root
}

fn command(script: &str) -> CommandStep {
    CommandStep {
        program: "shell".into(),
        argv: vec!["-c".into(), script.into()],
        stdin: None,
        timeout_seconds: None,
    }
}

#[test]
fn current_index_and_projected_worktree_are_distinct_for_partially_staged_files() {
    let root = repository();
    fs::write(root.path().join("partial.txt"), "staged text\n").unwrap();
    assert!(Command::new("git")
        .args(["add", "--", "partial.txt"])
        .current_dir(root.path())
        .status()
        .unwrap()
        .success());
    fs::write(root.path().join("partial.txt"), "working text\n").unwrap();
    let commands = vec![command(
        "git commit -m Commit; git add partial.txt; git commit -m Later",
    )];
    let manifest = commands::preview(root.path(), &commands).unwrap();
    let details = command_details::inspect(root.path(), &commands).unwrap();
    let staged = inspect(root.path(), &details[0], "partial.txt", &manifest).unwrap();
    let to_stage = inspect(root.path(), &details[1], "partial.txt", &manifest).unwrap();
    let projected = inspect(root.path(), &details[2], "partial.txt", &manifest).unwrap();
    assert_eq!(staged["source"], "index");
    assert_eq!(to_stage["baseline"], "index");
    assert_eq!(to_stage["before"], "staged text\n");
    assert_eq!(to_stage["after"], "working text\n");
    assert!(staged["before"].is_null());
    assert_eq!(staged["after"], "staged text\n");
    assert!(to_stage["patch"].as_str().unwrap().contains("-staged text"));
    assert!(to_stage["patch"].as_str().unwrap().contains("+working text"));
    assert!(staged["patch"].as_str().unwrap().contains("+staged text"));
    assert!(!staged["patch"].as_str().unwrap().contains("working text"));
    assert_eq!(projected["source"], "worktree");
    assert!(projected["patch"]
        .as_str()
        .unwrap()
        .contains("+working text"));
    assert_ne!(staged["snapshotId"], projected["snapshotId"]);
    assert!(projected["notice"]
        .as_str()
        .unwrap()
        .contains("not a frozen approval"));
    assert!(
        Command::new("git")
            .args(["diff", "--cached", "--exit-code", "--", "partial.txt"])
            .current_dir(root.path())
            .output()
            .unwrap()
            .status
            .code()
            == Some(1)
    );
}

#[test]
fn untracked_deleted_and_binary_current_snapshots_are_read_only() {
    let root = repository();
    fs::write(root.path().join("new.txt"), "new line\n").unwrap();
    fs::write(root.path().join("binary.bin"), [0, 1, 2]).unwrap();
    let commands = vec![command("git add . && git commit -m New")];
    let manifest = commands::preview(root.path(), &commands).unwrap();
    let details = command_details::inspect(root.path(), &commands).unwrap();
    let new = inspect(root.path(), &details[0], "new.txt", &manifest).unwrap();
    assert_eq!(new["added"], 1);
    assert_eq!(new["removed"], 0);
    let binary = inspect(root.path(), &details[0], "binary.bin", &manifest).unwrap();
    assert_eq!(binary["patch"], "");
    assert!(binary["notice"].as_str().unwrap().contains("Binary"));
    fs::remove_file(root.path().join("new.txt")).unwrap();
    let deleted = inspect(root.path(), &details[0], "new.txt", &manifest).unwrap();
    assert_eq!(deleted["patch"], "");
    assert_ne!(new["snapshotId"], deleted["snapshotId"]);
    let index = Command::new("git")
        .args(["ls-files", "-z"])
        .current_dir(root.path())
        .output()
        .unwrap();
    assert!(index.stdout.is_empty());
}

#[test]
fn only_listed_files_are_read_and_symlink_targets_are_refused() {
    let root = repository();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("secret.txt"), "private outside data").unwrap();
    fs::write(root.path().join("listed.txt"), "listed").unwrap();
    let commands = vec![command("git add listed.txt")];
    let manifest = commands::preview(root.path(), &commands).unwrap();
    let details = command_details::inspect(root.path(), &commands).unwrap();
    assert!(inspect(root.path(), &details[0], "../secret.txt", &manifest).is_err());
    assert!(inspect(root.path(), &details[0], "unlisted.txt", &manifest).is_err());
    fs::remove_file(root.path().join("listed.txt")).unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("secret.txt"),
        root.path().join("listed.txt"),
    )
    .unwrap();
    assert!(inspect(root.path(), &details[0], "listed.txt", &manifest).is_err());
    let mut nested = details[0].clone();
    nested.files = Some(vec!["nested/secret.txt".into()]);
    nested.file_previews = vec![command_details::FileSelection {
        path: "nested/secret.txt".into(),
        source: FileSource::Worktree,
    }];
    std::os::unix::fs::symlink(outside.path(), root.path().join("nested")).unwrap();
    assert!(inspect(root.path(), &nested, "nested/secret.txt", &manifest).is_err());
}

#[test]
fn counts_content_that_resembles_diff_headers() {
    let root = repository();
    fs::write(root.path().join("headers.txt"), "++ actual content\n-- also content\n").unwrap();
    let steps = vec![command("git add -- headers.txt")];
    let manifest = commands::preview(root.path(), &steps).unwrap();
    let details = command_details::inspect(root.path(), &steps).unwrap();
    let result = inspect(root.path(), &details[0], "headers.txt", &manifest).unwrap();
    assert_eq!(result["added"], 2);
    assert_eq!(result["removed"], 0);
    assert!(result["patch"].as_str().unwrap().contains("+++ actual content"));
}
