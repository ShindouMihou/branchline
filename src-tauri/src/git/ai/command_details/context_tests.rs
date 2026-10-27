use super::*;
use std::process::Command;

fn repository() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    assert!(Command::new("git")
        .args(["init", "--quiet", "--initial-branch=main"])
        .current_dir(dir.path())
        .status()
        .unwrap()
        .success());
    dir
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
fn publication_chain_keeps_literal_details_after_an_absolute_directory_change() {
    let root = repository();
    let text="cd '/requested/new project' && git init --initial-branch=main && git add --all && git -c user.name=Demo -c user.email=demo@example.invalid commit -m 'Publish the app' && gh repo create owner/project --public --source . --remote origin --description 'A developer tool' && git push --set-upstream origin main && gh repo edit owner/project --add-topic git-client --add-topic rust";
    let step = shell(text);
    let details = inspect(root.path(), &[step.clone()]).unwrap();
    assert_eq!(
        details
            .iter()
            .map(|detail| detail.kind.as_str())
            .collect::<Vec<_>>(),
        [
            "context.directory",
            "repository.init",
            "index.stage",
            "commit.create",
            "repository.create",
            "remote.push",
            "repository.edit"
        ]
    );
    assert_eq!(
        details[0].working_directory.as_deref(),
        Some("/requested/new project")
    );
    assert_eq!(details[1].target_branch.as_deref(), Some("main"));
    assert_eq!(details[3].message.as_deref(), Some("Publish the app"));
    assert!(details[3].files.is_none());
    assert!(details[3].file_previews.is_empty());
    assert_eq!(details[4].repository.as_deref(), Some("owner/project"));
    assert_eq!(details[4].visibility.as_deref(), Some("public"));
    assert_eq!(details[4].source_directory.as_deref(), Some("."));
    assert_eq!(details[5].remote.as_deref(), Some("origin"));
    assert_eq!(details[5].target_branch.as_deref(), Some("main"));
    assert_eq!(details[6].topics.as_ref().unwrap(), &["git-client", "rust"]);
    assert_eq!(step.argv[1], text);
    assert!(!root.path().join(".git/index").exists());
}
#[test]
fn changed_context_never_reads_message_files_or_invents_file_membership() {
    let root = repository();
    fs::write(
        root.path().join("message.txt"),
        "private-original-context-content",
    )
    .unwrap();
    let details=inspect(root.path(),&[shell("cd /unverified/target && git add . && git commit -F message.txt && gh pr create --body-file message.txt --title Title --head feat/test --base main")]).unwrap();
    assert!(details[2].message.is_none());
    assert!(details[3].body.is_none());
    assert_eq!(details[3].pull_request_title.as_deref(), Some("Title"));
    assert!(details.iter().all(|detail| detail.file_previews.is_empty()));
    assert!(!serde_json::to_string(&details)
        .unwrap()
        .contains("private-original-context-content"));
}
#[test]
fn identity_only_configuration_preserves_safe_index_previews_but_context_overrides_do_not() {
    let root = repository();
    fs::write(root.path().join("file.txt"), "change\n").unwrap();
    assert!(Command::new("git")
        .args(["add", "--", "file.txt"])
        .current_dir(root.path())
        .status()
        .unwrap()
        .success());
    let details = inspect(
        root.path(),
        &[shell(
            "git -c user.name=Demo -cuser.email=demo@example.invalid commit -m Save",
        )],
    )
    .unwrap();
    assert_eq!(details[0].files.as_ref().unwrap(), &["file.txt"]);
    let other = inspect(
        root.path(),
        &[shell("git -c core.worktree=/other commit -m Save")],
    )
    .unwrap();
    assert_eq!(other[0].message.as_deref(), Some("Save"));
    assert!(other[0].files.is_none());
    let directory = inspect(root.path(), &[shell("git -C /other commit -m Save")]).unwrap();
    assert_eq!(directory[0].working_directory.as_deref(), Some("/other"));
    assert!(directory[0].source_branch.is_none());
}
#[test]
fn directory_preview_stays_unknown_for_relative_or_dynamic_context_and_respects_terminators() {
    let root = repository();
    for text in [
        "cd elsewhere && git commit -m Save",
        "cd \"$PROJECT\" && git commit -m Save",
    ] {
        assert!(inspect(root.path(), &[shell(text)])
            .unwrap()
            .iter()
            .all(|detail| detail.kind == "details_unavailable"));
    }
    let details=inspect(root.path(),&[shell("gh repo create owner/project -- --public --source /private && gh repo edit owner/project -- --add-topic hidden")]).unwrap();
    assert!(details[0].visibility.is_none());
    assert!(details[0].source_directory.is_none());
    assert!(details[1].topics.is_none());
    let empty = inspect(
        root.path(),
        &[shell("gh repo create owner/project '' /private")],
    )
    .unwrap();
    assert!(empty[0].source_directory.is_none());
}

#[test]
fn an_overridden_git_context_cannot_supply_later_commit_file_membership() {
    let root = repository();
    fs::write(root.path().join("staged.txt"), "local\n").unwrap();
    assert!(Command::new("git")
        .args(["add", "--", "staged.txt"])
        .current_dir(root.path())
        .status()
        .unwrap()
        .success());
    let details = inspect(
        root.path(),
        &[shell(
            "git -C /other commit -m Other && git commit -m Local",
        )],
    )
    .unwrap();
    assert!(details[0].files.is_none());
    assert!(details[1].files.is_none());
    assert!(details.iter().all(|detail| detail.file_previews.is_empty()));
    assert_eq!(
        read_git(root.path(), &["diff", "--cached", "--name-only"]).as_deref(),
        Some("staged.txt\n")
    );
}

#[test]
fn release_upload_details_preserve_assets_and_notes_path_without_reading_it() {
    let root = repository();
    let details=inspect(root.path(),&[shell("gh release create v0.4.0 '/tmp/Branchline app.zip' /tmp/archive.sha256 --repo owner/branchline --target main --title 'Branchline 0.4.0' --notes-file /private/notes.md --latest")]).unwrap();
    let detail = &details[0];
    assert_eq!(detail.kind, "release.create");
    assert_eq!(detail.tag.as_deref(), Some("v0.4.0"));
    assert_eq!(detail.repository.as_deref(), Some("owner/branchline"));
    assert_eq!(detail.target_branch.as_deref(), Some("main"));
    assert_eq!(detail.release_title.as_deref(), Some("Branchline 0.4.0"));
    assert_eq!(detail.notes_file.as_deref(), Some("/private/notes.md"));
    assert_eq!(
        detail.assets.as_ref().unwrap(),
        &["/tmp/Branchline app.zip", "/tmp/archive.sha256"]
    );
    assert!(detail.body.is_none());
}
