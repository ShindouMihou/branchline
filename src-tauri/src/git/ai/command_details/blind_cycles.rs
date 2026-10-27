use super::*;
use std::{collections::BTreeSet, process::Command};

fn git(root: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}
fn repository() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    git(root.path(), &["init", "--quiet", "--initial-branch=main"]);
    for (key, value) in [
        ("user.name", "Blind Preview"),
        ("user.email", "blind@example.invalid"),
        ("commit.gpgsign", "false"),
        ("core.hooksPath", "/dev/null"),
    ] {
        git(root.path(), &["config", key, value]);
    }
    root
}
fn command(program: &str, argv: &[&str]) -> CommandStep {
    CommandStep {
        program: program.into(),
        argv: argv.iter().map(|value| value.to_string()).collect(),
        stdin: None,
        timeout_seconds: None,
    }
}
fn shell(script: &str) -> CommandStep {
    command("shell", &["-c", script])
}
fn names(value: &str) -> BTreeSet<String> {
    value
        .split('\0')
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
}
fn staged(root: &Path) -> BTreeSet<String> {
    names(&git(
        root,
        &["diff", "--cached", "--no-renames", "--name-only", "-z"],
    ))
}
fn seed(root: &Path, files: &[(&str, &str)]) {
    for (name, value) in files {
        fs::write(root.join(name), value).unwrap();
    }
    git(root, &["add", "--all"]);
    git(root, &["commit", "--quiet", "-m", "Seed"]);
}
#[test]
fn blind_cycle_01_quoted_unusual_filenames_match_actual_staging() {
    let root = repository();
    for name in [
        "space name.txt",
        "line\nbreak.txt",
        "-leading.txt",
        "quote'file.txt",
    ] {
        fs::write(root.path().join(name), "value").unwrap();
    }
    let script = "git add -- 'space name.txt' 'line\nbreak.txt' '-leading.txt' \"quote'file.txt\"";
    let details = inspect(root.path(), &[shell(script)]).unwrap();
    let actual = Command::new("/bin/sh")
        .args(["-c", script])
        .current_dir(root.path())
        .status()
        .unwrap();
    assert!(actual.success());
    assert_eq!(
        details[0]
            .files
            .as_ref()
            .unwrap()
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>(),
        staged(root.path())
    );
}
#[test]
fn blind_cycle_02_update_excludes_untracked_and_includes_deleted() {
    let root = repository();
    seed(
        root.path(),
        &[("changed.txt", "old"), ("deleted.txt", "old")],
    );
    fs::write(root.path().join("changed.txt"), "new").unwrap();
    fs::remove_file(root.path().join("deleted.txt")).unwrap();
    fs::write(root.path().join("untracked.txt"), "new").unwrap();
    let details = inspect(root.path(), &[shell("git add --update")]).unwrap();
    git(root.path(), &["add", "--update"]);
    assert_eq!(
        details[0]
            .files
            .as_ref()
            .unwrap()
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>(),
        staged(root.path())
    );
    assert!(!details[0]
        .files
        .as_ref()
        .unwrap()
        .contains(&"untracked.txt".into()));
}
#[test]
fn blind_cycle_03_directory_selection_excludes_ignored_and_siblings() {
    let root = repository();
    fs::create_dir(root.path().join("selected")).unwrap();
    fs::write(root.path().join(".gitignore"), "*.log\n").unwrap();
    fs::write(root.path().join("selected/yes.txt"), "yes").unwrap();
    fs::write(root.path().join("selected/no.log"), "no").unwrap();
    fs::write(root.path().join("outside.txt"), "outside").unwrap();
    let details = inspect(root.path(), &[shell("git add -- selected")]).unwrap();
    git(root.path(), &["add", "--", "selected"]);
    assert_eq!(
        details[0]
            .files
            .as_ref()
            .unwrap()
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>(),
        staged(root.path())
    );
}
#[test]
fn blind_cycle_04_direct_argv_message_is_not_shell_expanded() {
    let root = repository();
    fs::write(root.path().join("file.txt"), "value").unwrap();
    git(root.path(), &["add", "--", "file.txt"]);
    let message = "$HOME $(touch never-created) && literal";
    let details = inspect(root.path(), &[command("git", &["commit", "-m", message])]).unwrap();
    git(root.path(), &["commit", "--quiet", "-m", message]);
    assert_eq!(
        details[0].message.as_deref(),
        Some(git(root.path(), &["log", "-1", "--format=%B"]).trim())
    );
    assert!(!root.path().join("never-created").exists());
}
#[test]
fn blind_cycle_05_message_file_input_and_cleanup_are_distinguished() {
    let root = repository();
    fs::write(root.path().join("file.txt"), "value").unwrap();
    git(root.path(), &["add", "--", "file.txt"]);
    let input = "Title\n\nBody with Unicode: café 🧪\n";
    fs::write(root.path().join("message.txt"), input).unwrap();
    let details = inspect(root.path(), &[shell("git commit --file=message.txt")]).unwrap();
    git(root.path(), &["commit", "--quiet", "--file=message.txt"]);
    assert_eq!(details[0].message.as_deref(), Some(input));
    assert_eq!(
        git(root.path(), &["log", "-1", "--format=%B"]).trim(),
        input.trim()
    );
    assert!(details[0]
        .warnings
        .iter()
        .any(|warning| warning.contains("cleanup")));
}
#[test]
fn blind_cycle_06_gh_file_flags_owner_head_and_empty_body_are_literal() {
    let root = repository();
    fs::write(root.path().join("body.md"), "# Plan\n\n- item\n").unwrap();
    let details=inspect(root.path(), &[shell("gh pr create -tTitle -Fbody.md -Howner:feature -Brelease; gh pr create --title='Next' --body='' --head='owner:next' --base=main")]).unwrap();
    assert_eq!(details[0].pull_request_title.as_deref(), Some("Title"));
    assert_eq!(details[0].body.as_deref(), Some("# Plan\n\n- item\n"));
    assert_eq!(details[0].head_branch.as_deref(), Some("owner:feature"));
    assert_eq!(details[0].base_branch.as_deref(), Some("release"));
    assert_eq!(details[1].body.as_deref(), Some(""));
    assert!(details.iter().all(|detail| detail
        .warnings
        .iter()
        .any(|warning| warning.contains("not verified"))));
}
#[test]
fn blind_cycle_07_unsupported_staging_modes_do_not_invent_files() {
    let root = repository();
    fs::write(root.path().join("file.txt"), "value").unwrap();
    for script in [
        "git add --intent-to-add file.txt",
        "git add --patch file.txt",
        "git add --force file.txt",
        "git add --pathspec-from-file=list.txt",
        "git add --chmod=+x file.txt",
    ] {
        let details = inspect(root.path(), &[shell(script)]).unwrap();
        assert!(details[0].files.is_none(), "{script}");
        assert!(details[0].file_previews.is_empty(), "{script}");
        assert!(!details[0].warnings.is_empty(), "{script}");
    }
}
#[test]
fn blind_cycle_08_restaging_back_to_head_does_not_claim_commit_changes() {
    let root = repository();
    seed(root.path(), &[("cancelled.txt", "original\n")]);
    fs::write(root.path().join("cancelled.txt"), "staged\n").unwrap();
    git(root.path(), &["add", "--", "cancelled.txt"]);
    fs::write(root.path().join("cancelled.txt"), "original\n").unwrap();
    let details = inspect(
        root.path(),
        &[shell("git add -- cancelled.txt && git commit -m Save")],
    )
    .unwrap();
    git(root.path(), &["add", "--", "cancelled.txt"]);
    let actual = staged(root.path());
    assert!(actual.is_empty());
    assert!(
        details[1].files.is_none() || details[1].files.as_ref().unwrap().is_empty(),
        "The projection must not claim an unchanged file will be committed: {:?}",
        details[1].files
    );
}

#[test]
fn blind_cycle_08_unrelated_partial_staging_stays_in_the_index() {
    let root = repository();
    seed(
        root.path(),
        &[("partial.txt", "base\n"), ("selected.txt", "base\n")],
    );
    fs::write(root.path().join("partial.txt"), "staged partial\n").unwrap();
    git(root.path(), &["add", "--", "partial.txt"]);
    fs::write(root.path().join("partial.txt"), "unstaged partial\n").unwrap();
    fs::write(root.path().join("selected.txt"), "new selection\n").unwrap();
    let details = inspect(
        root.path(),
        &[shell("git add -- selected.txt && git commit -m Save")],
    )
    .unwrap();
    git(root.path(), &["add", "--", "selected.txt"]);
    assert_eq!(
        details[1]
            .files
            .as_ref()
            .unwrap()
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>(),
        staged(root.path())
    );
    let partial = details[1]
        .file_previews
        .iter()
        .find(|file| file.path == "partial.txt")
        .unwrap();
    assert_eq!(partial.source, FileSource::Index);
    let selected = details[1]
        .file_previews
        .iter()
        .find(|file| file.path == "selected.txt")
        .unwrap();
    assert_eq!(selected.source, FileSource::Worktree);
    assert_eq!(
        git(root.path(), &["show", ":partial.txt"]),
        "staged partial\n"
    );
}
#[test]
fn blind_cycle_08_unborn_restaging_scope_is_unknown_without_blob_reads() {
    let root = repository();
    fs::write(root.path().join("new.txt"), "staged\n").unwrap();
    git(root.path(), &["add", "--", "new.txt"]);
    fs::write(root.path().join("new.txt"), "updated\n").unwrap();
    let details = inspect(
        root.path(),
        &[shell("git add -- new.txt && git commit -m Save")],
    )
    .unwrap();
    assert!(details[1].files.is_none());
    assert!(details[1].file_previews.is_empty());
    assert_eq!(details[1].evidence_kind, "unknown");
    git(root.path(), &["add", "--", "new.txt"]);
    assert!(staged(root.path()).contains("new.txt"));
}
