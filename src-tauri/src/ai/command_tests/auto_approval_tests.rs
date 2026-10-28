use super::*;
use crate::ai::auto_approval::{GrantRecord, Operation};

fn grant(broker: &Shared, credential: &Credential, repo: &Path, operations: &[&str], wide: bool) {
    let mut request=json!({"clientId":credential.client_id,"repository":repo,"sessionWide":wide,"operations":operations,
        "remote":"origin","branch":"feat/test","githubRepository":"example/project","base":"main","minutes":30,"confirmed":true});
    let preview=Broker::native(broker,serde_json::from_value(json!({"op":"auto_approval_preview","request":request})).unwrap()).unwrap();
    request["previewDigest"]=preview["digest"].clone();
    Broker::native(broker,serde_json::from_value(json!({"op":"auto_approval_grant","request":request})).unwrap()).unwrap();
}
fn command(broker: &Shared, credential: &Credential, repo: &Path, key: &str, script: &str) -> Value {
    broker.lock().unwrap().tool(credential,"branchline_propose_command",json!({"schemaVersion":1,"idempotencyKey":key,"repositoryPath":repo,"command":script,"explanation":{"summary":"Fixture operation","reason":"Test native policies","impact":"Disposable repository only","uncertainties":[]}})).unwrap()
}
fn matches(broker: &Shared, plan: &Value) -> bool {
    let b = broker.lock().unwrap();
    b.auto_approval.matching(b.journal.plans.iter().find(|p| p.id == plan["id"]).unwrap()).is_some()
}
fn request(credential: &Credential, plan: &Value) {
    mcp::forward(credential,"branchline_request_review",json!({"planId":plan["id"]})).unwrap();
}

#[test]
fn policies_default_to_manual_and_cannot_be_enabled_via_mcp() {
    let (_dir,broker,credential,repo) = fixture();
    let plan=command(&broker,&credential,&repo,"manual","git add -- first.txt");
    request(&credential,&plan);
    assert_eq!(broker.lock().unwrap().plan_mut(plan["id"].as_str().unwrap()).unwrap().status,"pending_review");
    assert!(git(&repo,&["diff","--cached","--name-only"]).is_empty());
    assert!(mcp::forward(&credential,"auto_approval_grant",json!({"confirmed":true})).is_err());
    let mut input=json!({"op":"auto_approval_grant","request":{"clientId":credential.client_id,"repository":repo,"sessionWide":true,"operations":[],"confirmed":false}});
    assert!(Broker::native(&broker,serde_json::from_value(input.clone()).unwrap()).is_err());
    input["request"]["confirmed"]=json!(true);input["request"]["minutes"]=json!(0);
    assert!(Broker::native(&broker,serde_json::from_value(input).unwrap()).is_err());
    grant(&broker,&credential,&repo,&["stage"],false);
    request(&credential,&plan);
    assert!(!matches(&broker,&plan));
    assert_eq!(broker.lock().unwrap().plan_mut(plan["id"].as_str().unwrap()).unwrap().status,"pending_review");
}

#[test]
fn selected_rules_run_whole_literal_requests_once_and_audit_the_grant() {
    let (dir,broker,credential,repo) = fixture();
    grant(&broker,&credential,&repo,&["stage"],false);
    let plan=command(&broker,&credential,&repo,"auto-stage","git diff --cached --check && git add -- 'first.txt' && git add -- second.txt");
    assert!(matches(&broker,&plan));
    request(&credential,&plan);
    let completed=wait(&broker,&plan);
    assert_eq!(completed.status,"succeeded", "{:?}", completed.issue);
    assert_eq!(completed.approval.as_ref().unwrap().authority,"native_policy");
    let record=completed.approval.as_ref().unwrap().policy.as_ref().unwrap();
    assert_eq!(record.operations,vec![Operation::Stage]);
    assert_eq!(record.client_id,credential.client_id);
    assert_eq!(git(&repo,&["diff","--cached","--name-only"]),"first.txt\nsecond.txt");
    request(&credential,&plan);request(&credential,&plan);
    assert_eq!(broker.lock().unwrap().plan_mut(&completed.id).unwrap().execution[0].commands.len(),1);
    let journal:Value=serde_json::from_slice(&fs::read(dir.path().join("broker/journal-v1.json")).unwrap()).unwrap();
    assert!(journal.get("autoApproval").is_none());
    Broker::native(&broker,Native::AutoApprovalPause).unwrap();
    assert!(broker.lock().unwrap().auto_approval.status().is_empty());
    assert!(broker.lock().unwrap().plan_mut(&completed.id).unwrap().approval.as_ref().unwrap().policy.is_some());
}

#[test]
fn selected_rules_reject_unmatched_or_ambiguous_effects_without_running_a_prefix() {
    let (_dir,broker,credential,repo) = fixture();
    grant(&broker,&credential,&repo,&["stage","commit"],false);
    for (i,script) in [
        "git add -- first.txt && git push origin feat/test",
        "git commit --amend -m message", "git -c core.hooksPath=/tmp commit -m message",
        "git add -- first.txt; printf injected", "git add -- first.txt || git commit -m fallback",
        "git add -- first.txt &", "git add -- $(printf first.txt)",
        "git add -- first.txt > output.txt", "git add -- first.txt | cat",
        "cd /tmp && git add -- first.txt", "GIT_DIR=/tmp git add -- first.txt",
        "if true; then git add -- first.txt; fi", "eval 'git add -- first.txt'",
        "git add -- first.txt\nexit 0", "git commit --all -m message", "git commit -m {one,two}", "git commit -m prefix{one,two}",
    ].iter().enumerate() {
        let plan=command(&broker,&credential,&repo,&format!("ambiguous-{i}"),script);
        assert!(!matches(&broker,&plan),"{script}");
    }
    let plan=command(&broker,&credential,&repo,"no-prefix","git add -- first.txt && git push origin feat/test");
    request(&credential,&plan);
    assert_eq!(broker.lock().unwrap().plan_mut(plan["id"].as_str().unwrap()).unwrap().status,"pending_review");
    assert!(git(&repo,&["diff","--cached","--name-only"]).is_empty());
    assert!(matches(&broker,&command(&broker,&credential,&repo,"literal","git commit -m 'literal && git push; $NOT_EXECUTED'")));
}

#[test]
fn broad_session_grants_are_explicit_scoped_and_pause_stops_queued_work() {
    let (_dir,broker,credential,repo) = fixture();
    grant(&broker,&credential,&repo,&[],true);
    let plan=command(&broker,&credential,&repo,"wide","printf fixture > session-output.txt");
    assert!(matches(&broker,&plan));
    {
        let b=broker.lock().unwrap();
        let mut other=b.journal.plans.last().unwrap().clone();
        other.client_id="another-client".into();assert!(b.auto_approval.matching(&other).is_none());
        other.client_id=credential.client_id.clone();other.repository="/tmp/another-repository".into();assert!(b.auto_approval.matching(&other).is_none());
    }
    let gate=broker.lock().unwrap().operation_gate.clone();
    let held=gate.lock().unwrap();
    request(&credential,&plan);
    assert_eq!(broker.lock().unwrap().plan_mut(plan["id"].as_str().unwrap()).unwrap().status,"approved");
    Broker::native(&broker,Native::AutoApprovalPause).unwrap();
    drop(held);
    let completed=wait(&broker,&plan);
    assert_eq!(completed.status,"pending_review");
    assert!(completed.approval.is_none());assert!(!repo.join("session-output.txt").exists());
    grant(&broker,&credential,&repo,&[],true);
    let next=command(&broker,&credential,&repo,"wide-next","printf fixture > session-output.txt");
    request(&credential,&next);assert_eq!(wait(&broker,&next).status,"succeeded");
    assert_eq!(fs::read_to_string(repo.join("session-output.txt")).unwrap(),"fixture");
    Broker::native(&broker,Native::Enable { enabled:false }).unwrap();
    assert!(broker.lock().unwrap().auto_approval.status().is_empty());
}

#[test]
fn policy_cannot_bypass_stale_preview_or_connection_revocation() {
    let (_dir,broker,credential,repo) = fixture();
    grant(&broker,&credential,&repo,&["stage"],false);
    let plan=command(&broker,&credential,&repo,"stale-auto","git add -- first.txt");
    broker.lock().unwrap().plan_mut(plan["id"].as_str().unwrap()).unwrap().preview.as_mut().unwrap().fingerprint="changed".into();
    request(&credential,&plan);
    let p=broker.lock().unwrap().plan_mut(plan["id"].as_str().unwrap()).unwrap().clone();
    assert_eq!(p.status,"needs_review");assert!(p.approval.is_none());
    assert!(git(&repo,&["diff","--cached","--name-only"]).is_empty());
    Broker::native(&broker,Native::Revoke {client_id:credential.client_id}).unwrap();
    assert!(broker.lock().unwrap().auto_approval.status().is_empty());
}

#[test]
fn pr_and_push_rules_bind_exact_targets_and_reject_unknown_flags() {
    let (_dir,broker,credential,repo) = fixture();
    let remote=repo.parent().unwrap().join("remote.git");fs::create_dir(&remote).unwrap();git(&remote,&["init","--bare"]);
    git(&repo,&["remote","add","origin",remote.to_str().unwrap()]);
    git(&repo,&["add","--","first.txt"]);
    git(&repo,&["-c","user.name=Fixture","-c","user.email=fixture@example.test","commit","-m","Fixture"]);
    git(&repo,&["branch","feat/test"]);
    grant(&broker,&credential,&repo,&["push","pr_create","pr_edit"],false);
    for (i,script) in [
        "git push origin feat/test", "git push --set-upstream origin feat/test",
        "gh pr create --repo github.com/example/project --head feat/test --base main --title Title --body Body",
        "gh pr edit 1 --repo github.com/example/project --title Title --body Body",
    ].iter().enumerate() { assert!(matches(&broker,&command(&broker,&credential,&repo,&format!("target-{i}"),script)),"{script}"); }
    for (i,script) in [
        "git push origin main", "git push --force origin feat/test", "git push origin HEAD:feat/test",
        "git push origin feat/test:another-branch", "git push --atomic origin feat/test",
        "gh pr create --repo github.com/example/project --head feat/test --base develop --title Title --body Body",
        "gh pr edit 1 --repo github.com/example/project --base develop",
        "gh pr edit 1 --repo github.com/example/project --title Title --add-label bug",
        "gh pr edit 1 --repo other/project --title Title", "gh pr edit 1 --repo example/project --title Title",
        "gh pr edit 1 --repo github.com/example/project --body-file /tmp/body.md",
    ].iter().enumerate() { assert!(!matches(&broker,&command(&broker,&credential,&repo,&format!("wrong-target-{i}"),script)),"{script}"); }
    git(&repo,&["switch","feat/test"]);
    assert!(matches(&broker,&command(&broker,&credential,&repo,"head-matches","git push origin HEAD:refs/heads/feat/test")));
    let plan=command(&broker,&credential,&repo,"remote-change","git push origin feat/test");
    git(&repo,&["remote","set-url","origin","/tmp/different-fixture-remote"]);
    assert!(!matches(&broker,&plan));
    let _:Vec<GrantRecord>=broker.lock().unwrap().auto_approval.status();
}

#[test]
fn a_new_broker_never_restores_authority_from_policy_receipts() {
    let (dir,broker,credential,repo)=fixture();
    grant(&broker,&credential,&repo,&["stage"],false);
    let plan=command(&broker,&credential,&repo,"restart-policy","git add -- first.txt");
    request(&credential,&plan);assert_eq!(wait(&broker,&plan).status,"succeeded");
    let restored_dir=dir.path().join("restored-broker");fs::create_dir(&restored_dir).unwrap();
    fs::copy(dir.path().join("broker/journal-v1.json"),restored_dir.join("journal-v1.json")).unwrap();
    let restored=Broker::start(restored_dir).unwrap();
    let status=Broker::native(&restored,Native::Status {plan_id:None}).unwrap();
    assert!(status["autoApproval"].as_array().unwrap().is_empty());
    assert_eq!(status["plans"][0]["approval"]["authority"],"native_policy");
}

#[test]
fn supplied_pr_input_must_belong_to_one_command_and_is_never_a_general_pipe() {
    let (_dir,broker,credential,repo)=fixture();
    grant(&broker,&credential,&repo,&["pr_edit"],false);
    let propose_input=|key:&str,script:&str| broker.lock().unwrap().tool(&credential,"branchline_propose_command",json!({"schemaVersion":1,"idempotencyKey":key,"repositoryPath":repo,"command":script,"stdin":"Description","explanation":{"summary":"Edit fixture PR","reason":"Test input ownership","impact":"No real GitHub calls","uncertainties":[]}})).unwrap();
    let single=propose_input("single-input","gh pr edit 1 --repo github.com/example/project --body-file -");
    assert!(matches(&broker,&single));
    let chain=propose_input("ambiguous-input","git status && gh pr edit 1 --repo github.com/example/project --body-file -");
    assert!(!matches(&broker,&chain));
    let unrelated=propose_input("unused-input","gh pr edit 1 --repo github.com/example/project --title Title");
    assert!(!matches(&broker,&unrelated));
}

#[test]
fn pausing_between_review_and_policy_dispatch_restores_the_manual_prompt_once() {
    let (_dir,broker,credential,repo)=fixture();
    let alerts=Arc::new(Mutex::new(0));let observed=alerts.clone();
    broker.lock().unwrap().set_notifier(Arc::new(move |_| *observed.lock().unwrap()+=1));
    grant(&broker,&credential,&repo,&["stage"],false);
    let plan=command(&broker,&credential,&repo,"dispatch-race","git add -- first.txt");
    broker.lock().unwrap().tool(&credential,"branchline_request_review",json!({"planId":plan["id"]})).unwrap();
    assert_eq!(*alerts.lock().unwrap(),0);
    Broker::native(&broker,Native::AutoApprovalPause).unwrap();
    crate::ai::auto_approval::dispatch(&broker,plan["id"].as_str().unwrap());
    crate::ai::auto_approval::dispatch(&broker,plan["id"].as_str().unwrap());
    assert_eq!(*alerts.lock().unwrap(),1);
    assert_eq!(broker.lock().unwrap().plan_mut(plan["id"].as_str().unwrap()).unwrap().status,"pending_review");
    assert!(git(&repo,&["diff","--cached","--name-only"]).is_empty());
}

#[test]
fn blocked_requests_are_not_hidden_by_broad_session_rules() {
    let (_dir,broker,credential,repo)=fixture();
    let alerts=Arc::new(Mutex::new(0));let observed=alerts.clone();
    broker.lock().unwrap().set_notifier(Arc::new(move |_| *observed.lock().unwrap()+=1));
    grant(&broker,&credential,&repo,&[],true);
    let plan=command(&broker,&credential,&repo,"blocked-wide","git add -- first.txt");
    {let mut b=broker.lock().unwrap();let p=b.plan_mut(plan["id"].as_str().unwrap()).unwrap();p.status="blocked".into();p.preview=None;}
    request(&credential,&plan);
    assert_eq!(*alerts.lock().unwrap(),1);
    assert!(!matches(&broker,&plan));
    assert!(git(&repo,&["diff","--cached","--name-only"]).is_empty());
}

#[test]
fn confirming_a_rule_binds_the_reviewed_scope_and_current_push_url() {
    let (_dir,broker,credential,repo)=fixture();
    git(&repo,&["remote","add","origin","/tmp/original-fixture-remote"]);
    let mut request=json!({"clientId":credential.client_id,"repository":repo,"sessionWide":false,"operations":["push"],"remote":"origin","branch":"feat/test","confirmed":false});
    let preview=Broker::native(&broker,serde_json::from_value(json!({"op":"auto_approval_preview","request":request})).unwrap()).unwrap();
    assert_eq!(preview["scope"]["remoteUrl"],"/tmp/original-fixture-remote");
    assert!(broker.lock().unwrap().auto_approval.status().is_empty());
    request["confirmed"]=json!(true);request["previewDigest"]=preview["digest"].clone();
    git(&repo,&["remote","set-url","origin","/tmp/changed-fixture-remote"]);
    assert!(Broker::native(&broker,serde_json::from_value(json!({"op":"auto_approval_grant","request":request})).unwrap()).unwrap_err().contains("scope changed"));
    let preview=Broker::native(&broker,serde_json::from_value(json!({"op":"auto_approval_preview","request":request})).unwrap()).unwrap();
    request["previewDigest"]=preview["digest"].clone();request["sessionWide"]=json!(true);request["operations"]=json!([]);
    assert!(Broker::native(&broker,serde_json::from_value(json!({"op":"auto_approval_grant","request":request})).unwrap()).is_err());
    assert!(broker.lock().unwrap().auto_approval.status().is_empty());
}

#[test]
fn global_rules_cover_repositories_only_for_the_selected_connection() {
    let (dir,broker,credential,repo)=fixture();
    let other=dir.path().join("second-repo");fs::create_dir(&other).unwrap();git(&other,&["init","-b","main"]);fs::write(other.join("global.txt"),"fixture\n").unwrap();
    let mut input=json!({"clientId":credential.client_id,"repository":"","allRepositories":true,"sessionWide":false,"operations":["stage"],"confirmed":false});
    let preview=Broker::native(&broker,serde_json::from_value(json!({"op":"auto_approval_preview","request":input})).unwrap()).unwrap();
    assert_eq!(preview["scope"]["allRepositories"],true);
    input["confirmed"]=json!(true);input["previewDigest"]=preview["digest"].clone();
    Broker::native(&broker,serde_json::from_value(json!({"op":"auto_approval_grant","request":input})).unwrap()).unwrap();
    let plan=command(&broker,&credential,&other,"global-other-repo","git add -- global.txt");
    assert!(matches(&broker,&plan));request(&credential,&plan);
    let completed=wait(&broker,&plan);assert_eq!(completed.status,"succeeded");
    let snapshot=completed.approval.as_ref().unwrap().policy.as_ref().unwrap();
    assert!(snapshot.all_repositories);assert_eq!(snapshot.resolved_repository.as_deref(),Some(completed.repository.as_str()));
    assert!(git(&repo,&["diff","--cached","--name-only"]).is_empty());
    let mut impostor=completed.clone();impostor.status="ready".into();impostor.client_id="different-connection".into();
    assert!(broker.lock().unwrap().auto_approval.matching(&impostor).is_none());
    Broker::native(&broker,Native::AutoApprovalPause).unwrap();
    assert!(!broker.lock().unwrap().auto_approval.active(snapshot,&completed));
}

#[test]
fn global_push_approval_snapshots_the_remote_for_its_actual_repository() {
    let (_dir,broker,credential,repo)=fixture();
    git(&repo,&["add","--","first.txt"]);git(&repo,&["-c","user.name=Fixture","-c","user.email=fixture@example.test","commit","-m","Fixture"]);
    git(&repo,&["branch","feat/test"]);git(&repo,&["remote","add","origin","/tmp/global-fixture-one"]);
    let mut input=json!({"clientId":credential.client_id,"repository":"","allRepositories":true,"sessionWide":false,"operations":["push"],"remote":"origin","branch":"feat/test","confirmed":false});
    let preview=Broker::native(&broker,serde_json::from_value(json!({"op":"auto_approval_preview","request":input})).unwrap()).unwrap();
    input["confirmed"]=json!(true);input["previewDigest"]=preview["digest"].clone();
    Broker::native(&broker,serde_json::from_value(json!({"op":"auto_approval_grant","request":input})).unwrap()).unwrap();
    let plan=command(&broker,&credential,&repo,"global-push-drift","git push origin feat/test");
    let snapshot={let b=broker.lock().unwrap();let p=b.journal.plans.iter().find(|p|p.id==plan["id"]).unwrap();b.auto_approval.matching(p).unwrap()};
    assert_eq!(snapshot.resolved_push_url.as_deref(),Some("/tmp/global-fixture-one"));
    git(&repo,&["remote","set-url","origin","/tmp/global-fixture-two"]);
    let b=broker.lock().unwrap();let p=b.journal.plans.iter().find(|p|p.id==plan["id"]).unwrap();
    assert!(!b.auto_approval.active(&snapshot,p));
}
