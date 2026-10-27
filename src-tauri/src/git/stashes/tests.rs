use super::*;

fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    run(root, &["init", "-b", "main"], None).unwrap();
    run(root, &["config", "user.name", "Stash tests"], None).unwrap();
    run(root, &["config", "user.email", "stash@example.test"], None).unwrap();
    run(root, &["config", "commit.gpgsign", "false"], None).unwrap();
    std::fs::write(root.join("a.txt"), "original\n").unwrap();
    std::fs::write(root.join("b.txt"), "original b\n").unwrap();
    run(root, &["add", "."], None).unwrap();
    run(root, &["commit", "-m", "Fixture"], None).unwrap();
    dir
}
fn save(root: &Path, scope: &str, paths: &[String], untracked: bool) -> Value {
    let preview = save::preview(root, scope, paths, untracked).unwrap();
    save::save(
        root,
        scope,
        paths,
        untracked,
        "Saved work",
        preview["token"].as_str().unwrap(),
    )
    .unwrap()
}
fn restore(root: &Path, pop: bool, index: bool) -> Value {
    let entry = entries(root).unwrap().remove(0);
    handle(
        root,
        Request::Restore {
            expected_root: root.to_string_lossy().into(),
            reference: entry.reference,
            oid: entry.oid,
            pop,
            restore_index: index,
            expected: snapshot::token(root).unwrap(),
        },
    )
    .unwrap()
}

#[test]
fn unstaged_preserves_overlapping_staged_content_and_restores_only_unstaged() {
    let dir = fixture();
    let root = dir.path();
    std::fs::write(root.join("a.txt"), "staged\n").unwrap();
    run(root, &["add", "a.txt"], None).unwrap();
    std::fs::write(root.join("a.txt"), "unstaged\n").unwrap();
    let index = run(root, &["diff", "--cached", "--binary"], None).unwrap();
    let result = save(root, "unstaged", &[], false);
    assert_eq!(result["ok"], true);
    assert_eq!(
        std::fs::read_to_string(root.join("a.txt")).unwrap(),
        "staged\n"
    );
    assert_eq!(
        run(root, &["diff", "--cached", "--binary"], None).unwrap(),
        index
    );
    let oid = result["stashOid"].as_str().unwrap();
    let patch = review::diff(root, oid, "a.txt", false).unwrap();
    assert!(patch["patch"].as_str().unwrap().contains("-staged"));
    assert!(review::files(root, oid, true).unwrap()["files"]
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(restore(root, false, false)["ok"], true);
    assert_eq!(
        std::fs::read_to_string(root.join("a.txt")).unwrap(),
        "unstaged\n"
    );
    assert_eq!(
        run(root, &["diff", "--cached", "--binary"], None).unwrap(),
        index
    );
    assert_eq!(entries(root).unwrap().len(), 1);
}

#[test]
fn saves_new_files_and_pop_keeps_a_recovery_copy() {
    let dir = fixture();
    let root = dir.path();
    std::fs::write(root.join("new.txt"), "new\n").unwrap();
    let result = save(root, "unstaged", &[], true);
    assert_eq!(result["ok"], true);
    assert!(!root.join("new.txt").exists());
    assert_eq!(
        review::files(root, result["stashOid"].as_str().unwrap(), false).unwrap()["files"][0]
            ["source"],
        "untracked"
    );
    let restored = restore(root, true, false);
    assert_eq!(restored["outcome"], "popped");
    assert_eq!(
        std::fs::read_to_string(root.join("new.txt")).unwrap(),
        "new\n"
    );
    assert!(entries(root).unwrap().is_empty());
    assert_eq!(
        text(
            run(
                root,
                &["rev-parse", restored["recoveryRef"].as_str().unwrap()],
                None
            )
            .unwrap()
        )
        .unwrap()
        .trim(),
        result["stashOid"].as_str().unwrap()
    );
}

#[test]
fn selected_files_leave_other_changes_and_restore_index() {
    let dir = fixture();
    let root = dir.path();
    std::fs::write(root.join("a.txt"), "staged a\n").unwrap();
    run(root, &["add", "a.txt"], None).unwrap();
    std::fs::write(root.join("b.txt"), "working b\n").unwrap();
    assert_eq!(save(root, "selected", &["a.txt".into()], false)["ok"], true);
    assert_eq!(
        std::fs::read_to_string(root.join("b.txt")).unwrap(),
        "working b\n"
    );
    assert_eq!(restore(root, false, true)["ok"], true);
    assert!(text(run(root, &["diff", "--cached"], None).unwrap())
        .unwrap()
        .contains("+staged a"));
}

#[test]
fn conflicting_pop_retains_stash() {
    let dir = fixture();
    let root = dir.path();
    std::fs::write(root.join("a.txt"), "saved\n").unwrap();
    save(root, "all", &[], false);
    std::fs::write(root.join("a.txt"), "different\n").unwrap();
    run(root, &["add", "."], None).unwrap();
    run(root, &["commit", "-m", "Conflicting version"], None).unwrap();
    assert_eq!(restore(root, true, false)["ok"], false);
    assert_eq!(entries(root).unwrap().len(), 1);
}

#[test]
fn rejects_stale_preview_and_stale_selector() {
    let dir = fixture();
    let root = dir.path();
    std::fs::write(root.join("a.txt"), "one\n").unwrap();
    let preview = save::preview(root, "all", &[], false).unwrap();
    std::fs::write(root.join("a.txt"), "two\n").unwrap();
    assert!(save::save(
        root,
        "all",
        &[],
        false,
        "name",
        preview["token"].as_str().unwrap()
    )
    .is_err());
    save(root, "all", &[], false);
    let entry = entries(root).unwrap().remove(0);
    std::fs::write(root.join("b.txt"), "another\n").unwrap();
    save(root, "all", &[], false);
    assert!(selected(root, &entry.reference, &entry.oid).is_err());
    assert!(review::diff(root, &entry.oid, "../outside", false).is_err());
}

#[test]
fn unstaged_binary_deleted_and_added_files_keep_the_index() {
    let dir = fixture();
    let root = dir.path();
    std::fs::write(root.join("added.bin"), b"a\0b").unwrap();
    run(root, &["add", "added.bin"], None).unwrap();
    std::fs::write(root.join("added.bin"), b"c\0d").unwrap();
    std::fs::remove_file(root.join("b.txt")).unwrap();
    let index = run(root, &["diff", "--cached", "--binary"], None).unwrap();
    assert_eq!(save(root, "unstaged", &[], false)["ok"], true);
    assert_eq!(std::fs::read(root.join("added.bin")).unwrap(), b"a\0b");
    assert!(root.join("b.txt").exists());
    assert_eq!(restore(root, true, false)["ok"], true);
    assert_eq!(std::fs::read(root.join("added.bin")).unwrap(), b"c\0d");
    assert!(!root.join("b.txt").exists());
    assert_eq!(
        run(root, &["diff", "--cached", "--binary"], None).unwrap(),
        index
    );
}

#[test]
fn rename_selection_includes_both_paths_and_does_not_save_other_files() {
    let dir = fixture();
    let root = dir.path();
    run(root, &["mv", "a.txt", "renamed.txt"], None).unwrap();
    std::fs::write(root.join("b.txt"), "leave this\n").unwrap();
    let result = save(root, "selected", &["renamed.txt".into()], false);
    assert_eq!(result["ok"], true);
    assert!(root.join("a.txt").exists());
    assert!(!root.join("renamed.txt").exists());
    assert_eq!(
        std::fs::read_to_string(root.join("b.txt")).unwrap(),
        "leave this\n"
    );
    assert_eq!(restore(root, true, true)["ok"], true);
    assert!(!root.join("a.txt").exists());
    assert!(root.join("renamed.txt").exists());
}

#[test]
fn ignored_files_stay_and_untracked_inclusion_is_explicit() {
    let dir = fixture();
    let root = dir.path();
    std::fs::write(root.join(".gitignore"), "ignored.txt\n").unwrap();
    run(root, &["add", ".gitignore"], None).unwrap();
    run(root, &["commit", "-m", "Ignore fixture"], None).unwrap();
    std::fs::write(root.join("ignored.txt"), "private\n").unwrap();
    std::fs::write(root.join("new.txt"), "new\n").unwrap();
    std::fs::write(root.join("a.txt"), "tracked\n").unwrap();
    let saved = save(root, "all", &[], false);
    assert!(root.join("new.txt").exists());
    assert!(root.join("ignored.txt").exists());
    assert_eq!(
        review::files(root, saved["stashOid"].as_str().unwrap(), false).unwrap()["files"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(save(root, "unstaged", &[], true)["ok"], true);
    assert!(!root.join("new.txt").exists());
    assert!(root.join("ignored.txt").exists());
}

#[test]
fn delete_has_recovery_and_wrong_window_cannot_mutate() {
    let dir = fixture();
    let root = dir.path();
    std::fs::write(root.join("a.txt"), "saved\n").unwrap();
    save(root, "all", &[], false);
    let entry = entries(root).unwrap().remove(0);
    assert!(handle(
        root,
        Request::Drop {
            expected_root: "/another/repository".into(),
            reference: entry.reference.clone(),
            oid: entry.oid.clone()
        }
    )
    .is_err());
    assert_eq!(entries(root).unwrap().len(), 1);
    let result = handle(
        root,
        Request::Drop {
            expected_root: root.to_string_lossy().into(),
            reference: entry.reference,
            oid: entry.oid.clone(),
        },
    )
    .unwrap();
    assert!(entries(root).unwrap().is_empty());
    assert_eq!(
        text(
            run(
                root,
                &["rev-parse", result["recoveryRef"].as_str().unwrap()],
                None
            )
            .unwrap()
        )
        .unwrap()
        .trim(),
        entry.oid
    );
}

#[test]
fn staged_only_is_not_an_unstaged_stash() {
    let dir = fixture();
    let root = dir.path();
    std::fs::write(root.join("a.txt"), "staged\n").unwrap();
    run(root, &["add", "a.txt"], None).unwrap();
    let preview = save::preview(root, "unstaged", &[], false).unwrap();
    assert!(preview["files"].as_array().unwrap().is_empty());
    assert!(save::save(
        root,
        "unstaged",
        &[],
        false,
        "No-op",
        preview["token"].as_str().unwrap()
    )
    .is_err());
    assert!(entries(root).unwrap().is_empty());
}

#[test]
fn selected_snapshot_excludes_unrelated_staged_and_untracked_files() {
    let dir = fixture();
    let root = dir.path();
    std::fs::write(root.join("a.txt"), "staged a\n").unwrap();
    std::fs::write(root.join("b.txt"), "staged b\n").unwrap();
    run(root, &["add", "a.txt", "b.txt"], None).unwrap();
    std::fs::write(root.join("a.txt"), "working a\n").unwrap();
    std::fs::write(root.join("selected-new.txt"), "new selected\n").unwrap();
    std::fs::write(root.join("other-new.txt"), "new other\n").unwrap();
    let saved = save(
        root,
        "selected",
        &["a.txt".into(), "selected-new.txt".into()],
        true,
    );
    assert_eq!(saved["ok"], true);
    assert_eq!(
        std::fs::read_to_string(root.join("a.txt")).unwrap(),
        "original\n"
    );
    assert_eq!(
        text(run(root, &["show", ":b.txt"], None).unwrap()).unwrap(),
        "staged b\n"
    );
    assert!(root.join("other-new.txt").exists());
    assert!(!root.join("selected-new.txt").exists());
    let oid = saved["stashOid"].as_str().unwrap();
    assert_eq!(
        review::files(root, oid, false).unwrap()["files"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let staged = review::files(root, oid, true).unwrap();
    assert_eq!(staged["files"].as_array().unwrap().len(), 1);
    assert_eq!(staged["files"][0]["path"], "a.txt");
    assert_eq!(restore(root, true, false)["ok"], true);
    assert_eq!(
        std::fs::read_to_string(root.join("a.txt")).unwrap(),
        "working a\n"
    );
    assert_eq!(
        text(run(root, &["show", ":b.txt"], None).unwrap()).unwrap(),
        "staged b\n"
    );
    assert!(root.join("selected-new.txt").exists());
}
