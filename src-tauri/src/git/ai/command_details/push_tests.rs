use super::{inspect, read_git, repository, shell, Command};
use crate::git::ai::command_details::types::PushTargetKind;

fn remote_repository() -> tempfile::TempDir {
    let root = repository();
    assert!(Command::new("git")
        .args(["remote", "add", "origin", "/unreachable/example.git"])
        .current_dir(root.path())
        .status()
        .unwrap()
        .success());
    root
}

#[test]
fn atomic_push_preserves_explicit_remote_and_every_destination() {
    let root = remote_repository();
    let details = inspect(
        root.path(),
        &[shell("git push --atomic origin HEAD:main refs/tags/v0.4.1")],
    )
    .unwrap();
    let push = &details[0];
    assert_eq!(push.remote.as_deref(), Some("origin"));
    assert_eq!(push.atomic, Some(true));
    assert_eq!(push.push_targets.len(), 2);
    assert_eq!(push.push_targets[0].source_ref, "HEAD");
    assert_eq!(push.push_targets[0].source_branch.as_deref(), Some("main"));
    assert_eq!(push.push_targets[0].target_ref, "main");
    assert_eq!(push.push_targets[0].kind, PushTargetKind::Branch);
    assert_eq!(push.push_targets[1].source_ref, "refs/tags/v0.4.1");
    assert_eq!(push.push_targets[1].target_ref, "refs/tags/v0.4.1");
    assert_eq!(push.push_targets[1].kind, PushTargetKind::Tag);
    assert!(push.target_branch.is_none());
    assert!(push.evidence.contains("not been checked"));
}

#[test]
fn release_sequence_preserves_literal_head_when_earlier_commands_invalidate_branch_context() {
    let root = remote_repository();
    let details = inspect(root.path(), &[shell("git diff --check && git add -- src/main.ts && git -c user.name=Test -c user.email=test@example.invalid commit -m Release && git -c user.name=Test -c user.email=test@example.invalid tag -a v0.4.1 -m Release && git push --atomic origin HEAD:main refs/tags/v0.4.1")]).unwrap();
    let push = details
        .iter()
        .find(|detail| detail.kind == "remote.push")
        .unwrap();
    assert_eq!(push.remote.as_deref(), Some("origin"));
    assert_eq!(push.push_targets.len(), 2);
    assert_eq!(push.push_targets[0].source_ref, "HEAD");
    assert!(push.push_targets[0].source_branch.is_none());
    assert_eq!(push.push_targets[0].target_ref, "main");
    assert_eq!(push.evidence_kind, "projected");
}

#[test]
fn annotated_tag_preview_keeps_tag_identity_without_pushing_or_peeling_it() {
    let root = remote_repository();
    for args in [
        vec![
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "commit",
            "--allow-empty",
            "-m",
            "Initial",
        ],
        vec![
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "tag",
            "-a",
            "v1",
            "-m",
            "Release",
        ],
    ] {
        assert!(Command::new("git")
            .args(args)
            .current_dir(root.path())
            .status()
            .unwrap()
            .success());
    }
    let before = read_git(root.path(), &["show-ref"]).unwrap();
    let details = inspect(
        root.path(),
        &[shell(
            "git push origin --atomic refs/tags/v1:refs/tags/published",
        )],
    )
    .unwrap();
    let target = &details[0].push_targets[0];
    assert_eq!(target.kind, PushTargetKind::Tag);
    assert_eq!(target.source_ref, "refs/tags/v1");
    assert_eq!(target.target_ref, "refs/tags/published");
    assert!(target.source_branch.is_none());
    assert_eq!(read_git(root.path(), &["show-ref"]).unwrap(), before);
}

#[test]
fn atomic_options_respect_order_and_do_not_invent_implicit_targets() {
    let root = remote_repository();
    for (options, expected) in [
        ("--atomic --no-atomic", false),
        ("--no-atomic --atomic", true),
    ] {
        let details = inspect(
            root.path(),
            &[shell(&format!(
                "git push {options} -u origin HEAD:refs/heads/next"
            ))],
        )
        .unwrap();
        assert_eq!(details[0].atomic, Some(expected));
        assert_eq!(details[0].target_branch.as_deref(), Some("next"));
        assert!(details[0]
            .warnings
            .iter()
            .any(|warning| warning.contains("upstream")));
    }
    let details = inspect(root.path(), &[shell("git push --atomic origin")]).unwrap();
    assert_eq!(details[0].remote.as_deref(), Some("origin"));
    assert!(details[0].push_targets.is_empty());
    assert!(details[0].target_branch.is_none());
}

#[test]
fn unsupported_refspecs_remain_explicit_without_hiding_known_destinations() {
    let root = remote_repository();
    let details = inspect(
        root.path(),
        &[shell(
            "git push --atomic origin HEAD:main '+next:next' 'refs/heads/*:refs/heads/*' :obsolete",
        )],
    )
    .unwrap();
    assert_eq!(details[0].push_targets.len(), 1);
    assert!(details[0].push_targets_incomplete);
    assert_eq!(details[0].push_targets[0].target_ref, "main");
    assert!(details[0]
        .warnings
        .iter()
        .any(|warning| warning.contains("+next:next")));
    assert!(details[0]
        .warnings
        .iter()
        .any(|warning| warning.contains(":obsolete")));
    assert!(details[0].target_branch.is_none());
}

#[test]
fn unsupported_option_keeps_only_preceding_known_remote_and_never_guesses_option_values() {
    let root = remote_repository();
    let details = inspect(root.path(), &[shell("git push origin --receive-pack /some/helper HEAD:main; git push --receive-pack origin other HEAD:main")]).unwrap();
    assert_eq!(details[0].remote.as_deref(), Some("origin"));
    assert!(details[0].push_targets.is_empty());
    assert!(details[1].remote.is_none());
    assert!(details[1].push_targets.is_empty());
}

#[test]
fn older_activity_without_push_metadata_still_deserializes() {
    let root = remote_repository();
    let details = inspect(root.path(), &[shell("git push origin HEAD:main")]).unwrap();
    let mut saved = serde_json::to_value(&details[0]).unwrap();
    for field in ["pushTargets", "pushTargetsIncomplete", "atomic"] {
        saved.as_object_mut().unwrap().remove(field);
    }
    let restored: super::super::Detail = serde_json::from_value(saved).unwrap();
    assert!(restored.push_targets.is_empty());
    assert!(!restored.push_targets_incomplete);
    assert_eq!(restored.remote.as_deref(), Some("origin"));
    assert_eq!(restored.target_branch.as_deref(), Some("main"));
}
