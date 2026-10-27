use super::*;

fn details(script: &str) -> Vec<Detail> {
    inspect(
        Path::new("/nonexistent/branchline-preview-test"),
        &[CommandStep {
            program: "shell".into(),
            argv: vec!["-c".into(), script.into()],
            stdin: None,
            timeout_seconds: None,
        }],
    )
    .unwrap()
}

#[test]
fn guarded_stash_rebase_restore_request_retains_details_without_resolving_variables() {
    let script = r#"test "$(git symbolic-ref --short HEAD)" = 'codex/moodle-lti-integration' && test "$(git rev-parse HEAD)" = 'd1018d52cbbb3520c9188d3eacdf18ec95065b39' && test "$(git rev-parse alex-au/moodle-lti-integration)" = '32cd741b3bf6ea6c6f053e956e95be031f90e241' && git stash push --include-untracked -m 'Preserve uncommitted Moodle work before PR338 rebase' && codex_lti_stash=$(git rev-parse refs/stash) && printf '%s\n' "$codex_lti_stash" > /tmp/lti-pr338-rebase-stash.txt && git rebase --onto 32cd741b3bf6ea6c6f053e956e95be031f90e241 d1018d52cbbb3520c9188d3eacdf18ec95065b39 && git stash apply "$codex_lti_stash""#;
    let details = details(script);
    let stash = details
        .iter()
        .find(|detail| detail.kind == "stash.save")
        .unwrap();
    assert_eq!(
        stash.message.as_deref(),
        Some("Preserve uncommitted Moodle work before PR338 rebase")
    );
    assert_eq!(stash.include_untracked, Some(true));
    let rebase = details
        .iter()
        .find(|detail| detail.kind == "history.rebase")
        .unwrap();
    assert_eq!(
        rebase.base_ref.as_deref(),
        Some("32cd741b3bf6ea6c6f053e956e95be031f90e241")
    );
    assert_eq!(
        rebase.upstream_ref.as_deref(),
        Some("d1018d52cbbb3520c9188d3eacdf18ec95065b39")
    );
    let restore = details
        .iter()
        .find(|detail| detail.kind == "stash.apply")
        .unwrap();
    assert!(restore.stash_ref.is_none());
    assert_eq!(restore.evidence_kind, "unknown");
    assert!(!serde_json::to_string(&details)
        .unwrap()
        .contains("branchline_runtime"));
    assert!(details.iter().all(|detail| detail.file_previews.is_empty()));
}

#[test]
fn guarded_reset_and_unknown_preparations_do_not_hide_later_literal_targets() {
    let found = details("some-new-tool check && git reset --mixed HEAD && git commit -m 'Actual message' && git push origin HEAD:feature");
    let reset = found
        .iter()
        .find(|detail| detail.kind == "history.reset")
        .unwrap();
    assert_eq!(reset.operation.as_deref(), Some("mixed"));
    assert_eq!(reset.target_branch.as_deref(), Some("HEAD"));
    assert!(found
        .iter()
        .any(|detail| detail.message.as_deref() == Some("Actual message")));
    assert!(found.iter().any(|detail| detail.kind == "remote.push"));
    assert!(found
        .iter()
        .filter(|detail| detail.kind == "commit.create")
        .all(|detail| detail.files.is_none()));
}

#[test]
fn dynamic_arguments_do_not_appear_as_literal_messages_or_push_targets() {
    let found = details("git commit -m \"$MESSAGE\" && git push \"$REMOTE\" \"HEAD:$DESTINATION\"");
    assert!(found
        .iter()
        .any(|detail| detail.kind == "commit.create" && detail.message.is_none()));
    let push = found
        .iter()
        .find(|detail| detail.kind == "remote.push")
        .unwrap();
    assert!(push.remote.is_none());
    assert!(push.push_targets.is_empty());
    assert!(!serde_json::to_string(&found)
        .unwrap()
        .contains("branchline_runtime"));
}

#[test]
fn cleanup_traps_test_bootstraps_and_script_payloads_do_not_hide_publication() {
    let script = r#"test "$(git symbolic-ref --short HEAD)" = feature && test "$(git rev-parse HEAD)" = expected && trap 'node cleanup.js > /tmp/cleanup.log 2>&1' 0 && node bootstrap.js > /tmp/bootstrap.log 2>&1 && NODE_OPTIONS=--no-experimental-webstorage VITE_POSTHOG_KEY='' VITE_SENTRY_DSN='' node -e 'const {spawnSync}=require("node:child_process"); const env={...process.env,TEST_SECRET:process.env.OPERATOR_SERVICE_SECRET}; const r=spawnSync(process.execPath,["test.js"],{stdio:"inherit",env}); process.exit(r.status??1)' > /tmp/full-tests.log 2>&1 && python3 -c 'import json; d=json.load(open("/tmp/browser.json")); assert d["unexpected"]==0' && git diff --check && git add -- tests/helper.ts tests/journey.ts && git commit -m 'test: use precise admin and lab hydration readiness in journeys' && git push origin HEAD:feature"#;
    let found = details(script);
    assert_eq!(
        found
            .iter()
            .filter(|detail| detail.kind == "commit.create")
            .count(),
        1
    );
    assert!(found.iter().any(|detail| detail.message.as_deref()
        == Some("test: use precise admin and lab hydration readiness in journeys")));
    let push = found
        .iter()
        .find(|detail| detail.kind == "remote.push")
        .unwrap();
    assert_eq!(push.remote.as_deref(), Some("origin"));
    assert_eq!(push.push_targets[0].source_ref, "HEAD");
    assert_eq!(push.push_targets[0].target_ref, "feature");
    assert!(found.iter().all(|detail| detail.file_previews.is_empty()));
}
