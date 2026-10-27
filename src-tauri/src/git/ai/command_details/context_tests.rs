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
fn relative_directory_context_stays_unknown_and_options_respect_terminators() {
    let root = repository();
    let relative = inspect(root.path(), &[shell("cd elsewhere && git commit -m Save")]).unwrap();
    assert!(relative.iter().any(|detail| detail.message.as_deref() == Some("Save")));
    assert!(relative.iter().all(|detail| detail.files.is_none() && detail.file_previews.is_empty() && detail.source_branch.is_none()));
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

#[test]
fn dynamic_preflight_keeps_later_literal_release_steps_without_reading_runtime_state() {
    let root = repository();
    fs::write(root.path().join("notes.md"), "private snapshot content").unwrap();
    let script = "test \"$(git rev-parse HEAD)\" = expected && test \"$(shasum -a 256 inputs | awk '{print $1}')\" = digest && shasum -a 256 -c inputs && git add -- file.txt && git commit -m 'Release app' && git push --atomic origin main refs/tags/v1.2.3 && gh release create v1.2.3 app.zip --repo owner/app --title 'App 1.2.3' --notes-file notes.md --draft";
    let step = shell(script);
    let details = inspect(root.path(), &[step.clone()]).unwrap();
    assert!(details.iter().any(|detail| detail.kind == "index.stage"));
    let commit = details.iter().find(|detail| detail.kind == "commit.create").unwrap();
    assert_eq!(commit.message.as_deref(), Some("Release app"));
    assert!(commit.files.is_none());
    assert!(commit.source_branch.is_none());
    let push = details.iter().find(|detail| detail.kind == "remote.push").unwrap();
    assert_eq!(push.remote.as_deref(), Some("origin"));
    let release = details.iter().find(|detail| detail.kind == "release.create").unwrap();
    assert_eq!(release.release_title.as_deref(), Some("App 1.2.3"));
    assert_eq!(release.assets.as_ref().unwrap(), &["app.zip"]);
    assert!(details.iter().all(|detail| detail.file_previews.is_empty()));
    assert!(!serde_json::to_string(&details).unwrap().contains("private snapshot content"));
    assert_eq!(step.argv[1], script);
    assert!(!root.path().join(".git/index").exists());
}

#[test]
fn recovered_literals_never_invent_dynamic_files_branches_or_message_file_contents() {
    let root = repository();
    fs::write(root.path().join("notes.md"), "do-not-read-this-content").unwrap();
    for script in [
        "git add *.txt && git commit -m Save && git push origin HEAD:main",
        "cd \"$PROJECT\" && git commit -m Save && git commit -F notes.md",
        "test \"$(printf '%s' 'git push origin fake; &&')\" = ready && git commit -F notes.md",
    ] {
        let details = inspect(root.path(), &[shell(script)]).unwrap();
        assert_eq!(details[0].kind, "details_unavailable");
        assert!(details.iter().any(|detail| detail.kind == "commit.create"));
        assert!(details.iter().all(|detail| detail.files.is_none() && detail.file_previews.is_empty()));
        assert!(!serde_json::to_string(&details).unwrap().contains("do-not-read-this-content"));
        assert!(!details.iter().any(|detail| detail.target_branch.as_deref() == Some("fake")));
    }
}
