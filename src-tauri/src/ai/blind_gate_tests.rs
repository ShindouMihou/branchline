use super::*;

fn fixture() -> (tempfile::TempDir, Shared, Credential, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    fs::create_dir(&repo).unwrap();
    let status = std::process::Command::new("git")
        .args(["init", "--quiet", "--initial-branch=main"])
        .current_dir(&repo)
        .status()
        .unwrap();
    assert!(status.success());
    let repo = fs::canonicalize(repo).unwrap();
    fs::write(repo.join("a file.txt"), "temporary fixture\n").unwrap();
    let broker = Broker::start(dir.path().join("broker")).unwrap();
    Broker::native(&broker, Native::Enable { enabled: true }).unwrap();
    let credential = register(&broker, &repo, "Blind gate host");
    (dir, broker, credential, repo)
}

fn register(broker: &Shared, repo: &Path, name: &str) -> Credential {
    let result = Broker::native(broker, Native::Register {
        name: name.into(), root: repo.to_string_lossy().into(), content: true,
        provider: Some("codex".into()),
    }).unwrap();
    serde_json::from_slice(&fs::read(result["credentialFile"].as_str().unwrap()).unwrap()).unwrap()
}

fn request(repo: &Path, key: &str, command: &str) -> Value {
    json!({"schemaVersion":1,"idempotencyKey":key,"repositoryPath":repo,"reportedAi":"Blind lifecycle model","command":command,"explanation":{"summary":"Exercise a disposable command workflow","reason":"Verify permission and result boundaries","impact":"Changes only the disposable fixture after native approval","uncertainties":["Process success does not verify all effects"]}})
}

fn propose(broker: &Shared, credential: &Credential, repo: &Path, key: &str, command: &str) -> Value {
    let plan = mcp::forward(credential, "branchline_propose_command", request(repo, key, command)).unwrap();
    assert_eq!(plan["status"], "ready", "{}", plan["issue"]);
    mcp::forward(credential, "branchline_request_review", json!({"planId":plan["id"]})).unwrap();
    assert!(broker.lock().unwrap().journal.plans.iter().any(|p| p.id == plan["id"].as_str().unwrap() && p.status == "pending_review"));
    plan
}

fn approval(plan: &Value, digest: &str) -> Native {
    Native::Approve {plan_id:plan["id"].as_str().unwrap().into(),digest:digest.into(),through:1,acknowledge:true,acknowledged_risks:plan["preview"]["risks"].as_array().unwrap().iter().map(|risk|risk["code"].as_str().unwrap().into()).collect()}
}

fn result(credential: &Credential, plan: &Value, timeout: u64) -> Value {
    mcp::forward(credential,"branchline_wait_for_result",json!({"planId":plan["id"],"timeoutMs":timeout})).unwrap()
}

#[test]
fn blind_gate_timeout_denial_reason_and_idempotency_never_authorize_commands() {
    let (_dir, broker, credential, repo) = fixture();
    let script = "git add -- 'a file.txt' && printf denied > should-not-exist.txt";
    let plan = propose(&broker,&credential,&repo,"deny-weird-spacing",script);
    let timed = result(&credential,&plan,120);
    assert_eq!(timed["waitOutcome"],"timed_out");
    assert_eq!(timed["grantsApproval"],false);
    assert_eq!(timed["succeeded"],false);
    assert!(timed["plan"]["approval"].is_null());
    assert!(!repo.join("should-not-exist.txt").exists());
    let reason = "Use another branch first.\nPlease explain the destination → review.";
    Broker::native(&broker,Native::Decline {plan_id:plan["id"].as_str().unwrap().into(),reason:Some(reason.into())}).unwrap();
    let denied=result(&credential,&plan,0);
    assert_eq!(denied["status"],"cancelled");
    assert_eq!(denied["terminal"],true);
    assert_eq!(denied["succeeded"],false);
    assert_eq!(denied["plan"]["issue"],reason);
    assert!(denied["plan"]["execution"][0]["commands"].as_array().unwrap().is_empty());
    let duplicate=mcp::forward(&credential,"branchline_propose_command",request(&repo,"deny-weird-spacing",script)).unwrap();
    assert_eq!(duplicate["id"],plan["id"]);
    assert_eq!(duplicate["status"],"cancelled");
    assert!(Broker::native(&broker,approval(&plan,plan["digest"].as_str().unwrap())).is_err());
    assert!(!repo.join("should-not-exist.txt").exists());
    assert!(!repo.join(".git/index").exists());
}

#[test]
fn blind_gate_rejects_digest_forgery_command_edits_and_remote_approval() {
    let (_dir,broker,credential,repo)=fixture();
    let script="git add -- 'a file.txt' && git status --short";
    let plan=propose(&broker,&credential,&repo,"immutable-command",script);
    assert_eq!(plan["requestedSteps"][0]["argv"][1],script);
    assert!(Broker::native(&broker,approval(&plan,"forged-digest")).is_err());
    assert!(mcp::forward(&credential,"branchline_approve",json!({"planId":plan["id"]})).is_err());
    assert!(mcp::forward(&credential,"branchline_propose_command",request(&repo,"immutable-command","git add .")).is_err());
    assert_eq!(result(&credential,&plan,0)["status"],"pending_review");
    assert!(!repo.join(".git/index").exists());
}

#[test]
fn blind_gate_connections_cannot_read_wait_cancel_or_queue_each_others_requests() {
    let (_dir,broker,credential,repo)=fixture();
    let other=register(&broker,&repo,"Other blind host");
    let plan=propose(&broker,&credential,&repo,"owned-plan","git status --short");
    for tool in ["branchline_get_plan","branchline_request_review","branchline_cancel_plan"] {
        assert!(mcp::forward(&other,tool,json!({"planId":plan["id"]})).is_err(),"{tool}");
    }
    assert!(mcp::forward(&other,"branchline_wait_for_result",json!({"planId":plan["id"],"timeoutMs":0})).is_err());
    let mut forged=other;
    forged.client_id=credential.client_id.clone();
    assert!(mcp::forward(&forged,"branchline_get_plan",json!({"planId":plan["id"]})).is_err());
    assert_eq!(result(&credential,&plan,0)["status"],"pending_review");
}

#[test]
fn blind_gate_one_approval_executes_once_and_audits_exact_command_without_process_output() {
    let (dir,broker,credential,repo)=fixture();
    let script="git add -- 'a file.txt' && printf x >> once.txt; printf 'BLIND_OUTPUT_%s\\n' secret; printf 'BLIND_ERROR_%s\\n' private >&2";
    let plan=propose(&broker,&credential,&repo,"once-output",script);
    Broker::native(&broker,approval(&plan,plan["digest"].as_str().unwrap())).unwrap();
    assert!(Broker::native(&broker,approval(&plan,plan["digest"].as_str().unwrap())).is_err());
    let done=result(&credential,&plan,5000);
    assert_eq!(done["succeeded"],true,"{done}");
    assert_eq!(done["effectsVerified"],false);
    assert_eq!(fs::read_to_string(repo.join("once.txt")).unwrap(),"x");
    assert_eq!(done["plan"]["clientName"],"Blind gate host");
    assert_eq!(done["plan"]["clientProvider"],"codex");
    assert_eq!(done["plan"]["reportedAi"],"Blind lifecycle model");
    let command=&done["plan"]["execution"][0]["commands"][0];
    assert_eq!(command["argv"][1],script);
    assert_eq!(command["outcome"],"process_succeeded_unverified");
    assert!(command["finishedAt"].as_u64().unwrap()>=command["startedAt"].as_u64().unwrap());
    let export=dir.path().join("export.json");
    Broker::native(&broker,Native::Export {plan_id:plan["id"].as_str().unwrap().into(),path:export.to_string_lossy().into()}).unwrap();
    for data in [done.to_string(),fs::read_to_string(export).unwrap(),fs::read_to_string(broker.lock().unwrap().dir.join("journal-v1.json")).unwrap()] {
        assert!(!data.contains("BLIND_OUTPUT_secret"));
        assert!(!data.contains("BLIND_ERROR_private"));
        assert!(!data.contains("\"stdout\":"));
        assert!(!data.contains("\"stderr\":"));
    }
}

#[test]
fn blind_gate_terminal_outcomes_survive_late_native_and_public_cancellation() {
    let (_dir,broker,credential,repo)=fixture();
    for (key,script,expected,timeout) in [
        ("failed-late-cancel","git add -- missing-file.txt","failed",None),
        ("unknown-late-cancel","git status --short && sleep 2","outcome_unknown",Some(1)),
    ] {
        let mut input=request(&repo,key,script);
        if let Some(seconds)=timeout { input["timeoutSeconds"]=json!(seconds); }
        let plan=mcp::forward(&credential,"branchline_propose_command",input).unwrap();
        assert_eq!(plan["status"],"ready");
        mcp::forward(&credential,"branchline_request_review",json!({"planId":plan["id"]})).unwrap();
        Broker::native(&broker,approval(&plan,plan["digest"].as_str().unwrap())).unwrap();
        let finished=result(&credential,&plan,5000);
        assert_eq!(finished["status"],expected);
        assert_eq!(finished["terminal"],true);
        let events=finished["plan"]["events"].clone();
        assert!(Broker::native(&broker,Native::Cancel {plan_id:plan["id"].as_str().unwrap().into()}).is_err());
        let cancelled=mcp::forward(&credential,"branchline_cancel_plan",json!({"planId":plan["id"]})).unwrap();
        assert_eq!(cancelled["status"],expected,"Late cancellation must not rewrite an executed terminal request");
        assert_eq!(cancelled["events"],events);
        assert_eq!(result(&credential,&plan,0)["status"],expected);
    }
}

#[test]
fn blind_gate_schema_and_wait_boundaries_do_not_mutate_pending_request() {
    let (_dir,broker,credential,repo)=fixture();
    let plan=propose(&broker,&credential,&repo,"schema-probe","git status --short");
    for args in [json!({"planId":plan["id"],"timeoutMs":-1}),json!({"planId":plan["id"],"timeoutMs":25001}),json!({"planId":plan["id"],"timeoutMs":"0"}),json!({"planId":plan["id"],"approve":true})] {
        assert!(mcp::forward(&credential,"branchline_wait_for_result",args).is_err());
    }
    let mut extra=request(&repo,"schema-extra","git add .");
    extra["approve"]=json!(true);
    assert!(mcp::forward(&credential,"branchline_propose_command",extra).is_err());
    let mut version=request(&repo,"schema-version","git add .");
    version["schemaVersion"]=json!(2);
    assert!(mcp::forward(&credential,"branchline_propose_command",version).is_err());
    for _ in 0..3 {
        let again=mcp::forward(&credential,"branchline_request_review",json!({"planId":plan["id"]})).unwrap();
        assert_eq!(again["events"].as_array().unwrap().iter().filter(|event|event["kind"]=="review_requested").count(),1);
    }
    let pending=result(&credential,&plan,0);
    assert_eq!(pending["status"],"pending_review");
    assert!(pending["plan"]["approval"].is_null());
    assert!(pending["plan"]["execution"][0]["commands"].as_array().unwrap().is_empty());
    assert!(!repo.join(".git/index").exists());
}
mod cycles;
