use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn gate_cycle_01_only_reviews_alert_and_focus_respects_independent_preferences() {
    let (_dir,broker,credential,repo)=fixture();
    let raised=Arc::new(AtomicUsize::new(0));
    let count=raised.clone();
    broker.lock().unwrap().set_notifier(Arc::new(move |_| {count.fetch_add(1,Ordering::SeqCst);}));
    let plan=mcp::forward(&credential,"branchline_propose_command",request(&repo,"no-premature-focus","git status --short")).unwrap();
    mcp::forward(&credential,"branchline_inspect",json!({"repositoryPath":repo,"query":"status"})).unwrap();
    mcp::forward(&credential,"branchline_get_plan",json!({"planId":plan["id"]})).unwrap();
    result(&credential,&plan,0);
    assert_eq!(raised.load(Ordering::SeqCst),0);
    for _ in 0..3 {mcp::forward(&credential,"branchline_request_review",json!({"planId":plan["id"]})).unwrap();}
    assert_eq!(raised.load(Ordering::SeqCst),1);
    Broker::native(&broker,Native::AutoOpen {enabled:false}).unwrap();
    let background = propose(&broker,&credential,&repo,"focus-disabled","git status --short");
    assert_eq!(raised.load(Ordering::SeqCst),2);
    assert_ne!(broker.lock().unwrap().native_status(None)["focusPlanId"], background["id"]);
    assert_eq!(broker.lock().unwrap().review_alert_options(background["id"].as_str().unwrap()), Some((false, true)));
    Broker::native(&broker,Native::Notifications {enabled:false}).unwrap();
    propose(&broker,&credential,&repo,"all-alerts-disabled","git status --short");
    assert_eq!(raised.load(Ordering::SeqCst),2);
    assert!(!repo.join(".git/index").exists());
}

#[test]
fn gate_cycle_02_revocation_and_connector_disable_invalidate_pending_approval() {
    for revoke in [true,false] {
        let (_dir,broker,credential,repo)=fixture();
        let plan=propose(&broker,&credential,&repo,"remove-permission","git add -- 'a file.txt'");
        let action=if revoke {Native::Revoke {client_id:credential.client_id.clone()}} else {Native::Enable {enabled:false}};
        Broker::native(&broker,action).unwrap();
        assert!(Broker::native(&broker,approval(&plan,plan["digest"].as_str().unwrap())).is_err());
        assert!(mcp::forward(&credential,"branchline_get_plan",json!({"planId":plan["id"]})).is_err());
        let saved=broker.lock().unwrap().journal.plans[0].clone();
        assert_eq!(saved.status,"cancelled");
        assert!(saved.approval.is_none());
        assert!(saved.execution[0].commands.is_empty());
        assert!(!repo.join(".git/index").exists());
    }
}

#[test]
fn gate_cycle_03_idempotency_and_reported_identity_are_connection_specific() {
    let (_dir,broker,credential,repo)=fixture();
    let other=register(&broker,&repo,"Second blind owner");
    let first=propose(&broker,&credential,&repo,"shared-key","git add -- 'a file.txt'");
    let second=propose(&broker,&other,&repo,"shared-key","git status --short");
    assert_ne!(first["id"],second["id"]);
    Broker::native(&broker,Native::ClientType {client_id:credential.client_id.clone(),provider:"claude-desktop".into()}).unwrap();
    Broker::native(&broker,Native::Decline {plan_id:first["id"].as_str().unwrap().into(),reason:None}).unwrap();
    assert_eq!(result(&credential,&first,0)["plan"]["clientProvider"],"codex");
    assert_eq!(result(&other,&second,0)["status"],"pending_review");
    assert_eq!(result(&other,&second,0)["plan"]["clientName"],"Second blind owner");
    assert!(!repo.join(".git/index").exists());
}

#[test]
fn gate_cycle_04_invalid_denial_reason_keeps_request_pending_and_valid_reason_is_final() {
    let (_dir,broker,credential,repo)=fixture();
    let plan=propose(&broker,&credential,&repo,"denial-boundaries","git add -- 'a file.txt'");
    for reason in ["   ".to_string(),"x".repeat(2001)] {
        assert!(Broker::native(&broker,Native::Decline {plan_id:plan["id"].as_str().unwrap().into(),reason:Some(reason)}).is_err());
        assert_eq!(result(&credential,&plan,0)["status"],"pending_review");
    }
    let reason="x".repeat(2000);
    Broker::native(&broker,Native::Decline {plan_id:plan["id"].as_str().unwrap().into(),reason:Some(reason.clone())}).unwrap();
    assert!(Broker::native(&broker,Native::Decline {plan_id:plan["id"].as_str().unwrap().into(),reason:Some("replacement".into())}).is_err());
    let final_result=result(&credential,&plan,0);
    assert_eq!(final_result["plan"]["issue"],reason);
    assert!(final_result["plan"]["execution"][0]["commands"].as_array().unwrap().is_empty());
}

fn wait_for_broker_workers(broker: &Shared) {
    let started=std::time::Instant::now();
    while Arc::strong_count(broker)>2 {
        assert!(started.elapsed().as_secs()<2,"Broker workers did not finish");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[test]
fn gate_cycle_05_queued_cancellation_and_revocation_keep_their_recorded_outcomes() {
    for action in 0..4 {
        let (_dir,broker,credential,repo)=fixture();
        let plan=propose(&broker,&credential,&repo,"queue-cancel","git add -- 'a file.txt'");
        wait_for_broker_workers(&broker);
        let gate=broker.lock().unwrap().operation_gate.clone();
        let held=gate.lock().unwrap();
        Broker::native(&broker,approval(&plan,plan["digest"].as_str().unwrap())).unwrap();
        match action {
            0=>{Broker::native(&broker,Native::Cancel {plan_id:plan["id"].as_str().unwrap().into()}).unwrap();},
            1=>{mcp::forward(&credential,"branchline_cancel_plan",json!({"planId":plan["id"]})).unwrap();},
            2=>{Broker::native(&broker,Native::Revoke {client_id:credential.client_id.clone()}).unwrap();},
            _=>{Broker::native(&broker,Native::Enable {enabled:false}).unwrap();},
        }
        let events=broker.lock().unwrap().journal.plans[0].events.len();
        drop(held);
        wait_for_broker_workers(&broker);
        let finished=broker.lock().unwrap().journal.plans[0].clone();
        assert_eq!(finished.status,"cancelled","Queued cancellation is a user decision, not needs_review");
        assert_eq!(finished.events.len(),events);
        assert!(finished.execution[0].commands.is_empty());
        assert!(!repo.join(".git/index").exists());
    }
}

#[test]
fn gate_cycle_06_parallel_waiters_receive_the_same_denial_without_execution() {
    let (_dir,broker,credential,repo)=fixture();
    let plan=propose(&broker,&credential,&repo,"parallel-wait","git add -- 'a file.txt'");
    let barrier=Arc::new(std::sync::Barrier::new(3));
    let mut waiting=Vec::new();
    for _ in 0..2 {
        let credential:Credential=serde_json::from_value(serde_json::to_value(&credential).unwrap()).unwrap();
        let plan=plan.clone();let barrier=barrier.clone();
        waiting.push(std::thread::spawn(move || {barrier.wait();result(&credential,&plan,5000)}));
    }
    barrier.wait();
    Broker::native(&broker,Native::Decline {plan_id:plan["id"].as_str().unwrap().into(),reason:Some("Stop this attempt".into())}).unwrap();
    for waiter in waiting {
        let outcome=waiter.join().unwrap();
        assert_eq!(outcome["status"],"cancelled");
        assert_eq!(outcome["grantsApproval"],false);
        assert_eq!(outcome["plan"]["issue"],"Stop this attempt");
    }
    assert!(!repo.join(".git/index").exists());
}

#[test]
fn gate_cycle_07_expired_queued_approval_never_starts_the_command() {
    let (_dir,broker,credential,repo)=fixture();
    let plan=propose(&broker,&credential,&repo,"expired-approval","git add -- 'a file.txt'");
    let gate=broker.lock().unwrap().operation_gate.clone();
    let held=gate.lock().unwrap();
    Broker::native(&broker,approval(&plan,plan["digest"].as_str().unwrap())).unwrap();
    broker.lock().unwrap().journal.plans[0].approval.as_mut().unwrap().expires_at=0;
    drop(held);
    let outcome=result(&credential,&plan,5000);
    assert_eq!(outcome["status"],"cancelled");
    assert_eq!(outcome["succeeded"],false);
    assert!(outcome["plan"]["execution"][0]["commands"].as_array().unwrap().is_empty());
    assert!(!repo.join(".git/index").exists());
}

#[test]
fn gate_cycle_08_source_permission_changes_apply_to_existing_plans_and_inspection() {
    let (_dir,broker,credential,repo)=fixture();
    let plan=propose(&broker,&credential,&repo,"source-access","git add -- 'a file.txt' && git commit -m Test");
    assert!(!plan["preview"]["commandDetails"].as_array().unwrap().is_empty());
    Broker::native(&broker,Native::ReadSource {client_id:credential.client_id.clone(),enabled:false}).unwrap();
    let hidden=mcp::forward(&credential,"branchline_get_plan",json!({"planId":plan["id"]})).unwrap();
    assert!(hidden["preview"].get("commandDetails").is_none());
    assert!(hidden["preview"].get("patch").is_none());
    assert!(mcp::forward(&credential,"branchline_inspect",json!({"repositoryPath":repo,"query":"staged_diff"})).is_err());
    assert_eq!(hidden["requestedSteps"][0]["argv"][1],plan["requestedSteps"][0]["argv"][1]);
    Broker::native(&broker,Native::ReadSource {client_id:credential.client_id.clone(),enabled:true}).unwrap();
    let shown=result(&credential,&plan,0);
    assert_eq!(shown["plan"]["preview"]["commandDetails"],plan["preview"]["commandDetails"]);
    assert!(shown["plan"]["approval"].is_null());
    assert!(!repo.join(".git/index").exists());
}
