use super::*;
use std::os::unix::fs::PermissionsExt;

fn workflow(pr_exit: i32) {
    let (dir, broker, credential, repo) = fixture();
    let remote = dir.path().join("remote.git");
    let remote_init = std::process::Command::new("git")
        .args(["init", "--bare", "--quiet", "--initial-branch=main"])
        .arg(&remote).status().unwrap();
    assert!(remote_init.success());
    git(&repo, &["remote", "add", "origin", remote.to_str().unwrap()]);
    let bin = dir.path().join("bin");
    fs::create_dir(&bin).unwrap();
    let args_file = dir.path().join("gh-arguments");
    let gh = bin.join("gh");
    fs::write(&gh, format!("#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\nprintf '%s\\n' 'https://github.com/fixture/repository/pull/17'\nexit {pr_exit}\n", args_file.display())).unwrap();
    fs::set_permissions(&gh, fs::Permissions::from_mode(0o700)).unwrap();
    let script = format!("git switch -c feat/test && git add -- first.txt second.txt && git -c user.name='Fixture Test' -c user.email=fixture@example.invalid commit -m 'Blind workflow commit' && git push --set-upstream origin feat/test && PATH='{}':\"$PATH\" gh pr create --title 'Review the fixture' --body 'First line\nSecond line' --head feat/test --base main", bin.display());
    let mut request = input("blind-full-workflow", &[]);
    request.as_object_mut().unwrap().remove("steps");
    request["command"] = json!(script);
    request["repositoryPath"] = json!(repo);
    let plan = mcp::forward(&credential, "branchline_propose_command", request).unwrap();
    assert_eq!(plan["status"], "ready");
    assert_eq!(plan["requestedSteps"][0]["argv"][1], script);
    mcp::forward(&credential, "branchline_request_review", json!({"planId":plan["id"]})).unwrap();
    assert!(!repo.join(".git/index").exists());
    assert!(!args_file.exists());
    let pending = super::super::wait::run(&broker, &credential, json!({"planId":plan["id"],"timeoutMs":0})).unwrap();
    assert_eq!(pending["grantsApproval"], false);
    assert_eq!(pending["waitOutcome"], "timed_out");
    Broker::native(&broker, approval(&plan, 1, true, true)).unwrap();
    let done = wait(&broker, &plan);
    assert_eq!(done.status, if pr_exit == 0 {"succeeded"} else {"failed"});
    assert_eq!(done.execution.len(), 1);
    assert_eq!(done.execution[0].result.as_ref().unwrap()["exitCode"], pr_exit);
    let local_head = git(&repo, &["rev-parse", "HEAD"]);
    let remote_head = git(&remote, &["rev-parse", "refs/heads/feat/test"]);
    assert_eq!(local_head, remote_head);
    assert_eq!(git(&repo, &["log", "-1", "--format=%s"]), "Blind workflow commit");
    assert_eq!(git(&repo, &["branch", "--show-current"]), "feat/test");
    assert_eq!(fs::read_to_string(args_file).unwrap(), "pr\ncreate\n--title\nReview the fixture\n--body\nFirst line\nSecond line\n--head\nfeat/test\n--base\nmain\n");
    let result = mcp::forward(&credential, "branchline_wait_for_result", json!({"planId":plan["id"],"timeoutMs":0})).unwrap();
    assert_eq!(result["succeeded"], pr_exit == 0);
    assert_eq!(result["effectsVerified"], false);
    assert!(result["pullRequestUrl"].is_null());
    assert!(result["plan"]["execution"][0]["result"].get("stdout").is_none());
}

#[test]
fn blind_whole_command_runs_commit_push_and_offline_gh_arguments_in_order() {
    workflow(0);
}

#[test]
fn blind_gh_failure_reports_failure_without_hiding_completed_commit_and_push() {
    workflow(17);
}
