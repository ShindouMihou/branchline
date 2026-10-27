use super::*;

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
        timeout_seconds: Some(5),
    }
}

fn execute_reviewed(root: &Path, step: &CommandStep) -> Value {
    let manifest = preview(root, std::slice::from_ref(step)).unwrap();
    let mut events = Vec::new();
    let result = execute(
        root,
        step,
        &manifest,
        &mut |_, argv, _, finished, outcome| {
            events.push((argv.to_vec(), finished, outcome.map(str::to_string)));
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0], (step.argv.clone(), false, None));
    assert_eq!(events[1].0, step.argv);
    assert!(events[1].1);
    assert_eq!(result["effectsVerified"], false);
    result
}

#[test]
fn blind_shell_quoted_paths_connectors_subshells_and_git_options_are_unchanged() {
    let root = repository();
    fs::write(root.path().join("space ; dollar $.txt"), "literal\n").unwrap();
    let script = "(git add -- 'space ; dollar $.txt' && git diff --cached --name-only -z) | od -An -tx1; git add -- missing-file 2>error.txt || printf 'recovered\\n'; git -c color.ui=never status --short";
    let result = execute_reviewed(root.path(), &shell(script));
    assert_eq!(result["exitCode"], 0);
    assert_eq!(result["processSucceeded"], true);
    assert!(result["stdout"].as_str().unwrap().contains("recovered\n"));
    let staged = Command::new("git")
        .args(["diff", "--cached", "--name-only", "-z"])
        .current_dir(root.path())
        .output()
        .unwrap();
    assert_eq!(staged.stdout, b"space ; dollar $.txt\0");
    assert!(!fs::read(root.path().join("error.txt")).unwrap().is_empty());
    assert!(!root.path().join("dollar").exists());
}

#[test]
fn blind_nonzero_shell_sequence_has_partial_effects_and_exact_final_status() {
    let root = repository();
    let result = execute_reviewed(
        root.path(),
        &shell("printf before >before; false && printf forbidden >forbidden; printf after >after; exit 42"),
    );
    assert_eq!(result["exitCode"], 42);
    assert_eq!(result["processSucceeded"], false);
    assert_eq!(result["timedOut"], false);
    assert_eq!(
        fs::read_to_string(root.path().join("before")).unwrap(),
        "before"
    );
    assert_eq!(
        fs::read_to_string(root.path().join("after")).unwrap(),
        "after"
    );
    assert!(!root.path().join("forbidden").exists());
}

#[test]
fn blind_pipeline_result_follows_system_shell_without_implicit_pipefail() {
    let root = repository();
    let result = execute_reviewed(root.path(), &shell("false | printf pipeline-success"));
    assert_eq!(result["stdout"], "pipeline-success");
    assert_eq!(result["exitCode"], 0);
    assert_eq!(result["processSucceeded"], true);
    assert_eq!(result["effectsVerified"], false);
}

#[test]
fn blind_large_input_round_trips_binary_bytes_without_pipe_deadlock() {
    let root = repository();
    let mut step = shell("cat >input.bin; wc -c <input.bin");
    let content = "a\0b\n".repeat(INPUT_LIMIT / 4);
    step.stdin = Some(content.clone());
    let result = execute_reviewed(root.path(), &step);
    assert_eq!(result["processSucceeded"], true);
    assert_eq!(
        result["stdout"].as_str().unwrap().trim(),
        INPUT_LIMIT.to_string()
    );
    assert_eq!(
        fs::read(root.path().join("input.bin")).unwrap(),
        content.as_bytes()
    );
}

#[test]
fn blind_simultaneous_large_stdout_and_stderr_are_bounded_and_digest_matches() {
    let root = repository();
    let result = execute_reviewed(
        root.path(),
        &shell("awk 'BEGIN { for(i=0;i<30000;i++) { print \"stdout line\"; print \"stderr line\" > \"/dev/stderr\" } }'"),
    );
    assert_eq!(result["processSucceeded"], true);
    for stream in ["stdout", "stderr"] {
        let captured = result[stream].as_str().unwrap().as_bytes();
        assert_eq!(captured.len(), OUTPUT_LIMIT);
        assert_eq!(result[format!("{stream}Truncated")], true);
        assert_eq!(result[format!("{stream}CapturedBytes")], OUTPUT_LIMIT);
        assert_eq!(
            result[format!("{stream}CapturedDigest")],
            format!("{:x}", Sha256::digest(captured))
        );
    }
}

#[test]
fn blind_timeout_kills_delayed_descendants_and_never_reports_success() {
    let root = repository();
    let mut step = shell("(sleep 2; printf escaped >late) & wait");
    step.timeout_seconds = Some(1);
    let started = Instant::now();
    let result = execute_reviewed(root.path(), &step);
    assert_eq!(result["timedOut"], true);
    assert_eq!(result["processSucceeded"], false);
    assert!(started.elapsed() < Duration::from_secs(2));
    std::thread::sleep(Duration::from_millis(1400));
    assert!(!root.path().join("late").exists());
}

#[test]
fn blind_failed_intent_logging_prevents_execution_and_failed_receipt_is_uncertain() {
    let root = repository();
    let step = shell("printf ran >marker");
    let manifest = preview(root.path(), std::slice::from_ref(&step)).unwrap();
    let before = execute(root.path(), &step, &manifest, &mut |_, _, _, _, _| {
        Err("intent storage failed".into())
    });
    assert_eq!(before.unwrap_err(), "intent storage failed");
    assert!(!root.path().join("marker").exists());
    let after = execute(
        root.path(),
        &step,
        &manifest,
        &mut |_, _, _, finished, _| {
            if finished {
                Err("receipt storage failed".into())
            } else {
                Ok(())
            }
        },
    );
    assert_eq!(after.unwrap_err(), "receipt storage failed");
    assert_eq!(
        fs::read_to_string(root.path().join("marker")).unwrap(),
        "ran"
    );
}

#[test]
fn blind_multiline_here_document_and_command_substitution_remain_exact() {
    let root = repository();
    let result = execute_reviewed(
        root.path(),
        &shell("cat >literal.txt <<'BRANCHLINE_END'\n$(printf must-not-expand) && git push\nBRANCHLINE_END\nvalue=$(printf 'kept value'); printf '%s' \"$value\" >substitution.txt\n"),
    );
    assert_eq!(result["processSucceeded"], true);
    assert_eq!(
        fs::read_to_string(root.path().join("literal.txt")).unwrap(),
        "$(printf must-not-expand) && git push\n"
    );
    assert_eq!(
        fs::read_to_string(root.path().join("substitution.txt")).unwrap(),
        "kept value"
    );
}

#[test]
fn blind_background_job_completion_is_distinct_from_shell_process_success() {
    let root = repository();
    let result = execute_reviewed(
        root.path(),
        &shell("(sleep 1; printf later >background-result) & printf shell-finished"),
    );
    assert_eq!(result["processSucceeded"], true);
    assert_eq!(result["effectsVerified"], false);
    assert_eq!(result["stdout"], "shell-finished");
    assert!(!root.path().join("background-result").exists());
    std::thread::sleep(Duration::from_millis(1300));
    assert_eq!(
        fs::read_to_string(root.path().join("background-result")).unwrap(),
        "later"
    );
}

#[path = "blind_execution_tests/cycles.rs"]
mod cycles;
