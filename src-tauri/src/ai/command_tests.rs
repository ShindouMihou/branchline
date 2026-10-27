use super::*;

fn git(root: &Path, argv: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .current_dir(root)
        .args(argv)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8_lossy(&output.stdout).trim().into()
}

fn fixture() -> (tempfile::TempDir, Shared, Credential, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "-b", "main"]);
    let repo = fs::canonicalize(repo).unwrap();
    fs::write(repo.join("first.txt"), "first\n").unwrap();
    fs::write(repo.join("second.txt"), "second\n").unwrap();
    let broker = Broker::start(dir.path().join("broker")).unwrap();
    Broker::native(&broker, Native::Enable { enabled: true }).unwrap();
    let registered = Broker::native(&broker, Native::Register {
        name: "Command workflow tests".into(),
        root: repo.to_string_lossy().into_owned(),
        content: true,
        provider: None,
    }).unwrap();
    let credential = serde_json::from_slice(&fs::read(registered["credentialFile"].as_str().unwrap()).unwrap()).unwrap();
    (dir, broker, credential, repo)
}

fn input(key: &str, commands: &[&[&str]]) -> Value {
    json!({"schemaVersion":1,"idempotencyKey":key,"explanation":{"summary":"Run the selected Git commands","reason":"Test the approved workflow","impact":"Only the listed commands run after native approval","uncertainties":["Command effects are not independently verified"]},"steps":commands.iter().map(|argv|json!({"kind":"command.execute","program":"git","argv":argv})).collect::<Vec<_>>()})
}

fn propose(broker: &Shared, credential: &Credential, key: &str, commands: &[&[&str]]) -> Value {
    let plan = broker.lock().unwrap().tool(credential, "branchline_propose_plan", input(key, commands)).unwrap();
    assert_eq!(plan["status"], "ready", "{}", plan["issue"]);
    broker.lock().unwrap().tool(credential, "branchline_request_review", json!({"planId":plan["id"]})).unwrap();
    plan
}

fn approval(plan: &Value, through: usize, acknowledge: bool, risks: bool) -> Native {
    serde_json::from_value(json!({"op":"approve","plan_id":plan["id"],"digest":plan["digest"],"through":through,"acknowledge":acknowledge,"acknowledged_risks":if risks {plan["preview"]["risks"].as_array().unwrap().iter().map(|risk|risk["code"].clone()).collect::<Vec<_>>()}else{Vec::new()}})).unwrap()
}

fn wait(broker: &Shared, plan: &Value) -> Plan {
    let started = std::time::Instant::now();
    loop {
        let current = broker.lock().unwrap().journal.plans.iter().find(|p| p.id == plan["id"].as_str().unwrap()).unwrap().clone();
        if !["approved", "running"].contains(&current.status.as_str()) { return current; }
        assert!(started.elapsed().as_secs() < 10, "Command executor did not finish");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

#[test]
fn command_request_never_stages_without_native_approval() {
    let (_dir, broker, credential, repo) = fixture();
    let plan = propose(&broker, &credential, "pending", &[&["add", "--", "first.txt"]]);
    let result = super::wait::run(&broker, &credential, json!({"planId":plan["id"],"timeoutMs":0})).unwrap();
    assert_eq!(result["waitOutcome"], "timed_out");
    assert_eq!(result["grantsApproval"], false);
    assert!(git(&repo, &["diff", "--cached", "--name-only"]).is_empty());
    assert!(broker.lock().unwrap().journal.plans[0].execution[0].commands.is_empty());
}

#[test]
fn exact_command_sequence_requires_all_risks_and_full_scope() {
    let (_dir, broker, credential, repo) = fixture();
    let plan = propose(&broker, &credential, "scope", &[&["add", "--", "first.txt"], &["branch", "--list"], &["status", "--short"]]);
    assert!(Broker::native(&broker, approval(&plan, 3, false, true)).is_err());
    assert!(Broker::native(&broker, approval(&plan, 3, true, false)).is_err());
    assert!(Broker::native(&broker, approval(&plan, 1, true, true)).is_err());
    assert!(git(&repo, &["diff", "--cached", "--name-only"]).is_empty());
    Broker::native(&broker, approval(&plan, 3, true, true)).unwrap();
    let done = wait(&broker, &plan);
    assert_eq!(done.status, "succeeded", "{:?}", done.issue);
    assert_eq!(done.execution.len(), 3);
    assert!(done.execution.iter().all(|step| step.status == "succeeded"));
    assert_eq!(git(&repo, &["diff", "--cached", "--name-only"]), "first.txt");
    assert!(done.execution.iter().all(|step| step.result.as_ref().unwrap()["effectsVerified"] == false));
}

#[test]
fn failed_command_stops_remaining_commands() {
    let (_dir, broker, credential, repo) = fixture();
    let plan = propose(&broker, &credential, "failure", &[&["add", "--", "does-not-exist.txt"], &["add", "--", "second.txt"]]);
    Broker::native(&broker, approval(&plan, 2, true, true)).unwrap();
    let done = wait(&broker, &plan);
    assert_ne!(done.status, "succeeded");
    assert_ne!(done.execution[0].status, "succeeded");
    assert!(done.execution[1].commands.is_empty());
    assert!(git(&repo, &["diff", "--cached", "--name-only"]).is_empty());
}

#[test]
fn command_stdout_is_not_disclosed_by_mcp_even_with_source_access() {
    let (_dir, broker, credential, _repo) = fixture();
    let plan = propose(&broker, &credential, "privacy", &[&["status", "--short"]]);
    Broker::native(&broker, approval(&plan, 1, true, true)).unwrap();
    let done = wait(&broker, &plan);
    assert_eq!(done.status, "succeeded");
    let result = broker.lock().unwrap().tool(&credential, "branchline_get_plan", json!({"planId":plan["id"]})).unwrap();
    assert!(result["execution"][0]["result"].get("stdout").is_none());
    assert!(result["execution"][0]["result"].get("stderr").is_none());
    let result = super::wait::run(&broker, &credential, json!({"planId":plan["id"],"timeoutMs":0})).unwrap();
    assert!(result["plan"]["execution"][0]["result"].get("stdout").is_none());
    assert_eq!(result["pullRequestUrl"], Value::Null);
}

#[test]
fn changed_command_cannot_reuse_an_idempotency_key() {
    let (_dir, broker, credential, _repo) = fixture();
    propose(&broker, &credential, "same", &[&["status", "--short"]]);
    let error = broker.lock().unwrap().tool(&credential, "branchline_propose_plan", input("same", &[&["add", "--", "first.txt"]])).unwrap_err();
    assert!(error.contains("idempotency"));
}

#[test]
fn native_decline_reason_is_returned_in_the_plan() {
    let (_dir, broker, credential, repo) = fixture();
    let plan = propose(&broker, &credential, "decline", &[&["add", "--", "first.txt"]]);
    let decline: Native = serde_json::from_value(json!({"op":"decline","plan_id":plan["id"],"reason":"Please stage second.txt instead."})).unwrap();
    Broker::native(&broker, decline).unwrap();
    let result = broker.lock().unwrap().tool(&credential, "branchline_get_plan", json!({"planId":plan["id"]})).unwrap();
    assert_eq!(result["status"], "cancelled");
    assert!(result["events"].as_array().unwrap().iter().any(|event| event["detail"].as_str().unwrap_or_default().contains("Please stage second.txt instead.")));
    assert!(git(&repo, &["diff", "--cached", "--name-only"]).is_empty());
}

#[test]
fn unchanged_shell_command_waits_for_approval_and_respects_shell_short_circuiting() {
    let (_dir, broker, credential, repo) = fixture();
    let script = "git add -- first.txt && git status --short";
    let mut request = input("shell", &[]);
    request["steps"] = json!([{"kind":"command.execute","program":"shell","argv":["-c",script]}]);
    let plan = broker.lock().unwrap().tool(&credential, "branchline_propose_plan", request).unwrap();
    assert_eq!(plan["status"], "ready", "{}", plan["issue"]);
    assert_eq!(plan["requestedSteps"][0]["argv"][1], script);
    broker.lock().unwrap().tool(&credential, "branchline_request_review", json!({"planId":plan["id"]})).unwrap();
    assert!(git(&repo, &["diff", "--cached", "--name-only"]).is_empty());
    assert!(Broker::native(&broker, approval(&plan, 1, true, false)).is_err());
    Broker::native(&broker, approval(&plan, 1, true, true)).unwrap();
    assert_eq!(wait(&broker, &plan).status, "succeeded");
    assert_eq!(git(&repo, &["diff", "--cached", "--name-only"]), "first.txt");
    let result = super::wait::run(&broker, &credential, json!({"planId":plan["id"],"timeoutMs":0})).unwrap();
    assert_eq!(result["succeeded"], true);
    assert_eq!(result["effectsVerified"], false);

    let mut request = input("shell-failure", &[]);
    request["steps"] = json!([{"kind":"command.execute","program":"shell","argv":["-c","git add -- does-not-exist.txt && git add -- second.txt"]}]);
    let plan = broker.lock().unwrap().tool(&credential, "branchline_propose_plan", request).unwrap();
    assert_eq!(plan["status"], "ready");
    broker.lock().unwrap().tool(&credential, "branchline_request_review", json!({"planId":plan["id"]})).unwrap();
    Broker::native(&broker, approval(&plan, 1, true, true)).unwrap();
    assert_ne!(wait(&broker, &plan).status, "succeeded");
    assert_eq!(git(&repo, &["diff", "--cached", "--name-only"]), "first.txt");
}

#[test]
fn public_connector_has_one_exact_command_proposal_mode() {
    let (_dir, broker, credential, repo) = fixture();
    let names = mcp::tools()["tools"].as_array().unwrap().iter().map(|tool| tool["name"].as_str().unwrap().to_string()).collect::<Vec<_>>();
    assert!(names.iter().any(|name| name == "branchline_propose_command"));
    assert!(!names.iter().any(|name| name == "branchline_propose_plan"));
    assert!(mcp::forward(&credential, "branchline_propose_plan", input("legacy", &[&["status"]])).is_err());
    let command = "git add -- first.txt && git status --short";
    let mut request = input("public-command", &[]);
    request.as_object_mut().unwrap().remove("steps");
    request["command"] = json!(command);
    request["repositoryPath"] = json!(repo);
    let plan = mcp::forward(&credential, "branchline_propose_command", request).unwrap();
    assert_eq!(plan["status"], "ready", "{}", plan["issue"]);
    assert_eq!(plan["requestedSteps"][0]["argv"][1], command);
    mcp::forward(&credential, "branchline_request_review", json!({"planId":plan["id"]})).unwrap();
    assert!(git(&repo, &["diff", "--cached", "--name-only"]).is_empty());
    Broker::native(&broker, approval(&plan, 1, true, true)).unwrap();
    assert_eq!(wait(&broker, &plan).status, "succeeded");
    let result = mcp::forward(&credential, "branchline_wait_for_result", json!({"planId":plan["id"],"timeoutMs":0})).unwrap();
    assert_eq!(result["succeeded"], true);
    assert_eq!(result["effectsVerified"], false);
    assert!(result["plan"]["execution"][0]["result"].get("stdout").is_none());
    assert_eq!(git(&repo, &["diff", "--cached", "--name-only"]), "first.txt");
}

#[cfg(test)]
mod blind_workflow_tests;
mod activity_tests;

#[test]
fn native_alert_dispatch_preserves_each_new_review_and_independent_preferences() {
    let (_dir, broker, credential, _repo) = fixture();
    let hits = Arc::new(Mutex::new(Vec::new()));
    let captured = hits.clone();
    broker.lock().unwrap().set_notifier(Arc::new(move |id| {
        captured.lock().unwrap().push(id.to_string());
    }));
    Broker::native(&broker, Native::AutoOpen { enabled: false }).unwrap();
    let first = propose(&broker, &credential, "first-alert", &[&["status", "--short"]]);
    let second = propose(&broker, &credential, "second-alert", &[&["status", "--short"]]);
    assert_eq!(hits.lock().unwrap().as_slice(), &[first["id"].as_str().unwrap(), second["id"].as_str().unwrap()]);
    assert!(broker.lock().unwrap().native_status(None)["focusPlanId"].is_null());
    assert_eq!(broker.lock().unwrap().review_alert_options(second["id"].as_str().unwrap()), Some((false, true)));
    broker.lock().unwrap().tool(&credential, "branchline_request_review", json!({"planId":second["id"]})).unwrap();
    assert_eq!(hits.lock().unwrap().len(), 2);
    Broker::native(&broker, Native::Notifications { enabled: false }).unwrap();
    propose(&broker, &credential, "quiet-alert", &[&["status", "--short"]]);
    assert_eq!(hits.lock().unwrap().len(), 2);
    let policy: Value = serde_json::from_slice(&fs::read(broker.lock().unwrap().dir.join("launch-policy.json")).unwrap()).unwrap();
    assert_eq!(policy["notifications"], false);
    assert!(broker.lock().unwrap().journal.plans.iter().all(|plan| plan.approval.is_none() && plan.execution.iter().all(|step| step.commands.is_empty())));
}
