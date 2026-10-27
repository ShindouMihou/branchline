use super::*;

#[test]
fn clear_activity_is_native_only_confirmed_and_preserves_live_requests() {
    let (dir, broker, credential, repo) = fixture();
    let plan = propose(&broker, &credential, "base", &[&["status", "--short"]]);
    let live = ["ready", "pending_review", "approved", "running"];
    let settled = ["succeeded", "failed", "partially_completed", "cancelled", "blocked", "needs_review", "outcome_unknown"];
    {
        let mut b = broker.lock().unwrap();
        let original = b.journal.plans[0].clone();
        b.journal.plans.clear();
        for status in live.iter().chain(settled.iter()) {
            let mut p = original.clone();
            p.id = format!("history-{status}");
            p.status = (*status).into();
            b.journal.plans.push(p);
        }
        b.focus_plan = Some("history-blocked".into());
        b.save().unwrap();
    }
    assert!(mcp::forward(&credential, "branchline_clear_activity", json!({"confirmed":true})).is_err());
    assert!(Broker::native(&broker, Native::ClearActivity {confirmed:false}).is_err());
    assert_eq!(broker.lock().unwrap().journal.plans.len(), live.len()+settled.len());
    let result = Broker::native(&broker, Native::ClearActivity {confirmed:true}).unwrap();
    assert_eq!(result["cleared"], settled.len());
    let b = broker.lock().unwrap();
    assert_eq!(b.journal.plans.iter().map(|p| p.status.as_str()).collect::<Vec<_>>(),live);
    assert!(b.focus_plan.is_none());
    let disk: Journal = serde_json::from_slice(&fs::read(dir.path().join("broker/journal-v1.json")).unwrap()).unwrap();
    assert_eq!(disk.plans.len(),live.len());
    assert_eq!(disk.clients.len(),b.journal.clients.len());
    assert_eq!(git(&repo,&["status","--short"]),"?? first.txt\n?? second.txt");
    drop(b);
    assert_eq!(Broker::native(&broker,Native::ClearActivity {confirmed:true}).unwrap()["cleared"],0);
    assert!(Broker::native(&broker,approval(&plan,1,true,true)).is_err());
}

#[test]
fn clear_activity_failure_restores_memory_and_old_approval_cannot_be_replayed() {
    let (dir,broker,credential,repo) = fixture();
    let plan = propose(&broker,&credential,"cleared-key",&[&["add","--","first.txt"]]);
    Broker::native(&broker,Native::Decline {plan_id:plan["id"].as_str().unwrap().into(),reason:None}).unwrap();
    let journal = dir.path().join("broker/journal-v1.json");
    let previous = fs::read(&journal).unwrap();
    fs::remove_file(&journal).unwrap();
    fs::create_dir(&journal).unwrap();
    assert!(Broker::native(&broker,Native::ClearActivity {confirmed:true}).is_err());
    assert_eq!(broker.lock().unwrap().journal.plans.len(),1);
    fs::remove_dir(&journal).unwrap();
    fs::write(&journal,previous).unwrap();
    Broker::native(&broker,Native::ClearActivity {confirmed:true}).unwrap();
    assert!(mcp::forward(&credential,"branchline_get_plan",json!({"planId":plan["id"]})).is_err());
    assert!(Broker::native(&broker,approval(&plan,1,true,true)).is_err());
    let new = propose(&broker,&credential,"cleared-key",&[&["add","--","first.txt"]]);
    assert_ne!(new["id"],plan["id"]);
    assert!(new["approval"].is_null());
    assert!(git(&repo,&["diff","--cached","--name-only"]).is_empty());
}
