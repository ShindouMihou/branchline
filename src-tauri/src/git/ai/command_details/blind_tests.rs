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
fn shell(script: &str) -> CommandStep {
    CommandStep {
        program: "shell".into(),
        argv: vec!["-c".into(), script.into()],
        stdin: None,
        timeout_seconds: None,
    }
}
#[test]
fn blind_literal_quoting_keeps_git_and_pr_inputs_exact() {
    let root = repository();
    let details=inspect(root.path(), &[shell("git commit -m 'literal $HOME && ; > *' && gh pr create --title 'A \"quoted\" title' --body 'line one\nline two' --head 'feat/ready' --base main")]).unwrap();
    assert_eq!(
        details[0].message.as_deref(),
        Some("literal $HOME && ; > *")
    );
    assert_eq!(
        details[1].pull_request_title.as_deref(),
        Some("A \"quoted\" title")
    );
    assert_eq!(details[1].body.as_deref(), Some("line one\nline two"));
    assert_eq!(details[1].head_branch.as_deref(), Some("feat/ready"));
    assert_eq!(details[1].base_branch.as_deref(), Some("main"));
}
#[test]
fn blind_runtime_and_context_expressions_preserve_inputs_without_claiming_state() {
    let root = repository();
    for script in [
        "git commit -m \"$(printf resolved)\"",
        "git status || git push origin main",
        "git commit -F - <<'EOF'\nmessage\nEOF",
        "cd elsewhere; gh pr create -t Title -b Body",
        "MESSAGE=x git commit -m literal",
    ] {
        let details = inspect(root.path(), &[shell(script)]).unwrap();
        assert!(details.iter().any(|detail| detail.kind == "details_unavailable"), "{script}");
        assert!(details.iter().all(|detail| detail.files.is_none() && detail.file_previews.is_empty()), "{script}");
        assert!(details.iter().filter(|detail| detail.kind == "commit.create").all(|detail| detail.source_branch.is_none()), "{script}");
        if script.contains("push origin main") {
            assert!(details.iter().any(|detail| detail.kind == "remote.push" && detail.source_branch.as_deref() == Some("main")));
        }
        if script.contains("$(printf resolved)") {
            assert!(details.iter().filter(|detail| detail.kind == "commit.create").all(|detail| detail.message.is_none()));
        }
        if script.contains("-m literal") {
            assert!(details.iter().any(|detail| detail.message.as_deref() == Some("literal")));
        }
    }
}
#[test]
fn blind_git_option_terminator_is_not_reinterpreted_as_commit_message() {
    let root = repository();
    fs::write(root.path().join("-mfilename"), "content\n").unwrap();
    assert!(Command::new("git")
        .args(["add", "--", "-mfilename"])
        .current_dir(root.path())
        .status()
        .unwrap()
        .success());
    let details = inspect(
        root.path(),
        &[shell("git commit -m 'Real message' -- -mfilename")],
    )
    .unwrap();
    assert!(Command::new("git")
        .args([
            "-c",
            "user.name=Blind Test",
            "-c",
            "user.email=blind@example.invalid",
            "commit",
            "--quiet",
            "-m",
            "Real message",
            "--",
            "-mfilename"
        ])
        .current_dir(root.path())
        .status()
        .unwrap()
        .success());
    let actual = read_git(root.path(), &["log", "-1", "--format=%B"]).unwrap();
    assert_eq!(details[0].message.as_deref(), Some(actual.trim()));
    assert!(details[0].files.is_none());
    assert!(details[0].file_previews.is_empty());
}
#[test]
fn blind_previous_branch_switch_does_not_infer_dash_as_pr_head() {
    let root = repository();
    let details = inspect(
        root.path(),
        &[shell("git switch - && gh pr create -t Title -b Body")],
    )
    .unwrap();
    assert_ne!(details.last().unwrap().head_branch.as_deref(), Some("-"));
}
#[test]
fn blind_branch_creation_options_are_not_shown_as_start_points() {
    let root = repository();
    for script in [
        "git switch -c feature --no-track",
        "git checkout -b feature --track",
        "git switch -c feature --",
    ] {
        let details = inspect(root.path(), &[shell(script)]).unwrap();
        assert_eq!(details[0].kind, "details_unavailable", "{script}");
        assert!(details[0].source_branch.is_none(), "{script}");
    }
}
#[test]
fn blind_pr_option_terminator_does_not_invent_title_or_description() {
    let root = repository();
    let details = inspect(
        root.path(),
        &[shell("gh pr create -- --title Surprise --body Incorrect")],
    )
    .unwrap();
    assert!(details[0].pull_request_title.is_none());
    assert!(details[0].body.is_none());
}
#[test]
fn blind_quoted_newlines_and_owner_head_preserve_literal_values() {
    let root = repository();
    let details=inspect(root.path(), &[shell("gh pr create --title='literal # title' --body='A; B && C\nD' --head='owner:feat/one' --base='release/next' # ignored --title Other")]).unwrap();
    assert_eq!(
        details[0].pull_request_title.as_deref(),
        Some("literal # title")
    );
    assert_eq!(details[0].body.as_deref(), Some("A; B && C\nD"));
    assert_eq!(details[0].head_branch.as_deref(), Some("owner:feat/one"));
    assert_eq!(details[0].base_branch.as_deref(), Some("release/next"));
}
