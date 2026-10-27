use super::*;

#[test]
fn cycle_01_signal_termination_is_failure_without_timeout() {
    let root = repository();
    let result = execute_reviewed(root.path(), &shell("printf before-signal; kill -TERM $$"));
    assert_eq!(result["stdout"], "before-signal");
    assert!(result["exitCode"].is_null());
    assert_eq!(result["processSucceeded"], false);
    assert_eq!(result["timedOut"], false);
}

#[test]
fn cycle_02_closed_stdin_does_not_turn_success_into_a_transport_failure() {
    let root = repository();
    let mut step = shell("exec 0<&-; printf input-not-needed");
    step.stdin = Some("x".repeat(INPUT_LIMIT));
    let result = execute_reviewed(root.path(), &step);
    assert_eq!(result["processSucceeded"], true);
    assert_eq!(result["stdout"], "input-not-needed");
    assert_eq!(result["timedOut"], false);
}

#[test]
fn cycle_03_shell_syntax_error_does_not_execute_the_earlier_command() {
    let root = repository();
    let result = execute_reviewed(root.path(), &shell("printf ran >should-not-exist; if then"));
    assert_eq!(result["processSucceeded"], false);
    assert_ne!(result["exitCode"], 0);
    assert!(!root.path().join("should-not-exist").exists());
    assert!(!result["stderr"].as_str().unwrap().is_empty());
}

#[test]
fn cycle_04_binary_output_metadata_describes_raw_bytes_even_with_lossy_text() {
    let root = repository();
    let result = execute_reviewed(root.path(), &shell("printf '\\377\\000x'"));
    assert_eq!(result["processSucceeded"], true);
    assert_eq!(result["stdoutCapturedBytes"], 3);
    assert_eq!(
        result["stdoutCapturedDigest"],
        format!("{:x}", Sha256::digest([255, 0, b'x']))
    );
    assert_eq!(result["stdout"], "\u{fffd}\0x");
    assert_eq!(result["stdoutTruncated"], false);
}

#[test]
fn cycle_05_noninteractive_execution_flags_are_set_for_real_children() {
    let root = repository();
    let result = execute_reviewed(root.path(), &shell("test \"$GIT_TERMINAL_PROMPT\" = 0 && test \"$GH_PROMPT_DISABLED\" = 1 && test \"$GIT_PAGER\" = cat && test \"$GH_PAGER\" = cat && test \"$PAGER\" = cat && test \"$GIT_EDITOR\" = false && test \"$GIT_SEQUENCE_EDITOR\" = false && printf noninteractive"));
    assert_eq!(result["processSucceeded"], true);
    assert_eq!(result["stdout"], "noninteractive");
}

#[test]
fn cycle_06_shell_cd_is_preserved_but_does_not_leak_to_the_next_execution() {
    let root = repository();
    let first = execute_reviewed(
        root.path(),
        &shell(
            "mkdir nested && cd nested && printf child >where && git -C .. status --short >status",
        ),
    );
    assert_eq!(first["processSucceeded"], true);
    assert_eq!(
        fs::read_to_string(root.path().join("nested/where")).unwrap(),
        "child"
    );
    assert!(!root.path().join("where").exists());
    let second = execute_reviewed(root.path(), &shell("printf parent >where"));
    assert_eq!(second["processSucceeded"], true);
    assert_eq!(
        fs::read_to_string(root.path().join("where")).unwrap(),
        "parent"
    );
    assert_eq!(
        fs::read_to_string(root.path().join("nested/where")).unwrap(),
        "child"
    );
}

#[test]
fn cycle_07_missing_program_binding_stops_before_intent_and_effects() {
    let root = repository();
    let step = shell("printf forbidden >marker");
    let mut manifest = preview(root.path(), std::slice::from_ref(&step)).unwrap();
    manifest.executables[0].program = "gh".into();
    let mut logged = false;
    let result = execute(root.path(), &step, &manifest, &mut |_, _, _, _, _| {
        logged = true;
        Ok(())
    });
    assert!(result
        .unwrap_err()
        .contains("Missing reviewed executable binding"));
    assert!(!logged);
    assert!(!root.path().join("marker").exists());
}

#[test]
fn cycle_08_git_shell_alias_arguments_execute_without_rewriting() {
    let root = repository();
    let result = execute_reviewed(root.path(), &shell("git -c 'alias.blind=!f() { printf \"%s\" \"$1\" >alias-output; }; f' blind 'quoted ; $ literal' && git config --local branchline.blind 'value with spaces'"));
    assert_eq!(result["processSucceeded"], true);
    assert_eq!(
        fs::read_to_string(root.path().join("alias-output")).unwrap(),
        "quoted ; $ literal"
    );
    let value = Command::new("git")
        .args(["config", "--local", "--get", "branchline.blind"])
        .current_dir(root.path())
        .output()
        .unwrap();
    assert!(value.status.success());
    assert_eq!(
        String::from_utf8(value.stdout).unwrap(),
        "value with spaces\n"
    );
}
