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
fn shell(text: &str) -> CommandStep {
    CommandStep {
        program: "shell".into(),
        argv: vec!["-c".into(), text.into()],
        stdin: None,
        timeout_seconds: None,
    }
}
#[test]
fn previews_current_index_and_literal_message_without_committing() {
    let root = repository();
    fs::write(root.path().join("selected.txt"), "selected").unwrap();
    assert!(Command::new("git")
        .args(["add", "--", "selected.txt"])
        .current_dir(root.path())
        .status()
        .unwrap()
        .success());
    let details = inspect(
        root.path(),
        &[shell(
            "git commit -m 'First paragraph' --message='Second paragraph'",
        )],
    )
    .unwrap();
    assert_eq!(
        details[0].message.as_deref(),
        Some("First paragraph\n\nSecond paragraph")
    );
    assert_eq!(details[0].files.as_ref().unwrap(), &vec!["selected.txt"]);
    assert!(details[0].evidence.contains("current index"));
    assert!(!Command::new("git")
        .args(["rev-parse", "--verify", "HEAD"])
        .current_dir(root.path())
        .output()
        .unwrap()
        .status
        .success());
}
#[test]
fn projects_literal_staging_and_branch_then_shows_explicit_pr_fields() {
    let root = repository();
    fs::write(root.path().join("new.txt"), "new").unwrap();
    let text = "git switch -c feat/test && git add -- new.txt && git commit -m 'Save welcome' && gh pr create --title='Friendly title' --body 'One line\nAnother line' --head=feat/test --base main --draft";
    let details = inspect(root.path(), &[shell(text)]).unwrap();
    assert_eq!(details.len(), 4);
    assert!(details.iter().all(|detail| detail.command_index == 0));
    assert_eq!(details[0].kind, "branch.create");
    assert_eq!(details[2].source_branch.as_deref(), Some("feat/test"));
    assert_eq!(details[2].files.as_ref().unwrap(), &vec!["new.txt"]);
    assert!(details[2].evidence.contains("prospective"));
    assert_eq!(
        details[3].pull_request_title.as_deref(),
        Some("Friendly title")
    );
    assert_eq!(details[3].body.as_deref(), Some("One line\nAnother line"));
    assert_eq!(details[3].head_branch.as_deref(), Some("feat/test"));
    assert_eq!(details[3].base_branch.as_deref(), Some("main"));
    assert!(details[3]
        .warnings
        .iter()
        .any(|warning| warning.contains("draft")));
    assert_eq!(
        read_git(root.path(), &["symbolic-ref", "--short", "HEAD"])
            .unwrap()
            .trim(),
        "main"
    );
    assert_eq!(
        read_git(root.path(), &["diff", "--cached", "--name-only"]).unwrap(),
        ""
    );
}
#[test]
fn reads_bounded_local_message_files_but_not_symlinks_or_outside_paths() {
    let root = repository();
    fs::write(root.path().join("message.txt"), "File message\n").unwrap();
    let details = inspect(
        root.path(),
        &[shell(
            "git commit -F message.txt; gh pr create -t Title -F message.txt",
        )],
    )
    .unwrap();
    assert_eq!(details[0].message.as_deref(), Some("File message\n"));
    assert_eq!(details[1].body.as_deref(), Some("File message\n"));
    std::os::unix::fs::symlink(
        root.path().join("message.txt"),
        root.path().join("link.txt"),
    )
    .unwrap();
    assert!(read_text(Some(root.path()), "link.txt").is_err());
    assert!(read_text(Some(root.path()), "/etc/passwd").is_err());
    fs::write(root.path().join("large.txt"), vec![b'x'; 64_001]).unwrap();
    assert!(read_text(Some(root.path()), "large.txt").is_err());
}
#[test]
fn dynamic_syntax_and_missing_values_never_claim_exact_details() {
    let root = repository();
    let details = inspect(
        root.path(),
        &[shell("git commit -m \"$MESSAGE\" && gh pr create --fill")],
    )
    .unwrap();
    assert_eq!(details[0].kind, "details_unavailable");
    assert!(details[0].message.is_none());
    for text in [
        "git commit -m",
        "git commit -F",
        "gh pr create --title",
        "gh pr create --body-file",
    ] {
        assert!(inspect(root.path(), &[shell(text)]).is_ok());
    }
    let details = inspect(
        root.path(),
        &[shell("git add . && git commit -am 'All tracked' --amend")],
    )
    .unwrap();
    assert!(details[1]
        .warnings
        .iter()
        .any(|warning| warning.contains("incomplete")));
    assert!(details[1]
        .warnings
        .iter()
        .any(|warning| warning.contains("amends")));
    assert_eq!(details[1].message.as_deref(), Some("All tracked"));
}

#[test]
fn projects_add_all_and_update_with_new_files_and_deletions() {
    let root = repository();
    fs::write(root.path().join("deleted.txt"), "tracked in index").unwrap();
    assert!(Command::new("git")
        .args(["add", "--", "deleted.txt"])
        .current_dir(root.path())
        .status()
        .unwrap()
        .success());
    fs::remove_file(root.path().join("deleted.txt")).unwrap();
    fs::write(root.path().join("new.txt"), "new").unwrap();
    fs::write(root.path().join(".gitignore"), "ignored.txt\n").unwrap();
    fs::write(root.path().join("ignored.txt"), "ignored").unwrap();
    for command in [
        "git add . && git commit -m All",
        "git add -A && git commit -m All",
        "git add --all && git commit -m All",
    ] {
        let details = inspect(root.path(), &[shell(command)]).unwrap();
        let files = details[0].files.as_ref().unwrap();
        assert!(files.contains(&"deleted.txt".into()));
        assert!(files.contains(&"new.txt".into()));
        assert!(!files.contains(&"ignored.txt".into()));
        assert!(details[1].files.is_none());
        assert!(details[1].file_previews.is_empty());
        assert!(details[0].evidence.contains("prospective"));
    }
    let details = inspect(root.path(), &[shell("git add -u && git commit -m Tracked")]).unwrap();
    assert_eq!(details[0].files.as_ref().unwrap(), &vec!["deleted.txt"]);
    assert!(details[1].files.is_none());
    assert!(details[1].file_previews.is_empty());
    let details = inspect(
        root.path(),
        &[shell("git add --force . && git commit -m Forced")],
    )
    .unwrap();
    assert!(details[0].files.is_none());
    assert!(!details[0].warnings.is_empty());
    assert!(root.path().join("new.txt").exists());
    assert_eq!(
        read_git(root.path(), &["diff", "--cached", "--name-only"])
            .unwrap()
            .trim(),
        "deleted.txt"
    );
}

#[test]
fn changed_shell_context_does_not_read_a_message_from_the_wrong_directory() {
    let root = repository();
    fs::write(root.path().join("message.txt"), "Wrong root message").unwrap();
    fs::create_dir(root.path().join("nested")).unwrap();
    fs::write(root.path().join("nested/message.txt"), "Nested message").unwrap();
    let details = inspect(
        root.path(),
        &[shell("cd nested && git commit -F message.txt")],
    )
    .unwrap();
    assert!(details
        .iter()
        .all(|detail| detail.kind == "details_unavailable" && detail.message.is_none()));
    let details = inspect(
        root.path(),
        &[shell(
            "git push && gh pr create --title Title --body Body --base main --head feat/test",
        )],
    )
    .unwrap();
    assert_eq!(details[1].kind, "pr.create");
    assert_eq!(details[1].pull_request_title.as_deref(), Some("Title"));
    let details = inspect(
        root.path(),
        &[shell("cd nested"), shell("git commit -F message.txt")],
    )
    .unwrap();
    assert_eq!(details[1].message.as_deref(), Some("Wrong root message"));
}

#[test]
fn previews_explicit_push_target_without_network_and_keeps_pr_fields() {
    let root = repository();
    assert!(Command::new("git")
        .args(["remote", "add", "origin", "/unreachable/example.git"])
        .current_dir(root.path())
        .status()
        .unwrap()
        .success());
    for target in ["HEAD:refs/heads/feat/test", "HEAD:feat/test", "feat/test"] {
        let details = inspect(root.path(), &[shell(&format!("git push -u origin {target} && gh pr create --title Title --body Body --base main --head feat/test"))]).unwrap();
        assert_eq!(details[0].kind, "remote.push");
        assert_eq!(details[0].remote.as_deref(), Some("origin"));
        assert_eq!(details[0].target_branch.as_deref(), Some("feat/test"));
        assert_eq!(details[1].pull_request_title.as_deref(), Some("Title"));
        assert!(details[0].evidence.contains("not been checked"));
    }
    assert_eq!(
        read_git(root.path(), &["symbolic-ref", "--short", "HEAD"])
            .unwrap()
            .trim(),
        "main"
    );
}

#[test]
fn implicit_push_does_not_invent_a_remote_or_destination() {
    let root = repository();
    assert!(Command::new("git")
        .args(["remote", "add", "origin", "/unreachable/example.git"])
        .current_dir(root.path())
        .status()
        .unwrap()
        .success());
    let details = inspect(root.path(), &[shell("git push; git push origin")]).unwrap();
    assert!(details
        .iter()
        .all(|detail| detail.kind == "remote.push" && detail.target_branch.is_none()));
    assert!(details[0].remote.is_none());
    assert_eq!(details[1].remote.as_deref(), Some("origin"));
    assert!(details.iter().all(|detail| !detail.warnings.is_empty()));
}

#[path = "push_tests.rs"]
mod push_tests;
