use super::*;
use std::time::{Duration, Instant};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Arguments {
    plan_id: String,
    #[serde(default = "default_timeout")]
    timeout_ms: u64,
}
fn default_timeout() -> u64 {
    20_000
}

fn response(plan: Value, elapsed: Duration, timed_out: bool) -> Value {
    let status = plan["status"].as_str().unwrap_or("unknown");
    let terminal = [
        "succeeded",
        "failed",
        "cancelled",
        "partially_completed",
        "outcome_unknown",
    ]
    .contains(&status);
    let active = ["pending_review", "approved", "running"].contains(&status);
    let outcome = if terminal {
        "finished"
    } else if active {
        if timed_out {
            "timed_out"
        } else {
            "waiting"
        }
    } else {
        "needs_attention"
    };
    let succeeded = status == "succeeded"
        && plan["execution"].as_array().is_some_and(|steps| {
            !steps.is_empty()
                && steps.len() == plan["requestedSteps"].as_array().map_or(0, Vec::len)
                && steps.iter().all(|step| step["status"] == "succeeded")
        });
    let command_workflow = plan["requestedSteps"]
        .as_array()
        .is_some_and(|steps| steps.iter().any(|step| step["kind"] == "command.execute"));
    let effects_verified = succeeded && !command_workflow;
    let url = plan["execution"]
        .as_array()
        .and_then(|steps| {
            steps
                .iter()
                .find(|step| step["kind"] == "pr.create" && step["status"] == "succeeded")
        })
        .and_then(|step| step["result"]["pullRequestUrl"].as_str())
        .map(str::to_owned);
    json!({"schemaVersion":1,"planId":plan["id"],"status":status,"waitOutcome":outcome,"timedOut":outcome=="timed_out","terminal":terminal,"succeeded":succeeded,"effectsVerified":effects_verified,"grantsApproval":false,"waitedMs":elapsed.as_millis() as u64,"pullRequestUrl":url,"message":match outcome{"timed_out"=>"Still waiting. A timeout grants no approval and is not completion. Continue waiting on this same plan; do not resubmit or execute it elsewhere.","finished" if command_workflow=>"Report the actual process exit codes and per-step results. A succeeded command means its process exited successfully; its Git, GitHub, shell, hook, or external effects were not independently verified. Do not infer a pull request URL from arbitrary output.","finished"=>"Report the actual plan and per-step results. Only succeeded steps completed; report a pull request URL only when returned in verified results.",_=>"This plan needs attention. Report the issue; do not treat it as approved or completed."},"plan":plan})
}

pub(super) fn run(shared: &Shared, credential: &Credential, args: Value) -> Result<Value, String> {
    let args: Arguments = serde_json::from_value(args)
        .map_err(|_| "Provide planId and an optional integer timeoutMs (0–25000).")?;
    if args.plan_id.is_empty() || args.plan_id.len() > 128 || args.timeout_ms > 25_000 {
        return Err("Provide planId (1–128 characters) and timeoutMs between 0 and 25000.".into());
    }
    let started = Instant::now();
    let timeout = Duration::from_millis(args.timeout_ms);
    loop {
        let plan = match shared.try_lock() {
            Ok(mut broker) => broker.tool(
                credential,
                "branchline_get_plan",
                json!({"planId":args.plan_id}),
            )?,
            Err(std::sync::TryLockError::Poisoned(_)) => return Err("Connector unavailable".into()),
            Err(std::sync::TryLockError::WouldBlock) => {
                let elapsed = started.elapsed();
                if elapsed >= timeout {
                    return Err("Wait timed out before the latest request state could be read. No approval or completion is implied; check this same plan again.".into());
                }
                std::thread::sleep(Duration::from_millis(100).min(timeout.saturating_sub(elapsed)));
                continue;
            }
        };
        let elapsed = started.elapsed();
        let result = response(plan, elapsed, elapsed >= timeout);
        if result["waitOutcome"] != "waiting" {
            return Ok(result);
        }
        std::thread::sleep(Duration::from_millis(100).min(timeout.saturating_sub(elapsed)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn command_success_only_confirms_process_exit_and_never_guesses_a_pr_url() {
        let result = response(json!({"id":"commands","status":"succeeded","requestedSteps":[{"kind":"command.execute"}],"execution":[{"kind":"command.execute","status":"succeeded","result":{"processSucceeded":true,"effectsVerified":false,"stdout":"https://github.com/owner/repo/pull/99"}}]}), Duration::ZERO, false);
        assert_eq!(result["succeeded"], true);
        assert_eq!(result["effectsVerified"], false);
        assert_eq!(result["pullRequestUrl"], Value::Null);
        assert!(result["message"].as_str().unwrap().contains("process exited successfully"));
    }
    #[test]
    fn verified_pr_url_is_reported_without_turning_partial_success_into_full_success() {
        let plan = json!({"id":"p","status":"partially_completed","requestedSteps":[{},{}],"execution":[{"kind":"pr.create","status":"succeeded","result":{"pullRequestUrl":"https://github.com/owner/repo/pull/42"}}]});
        let result = response(plan, Duration::ZERO, false);
        assert_eq!(
            result["pullRequestUrl"],
            "https://github.com/owner/repo/pull/42"
        );
        assert_eq!(result["succeeded"], false);
        assert_eq!(result["terminal"], true);
        let result = response(
            json!({"id":"p","status":"running","execution":[{"kind":"pr.create","status":"outcome_unknown","result":{"pullRequestUrl":"https://github.com/owner/repo/pull/99"}}]}),
            Duration::ZERO,
            true,
        );
        assert_eq!(result["pullRequestUrl"], Value::Null);
        assert_eq!(result["timedOut"], true);
        assert_eq!(result["grantsApproval"], false);
        let result = response(
            json!({"id":"p","status":"succeeded","requestedSteps":[{}],"execution":[{"kind":"pr.create","status":"succeeded","result":{"pullRequestUrl":"https://github.com/owner/repo/pull/42"}}]}),
            Duration::ZERO,
            false,
        );
        assert_eq!(result["succeeded"], true);
        assert_eq!(
            result["pullRequestUrl"],
            "https://github.com/owner/repo/pull/42"
        );
        assert_eq!(result["waitOutcome"], "finished");
    }
}
