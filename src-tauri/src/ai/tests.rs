use super::*;

#[test]
fn native_file_review_is_read_only_allowlisted_and_unavailable_through_mcp() {
    let (_dir, broker, credential, repo) = fixture();
    staged(&repo);
    fs::write(repo.join("private.txt"), "Unrelated private file\n").unwrap();
    let head = git(&repo, &["rev-parse", "HEAD"]);
    let propose = |key: &str, command: &str| mcp::forward(&credential, "branchline_propose_command", json!({"schemaVersion":1,"idempotencyKey":key,"repositoryPath":repo,"command":command,"explanation":{"summary":"Review the proposed commit","reason":"Check the selected file","impact":"Only runs after approval","uncertainties":[]}})).unwrap();
    let plan = propose("native-diff", "git commit -m Reviewed");
    let id = plan["id"].as_str().unwrap().to_string();
    let journal = serde_json::to_vec(&broker.lock().unwrap().journal).unwrap();
    let index = fs::read(repo.join(".git/index")).unwrap();
    let result = Broker::native(&broker, Native::ReviewDiff { plan_id: id.clone(), detail_index: 0, path: "sample.txt".into() }).unwrap();
    assert_eq!(result["source"], "index");
    assert!(result["patch"].as_str().unwrap().contains("-before"));
    assert!(result["patch"].as_str().unwrap().contains("+after"));
    assert_eq!(result["added"], 1);
    assert_eq!(result["removed"], 1);
    assert!(Broker::native(&broker, Native::ReviewDiff { plan_id: id.clone(), detail_index: 0, path: "private.txt".into() }).is_err());
    assert!(mcp::forward(&credential, "branchline_review_diff", json!({"planId":id,"detailIndex":0,"path":"sample.txt"})).is_err());
    let disclosed = mcp::forward(&credential, "branchline_get_plan", json!({"planId":id})).unwrap();
    assert!(!disclosed.to_string().contains("+after"));
    assert_eq!(serde_json::to_vec(&broker.lock().unwrap().journal).unwrap(), journal);
    assert_eq!(fs::read(repo.join(".git/index")).unwrap(), index);
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), head);
    fs::remove_file(repo.join("sample.txt")).unwrap();
    let deletion = propose("native-deletion-diff", "git add sample.txt");
    let result = Broker::native(&broker, Native::ReviewDiff { plan_id: deletion["id"].as_str().unwrap().into(), detail_index: 0, path: "sample.txt".into() }).unwrap();
    assert_eq!(result["source"], "worktree");
    assert_eq!(result["removed"], 1);
    assert_eq!(result["added"], 0);
    assert_eq!(result["baseline"], "index");
    assert!(result["patch"].as_str().unwrap().contains("-after"));
    assert!(broker.lock().unwrap().journal.plans.iter().all(|plan| plan.approval.is_none() && plan.execution.iter().all(|step| step.commands.is_empty())));
}
fn git(root: &Path, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .current_dir(root)
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "Git fixture command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().into()
}
fn fixture() -> (tempfile::TempDir, Shared, Credential, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "-b", "main"]);
    git(&repo, &["config", "user.name", "Branchline Tests"]);
    git(&repo, &["config", "user.email", "test@example.invalid"]);
    git(&repo, &["config", "commit.gpgsign", "false"]);
    fs::write(repo.join("sample.txt"), "before\n").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "Initial"]);
    let repo = fs::canonicalize(repo).unwrap();
    let b = Broker::start(dir.path().join("broker")).unwrap();
    Broker::native(&b, Native::Enable { enabled: true }).unwrap();
    let registered = Broker::native(
        &b,
        Native::Register {
            name: "Paired test host".into(),
            root: repo.to_string_lossy().into_owned(),
            content: true,
            provider: None,
        },
    )
    .unwrap();
    let credential: Credential =
        serde_json::from_slice(&fs::read(registered["credentialFile"].as_str().unwrap()).unwrap())
            .unwrap();
    (dir, b, credential, repo)
}
fn proposed(b: &Shared, c: &Credential, key: &str, push: bool) -> Value {
    let mut steps = vec![json!({"kind":"commit.create","message":"Improve sample"})];
    if push {
        steps.push(json!({"kind":"remote.push","remote":"origin","branch":"main"}))
    }
    b.lock().unwrap().tool(c,"branchline_propose_plan",json!({"schemaVersion":1,"idempotencyKey":key,"reportedAi":"Self-reported test model","explanation":{"summary":"Save the reviewed change","reason":"Keep the completed work in history.","impact":"Creates a local commit, and publishes only if you approve that step.","uncertainties":[]},"steps":steps})).unwrap()
}
fn staged(repo: &Path) {
    fs::write(repo.join("sample.txt"), "after\n").unwrap();
    git(repo, &["add", "sample.txt"]);
}
fn review(b: &Shared, c: &Credential, p: &Value) {
    b.lock()
        .unwrap()
        .tool(c, "branchline_request_review", json!({"planId":p["id"]}))
        .unwrap();
}
fn approve(b: &Shared, p: &Value, through: usize) {
    Broker::native(
        b,
        Native::Approve {
            plan_id: p["id"].as_str().unwrap().into(),
            digest: p["digest"].as_str().unwrap().into(),
            through,
            acknowledge: true,
            acknowledged_risks: p["preview"]["risks"]
                .as_array()
                .map(|rs| {
                    rs.iter()
                        .filter_map(|r| r["code"].as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default(),
        },
    )
    .unwrap();
}
fn wait(b: &Shared, id: &str) -> Plan {
    let start = std::time::Instant::now();
    loop {
        let p = b
            .lock()
            .unwrap()
            .journal
            .plans
            .iter()
            .find(|p| p.id == id)
            .unwrap()
            .clone();
        if !["approved", "running"].contains(&p.status.as_str()) {
            return p;
        }
        assert!(start.elapsed().as_secs() < 10, "Executor timed out");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}
#[test]
fn native_approval_is_required_and_receipt_contains_commands_identity_and_times() {
    let (_dir, b, c, repo) = fixture();
    staged(&repo);
    let before = git(&repo, &["rev-parse", "HEAD"]);
    let p = proposed(&b, &c, "commit", false);
    assert_eq!(p["status"], "ready");
    assert!(p["preview"].is_object(), "{}", p["issue"]);
    review(&b, &c, &p);
    assert!(b
        .lock()
        .unwrap()
        .tool(&c, "branchline_approve", json!({"planId":p["id"]}))
        .is_err());
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), before);
    approve(&b, &p, 1);
    let done = wait(&b, p["id"].as_str().unwrap());
    assert_eq!(done.status, "succeeded", "{:?}", done.issue);
    let result = mcp::forward(
        &c,
        "branchline_wait_for_result",
        json!({"planId":p["id"],"timeoutMs":0}),
    )
    .unwrap();
    assert_eq!(result["waitOutcome"], "finished");
    assert_eq!(result["succeeded"], true);
    assert_eq!(result["plan"]["execution"][0]["status"], "succeeded");
    assert_ne!(git(&repo, &["rev-parse", "HEAD"]), before);
    assert_eq!(done.client_name, "Paired test host");
    assert_eq!(
        done.reported_ai.as_deref(),
        Some("Self-reported test model")
    );
    let command = &done.execution[0].commands[0];
    assert!(command.argv.iter().any(|a| a == "commit"));
    assert!(command.started_at >= done.approval.as_ref().unwrap().at);
    assert!(command.finished_at.unwrap() >= command.started_at);
    assert_eq!(command.outcome.as_deref(), Some("succeeded"));
    assert!(command.stdin_digest.is_some());
    let persisted: Journal =
        serde_json::from_slice(&fs::read(b.lock().unwrap().dir.join("journal-v1.json")).unwrap())
            .unwrap();
    assert_eq!(persisted.plans[0].events.last().unwrap().kind, "finished");
    let cancelled = b
        .lock()
        .unwrap()
        .tool(&c, "branchline_cancel_plan", json!({"planId":p["id"]}))
        .unwrap();
    assert_eq!(cancelled["status"], "succeeded");
}
#[test]
fn stale_diff_idempotency_content_scope_and_revocation_are_enforced() {
    let (_dir, b, c, repo) = fixture();
    staged(&repo);
    let p = proposed(&b, &c, "stale", false);
    let same = proposed(&b, &c, "stale", false);
    assert_eq!(p["id"], same["id"]);
    let mut bad: Credential = serde_json::from_value(serde_json::to_value(&c).unwrap()).unwrap();
    bad.token = "wrong".into();
    assert!(b
        .lock()
        .unwrap()
        .tool(&bad, "branchline_inspect", json!({"query":"status"}))
        .is_err());
    b.lock().unwrap().journal.clients[0].read_source = false;
    let read = b
        .lock()
        .unwrap()
        .tool(&c, "branchline_get_plan", json!({"planId":p["id"]}))
        .unwrap();
    assert!(read["preview"].get("patch").is_none());
    assert!(b
        .lock()
        .unwrap()
        .tool(&c, "branchline_inspect", json!({"query":"staged_diff"}))
        .is_err());
    review(&b, &c, &p);
    fs::write(repo.join("sample.txt"), "changed again\n").unwrap();
    git(&repo, &["add", "sample.txt"]);
    let before = git(&repo, &["rev-parse", "HEAD"]);
    assert!(Broker::native(
        &b,
        Native::Approve {
            plan_id: p["id"].as_str().unwrap().into(),
            digest: p["digest"].as_str().unwrap().into(),
            through: 1,
            acknowledge: true,
            acknowledged_risks: Vec::new()
        }
    )
    .is_err());
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), before);
    Broker::native(
        &b,
        Native::Revoke {
            client_id: c.client_id.clone(),
        },
    )
    .unwrap();
    assert!(b
        .lock()
        .unwrap()
        .tool(&c, "branchline_capabilities", json!({}))
        .is_err());
}
#[test]
fn commit_push_rejects_partial_approval_and_respects_exact_publication_scope() {
    let (dir, b, c, repo) = fixture();
    let remote = dir.path().join("remote.git");
    fs::create_dir(&remote).unwrap();
    git(&remote, &["init", "--bare"]);
    git(
        &repo,
        &["remote", "add", "origin", remote.to_str().unwrap()],
    );
    git(&repo, &["push", "-u", "origin", "main"]);
    let before = git(&remote, &["rev-parse", "refs/heads/main"]);
    staged(&repo);
    let p = proposed(&b, &c, "prefix", true);
    assert!(p["preview"].is_object(), "{}", p["issue"]);
    review(&b, &c, &p);
    let denied = Broker::native(
        &b,
        Native::Approve {
            plan_id: p["id"].as_str().unwrap().into(),
            digest: p["digest"].as_str().unwrap().into(),
            through: 1,
            acknowledge: false,
            acknowledged_risks: Vec::new(),
        },
    )
    .unwrap_err();
    assert!(denied.contains("approval") || denied.contains("request"));
    assert_eq!(b.lock().unwrap().journal.plans[0].status, "pending_review");
    assert!(b.lock().unwrap().journal.plans[0].execution.iter().all(|step| step.commands.is_empty()));
    assert_eq!(git(&remote, &["rev-parse", "refs/heads/main"]), before);
    fs::write(repo.join("sample.txt"), "next change\n").unwrap();
    git(&repo, &["add", "sample.txt"]);
    let p = proposed(&b, &c, "full", true);
    review(&b, &c, &p);
    approve(&b, &p, 2);
    let full = wait(&b, p["id"].as_str().unwrap());
    assert_eq!(full.status, "succeeded", "{:?}", full.issue);
    assert_eq!(
        git(&remote, &["rev-parse", "refs/heads/main"]),
        git(&repo, &["rev-parse", "HEAD"])
    );
    assert_eq!(full.execution[1].commands.len(), 3);
}
#[test]
fn hard_to_explain_actions_and_active_hooks_are_visible_but_not_approvable() {
    let (_dir, b, c, repo) = fixture();
    staged(&repo);
    let hook = repo.join(".git/hooks/pre-commit");
    fs::write(&hook, "#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o700)).unwrap();
    let p = proposed(&b, &c, "hook", false);
    assert_eq!(p["status"], "blocked");
    assert!(p["issue"].as_str().unwrap().contains("hooks"));
    review(&b, &c, &p);
    assert_eq!(b.lock().unwrap().journal.plans[0].status, "blocked");
    assert!(Broker::native(
        &b,
        Native::Approve {
            plan_id: p["id"].as_str().unwrap().into(),
            digest: p["digest"].as_str().unwrap().into(),
            through: 1,
            acknowledge: true,
            acknowledged_risks: Vec::new()
        }
    )
    .is_err());
}
#[test]
fn paired_repository_replacement_and_cross_client_plan_access_are_rejected() {
    let (dir, b, c, repo) = fixture();
    staged(&repo);
    let p = proposed(&b, &c, "owned", false);
    let other = Broker::native(
        &b,
        Native::Register {
            name: "Other".into(),
            root: repo.to_string_lossy().into_owned(),
            content: false,
            provider: None,
        },
    )
    .unwrap();
    let other: Credential =
        serde_json::from_slice(&fs::read(other["credentialFile"].as_str().unwrap()).unwrap())
            .unwrap();
    assert!(b
        .lock()
        .unwrap()
        .tool(&other, "branchline_get_plan", json!({"planId":p["id"]}))
        .is_err());
    fs::rename(repo.join(".git"), dir.path().join("old-git")).unwrap();
    git(&repo, &["init", "-b", "main"]);
    assert!(b
        .lock()
        .unwrap()
        .tool(
            &c,
            "branchline_inspect",
            json!({"repositoryPath":repo,"query":"status"})
        )
        .is_ok());
    review(&b, &c, &p);
    assert!(Broker::native(
        &b,
        Native::Approve {
            plan_id: p["id"].as_str().unwrap().into(),
            digest: p["digest"].as_str().unwrap().into(),
            through: 1,
            acknowledge: true,
            acknowledged_risks: vec![]
        }
    )
    .is_err());
}

#[test]
fn remote_changes_stop_publication_and_restart_never_restores_execution_authority() {
    let (dir, b, c, repo) = fixture();
    let remote = dir.path().join("remote.git");
    fs::create_dir(&remote).unwrap();
    git(&remote, &["init", "--bare"]);
    git(
        &repo,
        &["remote", "add", "origin", remote.to_str().unwrap()],
    );
    git(&repo, &["push", "-u", "origin", "main"]);
    staged(&repo);
    let p = proposed(&b, &c, "remote-race", true);
    review(&b, &c, &p);
    let other = dir.path().join("other");
    git(
        dir.path(),
        &[
            "clone",
            "-b",
            "main",
            remote.to_str().unwrap(),
            other.to_str().unwrap(),
        ],
    );
    git(&other, &["config", "user.name", "Other"]);
    git(&other, &["config", "user.email", "other@example.invalid"]);
    git(&other, &["config", "commit.gpgsign", "false"]);
    fs::write(other.join("other.txt"), "remote update").unwrap();
    git(&other, &["add", "."]);
    git(&other, &["commit", "-m", "Other change"]);
    git(&other, &["push"]);
    let tip = git(&remote, &["rev-parse", "refs/heads/main"]);
    approve(&b, &p, 2);
    let stopped = wait(&b, p["id"].as_str().unwrap());
    assert_eq!(stopped.status, "partially_completed");
    assert_eq!(stopped.execution[0].status, "succeeded");
    assert_eq!(stopped.execution[1].commands.len(), 1);
    assert!(stopped.issue.unwrap().contains("remote branch changed"));
    assert_eq!(git(&remote, &["rev-parse", "refs/heads/main"]), tip);
    let mut journal: Journal =
        serde_json::from_slice(&fs::read(b.lock().unwrap().dir.join("journal-v1.json")).unwrap())
            .unwrap();
    journal.plans[0].status = "running".into();
    journal.plans[0].execution[1].status = "running".into();
    recover(&mut journal);
    assert_eq!(journal.plans[0].status, "needs_review");
    assert!(journal.plans[0].approval.is_none());
    assert_eq!(journal.plans[0].execution[1].status, "outcome_unknown");
}

#[test]
fn one_global_connection_handles_two_repositories_without_reissuing_credentials() {
    let (dir, b, c, repo) = fixture();
    let other = dir.path().join("second");
    fs::create_dir(&other).unwrap();
    git(&other, &["init", "-b", "other"]);
    git(&other, &["config", "user.name", "Test"]);
    git(&other, &["config", "user.email", "test@example.invalid"]);
    git(&other, &["config", "commit.gpgsign", "false"]);
    fs::write(other.join("file.txt"), "initial").unwrap();
    git(&other, &["add", "."]);
    git(&other, &["commit", "-m", "Initial"]);
    let config = Broker::native(
        &b,
        Native::Configuration {
            client_id: c.client_id.clone(),
        },
    )
    .unwrap();
    Broker::native(
        &b,
        Native::GrantRepository {
            client_id: c.client_id.clone(),
            root: other.to_string_lossy().into_owned(),
            content: false,
        },
    )
    .unwrap();
    assert_eq!(
        Broker::native(
            &b,
            Native::Configuration {
                client_id: c.client_id.clone()
            }
        )
        .unwrap(),
        config
    );
    let listing = b
        .lock()
        .unwrap()
        .tool(&c, "branchline_repositories", json!({}))
        .unwrap();
    assert_eq!(listing["repositories"].as_array().unwrap().len(), 2);
    assert!(b
        .lock()
        .unwrap()
        .tool(&c, "branchline_inspect", json!({"query":"status"}))
        .is_err());
    let first = listing["repositories"][0]["repositoryId"].clone();
    let second = listing["repositories"][1]["repositoryId"].clone();
    let status = b
        .lock()
        .unwrap()
        .tool(
            &c,
            "branchline_inspect",
            json!({"repositoryId":second,"query":"status"}),
        )
        .unwrap();
    assert_eq!(status["branch"], "other");
    Broker::native(
        &b,
        Native::ReadSource {
            client_id: c.client_id.clone(),
            enabled: false,
        },
    )
    .unwrap();
    assert!(b
        .lock()
        .unwrap()
        .tool(
            &c,
            "branchline_inspect",
            json!({"repositoryId":second,"query":"staged_diff"})
        )
        .is_err());
    Broker::native(
        &b,
        Native::RemoveRepository {
            client_id: c.client_id.clone(),
            repository_id: second.as_str().unwrap().into(),
        },
    )
    .unwrap();
    assert!(b
        .lock()
        .unwrap()
        .tool(
            &c,
            "branchline_inspect",
            json!({"repositoryId":second,"query":"status"})
        )
        .is_err());
    let status = b
        .lock()
        .unwrap()
        .tool(
            &c,
            "branchline_inspect",
            json!({"repositoryId":first,"query":"status"}),
        )
        .unwrap();
    assert_eq!(status["root"].as_str(), Some(repo.to_str().unwrap()));
}
#[test]
fn old_repository_connections_migrate_to_global_scopes_and_prompt_only_once() {
    let (_dir, b, c, repo) = fixture();
    let original = b.lock().unwrap().journal.clients[0].clone();
    let mut legacy:Journal=serde_json::from_value(json!({"schemaVersion":1,"enabled":true,"clients":[{"id":original.id,"name":original.name,"root":repo,"identity":original.repositories[0].identity,"tokenHash":original.token_hash,"content":true,"active":true,"createdAt":1}],"plans":[]})).unwrap();
    migrate(&mut legacy).unwrap();
    assert_eq!(legacy.schema_version, 3);
    assert_eq!(legacy.clients[0].id, c.client_id);
    assert_eq!(legacy.clients[0].repositories.len(), 1);
    assert!(legacy.clients[0].root.is_empty());
    let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let hits = count.clone();
    b.lock().unwrap().set_notifier(Arc::new(move |_| {
        hits.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }));
    staged(&repo);
    let p = proposed(&b, &c, "prompt", false);
    review(&b, &c, &p);
    review(&b, &c, &p);
    assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_eq!(
        b.lock().unwrap().native_status(None)["focusPlanId"],
        p["id"]
    );
    Broker::native(&b, Native::AutoOpen { enabled: false }).unwrap();
    Broker::native(&b, Native::Notifications { enabled: false }).unwrap();
    let p = proposed(&b, &c, "quiet", false);
    review(&b, &c, &p);
    assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 1);
    let policy: Value = serde_json::from_slice(
        &fs::read(b.lock().unwrap().dir.join("launch-policy.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(policy["autoOpen"], false);
}

#[test]
fn primary_targets_and_branch_mismatches_require_specific_native_acknowledgement() {
    let (dir, b, c, repo) = fixture();
    let remote = dir.path().join("remote.git");
    fs::create_dir(&remote).unwrap();
    git(&remote, &["init", "--bare"]);
    git(
        &repo,
        &["remote", "add", "origin", remote.to_str().unwrap()],
    );
    git(&repo, &["push", "-u", "origin", "main"]);
    git(&repo, &["switch", "-c", "branch-a"]);
    staged(&repo);
    let p = proposed(&b, &c, "risk", true);
    let risks = p["preview"]["risks"].as_array().unwrap();
    assert_eq!(risks.len(), 2);
    assert!(risks.iter().any(|r| r["code"] == "primary_branch_target"));
    assert!(risks
        .iter()
        .any(|r| r["code"] == "branch_mismatch"
            && r["detail"].as_str().unwrap().contains("branch-a")));
    review(&b, &c, &p);
    let before = git(&repo, &["rev-parse", "HEAD"]);
    assert!(Broker::native(
        &b,
        Native::Approve {
            plan_id: p["id"].as_str().unwrap().into(),
            digest: p["digest"].as_str().unwrap().into(),
            through: 2,
            acknowledge: true,
            acknowledged_risks: Vec::new()
        }
    )
    .is_err());
    assert!(Broker::native(
        &b,
        Native::Approve {
            plan_id: p["id"].as_str().unwrap().into(),
            digest: p["digest"].as_str().unwrap().into(),
            through: 2,
            acknowledge: true,
            acknowledged_risks: vec!["primary_branch_target".into()]
        }
    )
    .is_err());
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), before);
    git(&repo, &["push", "origin", "HEAD:refs/heads/branch-a"]);
    let preview = git::ai::preview(
        &repo,
        &[Step::Push {
            remote: "origin".into(),
            branch: "branch-a".into(),
        }],
    )
    .unwrap();
    assert!(preview.risks.is_empty());
    git(&repo, &["push", "origin", "HEAD:refs/heads/master"]);
    let preview = git::ai::preview(
        &repo,
        &[Step::Push {
            remote: "origin".into(),
            branch: "master".into(),
        }],
    )
    .unwrap();
    assert!(preview
        .risks
        .iter()
        .any(|r| r.code == "primary_branch_target" && r.title.contains("master")));
}

#[test]
fn a_global_client_can_address_an_unlisted_repository_and_keeps_approval_pinned() {
    let (dir, b, c, _) = fixture();
    let repo = dir.path().join("not-enrolled");
    fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "-b", "topic"]);
    git(&repo, &["config", "user.name", "Test"]);
    git(&repo, &["config", "user.email", "test@example.invalid"]);
    git(&repo, &["config", "commit.gpgsign", "false"]);
    fs::write(repo.join("file.txt"), "first").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-m", "Initial"]);
    fs::write(repo.join("file.txt"), "reviewed").unwrap();
    git(&repo, &["add", "."]);
    let status = b
        .lock()
        .unwrap()
        .tool(
            &c,
            "branchline_inspect",
            json!({"repositoryPath":repo,"query":"status"}),
        )
        .unwrap();
    assert_eq!(status["branch"], "topic");
    let p=b.lock().unwrap().tool(&c,"branchline_propose_plan",json!({"schemaVersion":1,"repositoryPath":repo,"idempotencyKey":"any-repository","explanation":{"summary":"Save reviewed work","reason":"Keep this change in history.","impact":"Creates one local commit.","uncertainties":[]},"steps":[{"kind":"commit.create","message":"Save reviewed work"}]})).unwrap();
    assert_eq!(p["status"], "ready");
    assert_eq!(
        p["repository"].as_str(),
        Some(fs::canonicalize(&repo).unwrap().to_str().unwrap())
    );
    let before = git(&repo, &["rev-parse", "HEAD"]);
    review(&b, &c, &p);
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), before);
    approve(&b, &p, 1);
    assert_eq!(wait(&b, p["id"].as_str().unwrap()).status, "succeeded");
    let config = Broker::native(
        &b,
        Native::Configuration {
            client_id: c.client_id.clone(),
        },
    )
    .unwrap();
    Broker::native(
        &b,
        Native::ClientType {
            client_id: c.client_id.clone(),
            provider: "codex".into(),
        },
    )
    .unwrap();
    assert_eq!(b.lock().unwrap().journal.clients[0].provider, "codex");
    assert_eq!(
        Broker::native(
            &b,
            Native::Configuration {
                client_id: c.client_id
            }
        )
        .unwrap(),
        config
    );
}

#[test]
fn new_remote_branch_is_created_only_after_native_approval() {
    let (dir, b, c, repo) = fixture();
    let remote = dir.path().join("remote.git");
    fs::create_dir(&remote).unwrap();
    git(&remote, &["init", "--bare"]);
    git(
        &repo,
        &["remote", "add", "origin", remote.to_str().unwrap()],
    );
    git(&repo, &["push", "origin", "main"]);
    git(&repo, &["switch", "-c", "feat/test"]);
    staged(&repo);
    let p=b.lock().unwrap().tool(&c,"branchline_propose_plan",json!({"schemaVersion":1,"idempotencyKey":"new-branch","explanation":{"summary":"Publish feature","reason":"Share work","impact":"Create origin/feat/test","uncertainties":[]},"steps":[{"kind":"commit.create","message":"Feature"},{"kind":"remote.push","remote":"origin","branch":"feat/test"}]})).unwrap();
    assert_eq!(p["status"], "ready", "{}", p["issue"]);
    assert_eq!(p["preview"]["push"]["expectedOid"], "");
    assert_eq!(git(&remote, &["for-each-ref", "refs/heads/feat/test"]), "");
    review(&b, &c, &p);
    approve(&b, &p, 2);
    let done = wait(&b, p["id"].as_str().unwrap());
    assert_eq!(done.status, "succeeded");
    assert_eq!(
        git(&repo, &["rev-parse", "HEAD"]),
        git(&remote, &["rev-parse", "refs/heads/feat/test"])
    );
    assert_eq!(
        done.execution[1].result.as_ref().unwrap()["createdBranch"],
        true
    );
}
#[test]
fn new_branch_that_appears_after_preview_is_never_overwritten() {
    let (dir, b, c, repo) = fixture();
    let remote = dir.path().join("remote.git");
    fs::create_dir(&remote).unwrap();
    git(&remote, &["init", "--bare"]);
    git(
        &repo,
        &["remote", "add", "origin", remote.to_str().unwrap()],
    );
    git(&repo, &["push", "origin", "main"]);
    git(&repo, &["switch", "-c", "feat/race"]);
    staged(&repo);
    let p=b.lock().unwrap().tool(&c,"branchline_propose_plan",json!({"schemaVersion":1,"idempotencyKey":"race","explanation":{"summary":"Publish feature","reason":"Share work","impact":"Create branch","uncertainties":[]},"steps":[{"kind":"commit.create","message":"Feature"},{"kind":"remote.push","remote":"origin","branch":"feat/race"}]})).unwrap();
    let original = git(&repo, &["rev-parse", "HEAD"]);
    git(&remote, &["update-ref", "refs/heads/feat/race", &original]);
    review(&b, &c, &p);
    approve(&b, &p, 2);
    let done = wait(&b, p["id"].as_str().unwrap());
    assert_eq!(done.status, "partially_completed");
    assert_eq!(done.execution[0].status, "succeeded");
    assert_ne!(git(&repo, &["rev-parse", "HEAD"]), original);
    assert_eq!(git(&remote, &["rev-parse", "feat/race"]), original);
    assert!(done.execution[1]
        .commands
        .iter()
        .all(|c| !c.argv.iter().any(|a| a == "push")));
}
#[test]
fn pull_request_must_follow_the_exact_push_destination() {
    let (_dir, _b, _c, repo) = fixture();
    staged(&repo);
    let pr = Step::PullRequest {
        remote: "origin".into(),
        head: "feat/test".into(),
        base: "main".into(),
        title: "Feature".into(),
        body: "Description".into(),
        draft: true,
    };
    assert!(git::ai::preview(&repo, &[pr.clone()])
        .unwrap_err()
        .contains("after its push"));
    let err = git::ai::preview(
        &repo,
        &[
            Step::Push {
                remote: "origin".into(),
                branch: "other".into(),
            },
            pr,
        ],
    )
    .unwrap_err();
    assert!(err.contains("exact branch and remote"));
}

#[test]
fn command_catalog_distinguishes_executable_scopes_and_manual_operations() {
    let (_dir, b, c, _repo) = fixture();
    let mut b = b.lock().unwrap();
    let catalog = b
        .tool(
            &c,
            "branchline_command_catalog",
            json!({"program":"gh","command":"pr"}),
        )
        .unwrap();
    let rows = catalog["commands"].as_array().unwrap();
    assert!(rows.len() > 10);
    assert!(rows.iter().all(|r| r["program"] == "gh"));
    assert!(rows
        .iter()
        .any(|r| r["command"] == "pr create" && r["operation"] == "pr.create"));
    assert!(rows
        .iter()
        .any(|r| r["command"] == "pr merge" && r["support"] == "manual"));
    assert_eq!(
        b.tool(
            &c,
            "branchline_command_catalog",
            json!({"program":"git","command":"nonexistent"})
        )
        .unwrap()["commands"],
        json!([])
    );
    assert!(b
        .tool(&c, "branchline_command_catalog", json!({"program":"sh"}))
        .is_err());
    let old:Command=serde_json::from_value(json!({"argv":["commit"],"startedAt":1,"finishedAt":2,"outcome":"succeeded","stdinDigest":null})).unwrap();
    assert_eq!(old.program, "git");
}

#[test]
fn wait_timeout_never_approves_executes_or_raises_the_app() {
    let (_dir, b, c, repo) = fixture();
    staged(&repo);
    let p = proposed(&b, &c, "waiting", false);
    review(&b, &c, &p);
    let original = git(&repo, &["rev-parse", "HEAD"]);
    let id = p["id"].as_str().unwrap();
    let events = b.lock().unwrap().journal.plans[0].events.len();
    let raises = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed = raises.clone();
    b.lock()
        .unwrap()
        .set_notifier(std::sync::Arc::new(move |_| {
            observed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }));
    let result = mcp::forward(
        &c,
        "branchline_wait_for_result",
        json!({"planId":id,"timeoutMs":40}),
    )
    .unwrap();
    assert_eq!(result["waitOutcome"], "timed_out");
    assert_eq!(result["status"], "pending_review");
    assert_eq!(result["grantsApproval"], false);
    assert_eq!(result["succeeded"], false);
    assert_eq!(result["pullRequestUrl"], Value::Null);
    assert!(result["plan"]["approval"].is_null());
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), original);
    assert_eq!(raises.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert_eq!(b.lock().unwrap().journal.plans[0].events.len(), events);
    assert!(b.lock().unwrap().journal.plans[0].execution[0]
        .commands
        .is_empty());
    assert!(mcp::forward(
        &c,
        "branchline_wait_for_result",
        json!({"planId":id,"timeoutMs":25001})
    )
    .is_err());
    assert!(mcp::forward(
        &c,
        "branchline_wait_for_result",
        json!({"planId":id,"timeoutMs":-1})
    )
    .is_err());
    assert!(mcp::forward(
        &c,
        "branchline_wait_for_result",
        json!({"planId":id,"timeoutMs":1.5})
    )
    .is_err());
    assert!(mcp::forward(
        &c,
        "branchline_wait_for_result",
        json!({"planId":id,"approve":true})
    )
    .is_err());
}

#[test]
fn waiting_does_not_block_native_decline_and_returns_its_actual_result() {
    let (_dir, b, c, repo) = fixture();
    staged(&repo);
    let p = proposed(&b, &c, "declined-wait", false);
    review(&b, &c, &p);
    let id = p["id"].as_str().unwrap().to_string();
    let args = json!({"planId":id,"timeoutMs":2000});
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        sender
            .send(mcp::forward(&c, "branchline_wait_for_result", args))
            .unwrap();
    });
    assert!(matches!(
        receiver.recv_timeout(std::time::Duration::from_millis(20)),
        Err(std::sync::mpsc::RecvTimeoutError::Timeout)
    ));
    Broker::native(&b, Native::Decline { plan_id: id, reason: None }).unwrap();
    let result = receiver
        .recv_timeout(std::time::Duration::from_secs(3))
        .unwrap()
        .unwrap();
    assert_eq!(result["waitOutcome"], "finished");
    assert_eq!(result["status"], "cancelled");
    assert_eq!(result["succeeded"], false);
    assert_eq!(result["timedOut"], false);
}

#[test]
fn wait_enforces_ownership_redaction_revocation_and_attention_states() {
    let (_dir, b, c, repo) = fixture();
    let blocked = proposed(&b, &c, "blocked-wait", false);
    let r = mcp::forward(
        &c,
        "branchline_wait_for_result",
        json!({"planId":blocked["id"],"timeoutMs":25000}),
    )
    .unwrap();
    assert_eq!(r["waitOutcome"], "needs_attention");
    assert!(r["plan"]["issue"].is_string());
    staged(&repo);
    let p = proposed(&b, &c, "private-wait", false);
    review(&b, &c, &p);
    let registered = Broker::native(
        &b,
        Native::Register {
            name: "Other waiter".into(),
            root: repo.to_string_lossy().into(),
            content: false,
            provider: None,
        },
    )
    .unwrap();
    let other: Credential =
        serde_json::from_slice(&fs::read(registered["credentialFile"].as_str().unwrap()).unwrap())
            .unwrap();
    assert!(mcp::forward(
        &other,
        "branchline_wait_for_result",
        json!({"planId":p["id"],"timeoutMs":0})
    )
    .is_err());
    b.lock()
        .unwrap()
        .journal
        .clients
        .iter_mut()
        .find(|client| client.id == c.client_id)
        .unwrap()
        .read_source = false;
    let r = mcp::forward(
        &c,
        "branchline_wait_for_result",
        json!({"planId":p["id"],"timeoutMs":0}),
    )
    .unwrap();
    assert!(r["plan"]["preview"].get("patch").is_none());
    Broker::native(
        &b,
        Native::Revoke {
            client_id: c.client_id.clone(),
        },
    )
    .unwrap();
    assert!(mcp::forward(
        &c,
        "branchline_wait_for_result",
        json!({"planId":p["id"],"timeoutMs":0})
    )
    .is_err());
    let names = mcp::tools();
    let wait = names["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| tool["name"] == "branchline_wait_for_result")
        .unwrap();
    assert_eq!(wait["annotations"]["readOnlyHint"], true);
    assert_eq!(
        wait["inputSchema"]["properties"]["timeoutMs"]["maximum"],
        25000
    );
}

#[test]
fn waiting_observes_mid_wait_revocation_and_busy_timeout_is_not_success() {
    let (_dir, b, c, repo) = fixture();
    staged(&repo);
    let p = proposed(&b, &c, "revoke-while-waiting", false);
    review(&b, &c, &p);
    let waiter: Credential = serde_json::from_value(serde_json::to_value(&c).unwrap()).unwrap();
    let shared = b.clone();
    let args = json!({"planId":p["id"],"timeoutMs":2000});
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        sender.send(wait::run(&shared, &waiter, args)).unwrap();
    });
    assert!(matches!(
        receiver.recv_timeout(std::time::Duration::from_millis(20)),
        Err(std::sync::mpsc::RecvTimeoutError::Timeout)
    ));
    Broker::native(
        &b,
        Native::Revoke {
            client_id: c.client_id.clone(),
        },
    )
    .unwrap();
    assert!(receiver
        .recv_timeout(std::time::Duration::from_secs(3))
        .unwrap()
        .is_err());
    let _locked = b.lock().unwrap();
    let start = std::time::Instant::now();
    assert!(wait::run(&b, &c, json!({"planId":p["id"],"timeoutMs":20}))
        .unwrap_err()
        .contains("No approval"));
    assert!(start.elapsed() < std::time::Duration::from_secs(1));
}
