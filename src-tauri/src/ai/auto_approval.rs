mod classifier;
use super::*;
use std::{collections::HashSet, time::{Duration, Instant}};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Operation { Stage, Commit, Push, PrCreate, PrEdit }

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GrantRequest {
    pub client_id: String,
    pub repository: String,
    #[serde(default)]
    pub all_repositories: bool,
    pub session_wide: bool,
    pub operations: Vec<Operation>,
    pub remote: Option<String>,
    pub branch: Option<String>,
    pub github_repository: Option<String>,
    pub base: Option<String>,
    pub minutes: Option<u64>,
    pub confirmed: bool,
    pub preview_digest: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GrantRecord {
    pub id: String,
    pub revision: u32,
    pub client_id: String,
    pub repository: String,
    #[serde(default)]
    pub all_repositories: bool,
    pub session_wide: bool,
    pub operations: Vec<Operation>,
    pub remote: Option<String>,
    pub remote_url: Option<String>,
    pub branch: Option<String>,
    pub github_repository: Option<String>,
    pub base: Option<String>,
    pub granted_at: u64,
    pub expires_at: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_repository: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_push_url: Option<String>,
}

struct Grant {
    record: GrantRecord,
    identity: String,
    deadline: Option<Instant>,
    excluded: HashSet<String>,
}

#[derive(Default)]
pub(super) struct Store { grants: Vec<Grant> }

fn scope_digest(grant: &Grant) -> String {
    let r = &grant.record;
    digest(&serde_json::to_vec(&json!({"revision":r.revision,"clientId":r.client_id,"allRepositories":r.all_repositories,"repository":r.repository,"identity":grant.identity,
        "sessionWide":r.session_wide,"operations":r.operations,"remote":r.remote,"remoteUrl":r.remote_url,"branch":r.branch,
        "githubRepository":r.github_repository,"base":r.base,"duration":r.expires_at.map(|end| end-r.granted_at)})).unwrap())
}

impl Store {
    pub fn clear(&mut self) { self.grants.clear(); }
    pub fn revoke(&mut self, id: &str) { self.grants.retain(|g| g.record.id != id && g.record.client_id != id); }
    pub fn status(&self) -> Vec<GrantRecord> {
        self.grants.iter().filter(|g| g.deadline.is_none_or(|d| Instant::now() < d)).map(|g| g.record.clone()).collect()
    }
    pub fn preview(mut request: GrantRequest, plans: &[Plan]) -> Result<Value, String> {
        request.confirmed = true;
        let mut temporary = Self::default();
        temporary.add_inner(request, plans, false)?;
        let grant = &temporary.grants[0];
        Ok(json!({"scope":grant.record,"digest":scope_digest(grant)}))
    }
    pub fn add(&mut self, request: GrantRequest, plans: &[Plan]) -> Result<(), String> {
        self.add_inner(request, plans, true)
    }
    fn add_inner(&mut self, request: GrantRequest, plans: &[Plan], require_digest: bool) -> Result<(), String> {
        if !request.confirmed { return Err("Confirm the automatic approval scope in Branchline first.".into()); }
        if request.minutes.is_some_and(|m| !matches!(m, 15 | 30 | 60)) { return Err("Choose 15, 30 or 60 minutes, or until Branchline quits.".into()); }
        self.grants.retain(|g| g.deadline.is_none_or(|d| Instant::now() < d));
        if self.grants.len() >= 20 { return Err("Remove an existing rule before adding another.".into()); }
        if request.operations.is_empty() && !request.session_wide { return Err("Choose at least one operation.".into()); }
        if request.session_wide && !request.operations.is_empty() { return Err("Session-wide approval cannot include operation rules.".into()); }
        if !request.all_repositories && (!Path::new(&request.repository).is_absolute() || request.repository.len() > 4096) {
            return Err("Choose an absolute repository path.".into());
        }
        if request.operations.len() > 5 || request.operations.iter().enumerate().any(|(i, op)| request.operations[..i].contains(op)) {
            return Err("Choose each supported operation once.".into());
        }
        let repository = if request.all_repositories { String::new() } else { git::ai::locate(Path::new(&request.repository))?.to_string_lossy().into_owned() };
        let identity = if request.all_repositories { String::new() } else { git::ai::repository_identity(Path::new(&repository))? };
        let push = request.operations.contains(&Operation::Push);
        let pr = request.operations.iter().any(|op| matches!(op, Operation::PrCreate | Operation::PrEdit));
        let remote = request.remote.filter(|v| push && !v.is_empty());
        let branch = request.branch.filter(|v| push && !v.is_empty());
        let github_repository = request.github_repository.filter(|v| pr && !v.is_empty());
        let base = request.base.filter(|v| request.operations.contains(&Operation::PrCreate) && !v.is_empty());
        let remote_url = if push {
            let remote = remote.as_deref().filter(|v| classifier::name(v)).ok_or("Enter a literal remote name.")?;
            if !branch.as_deref().is_some_and(classifier::branch) { return Err("Enter the destination branch.".into()); }
            if request.all_repositories { None } else { Some(classifier::push_url(&repository, remote).ok_or("The remote must have one explicit push destination.")?) }
        } else { None };
        if pr && !github_repository.as_deref().is_some_and(classifier::github_repository) { return Err("Enter a GitHub repository as owner/name.".into()); }
        if request.operations.contains(&Operation::PrCreate) && !base.as_deref().is_some_and(classifier::branch) { return Err("Enter the pull request base branch.".into()); }
        let granted_at = now();
        let duration = request.minutes.map(|m| Duration::from_secs(m * 60));
        let grant = Grant {
            record: GrantRecord { id: random()?, revision: 1, client_id: request.client_id, repository, all_repositories: request.all_repositories, resolved_repository: None, resolved_push_url: None, session_wide: request.session_wide, operations: request.operations, remote, remote_url, branch, github_repository, base, granted_at, expires_at: duration.map(|d| granted_at + d.as_secs()) },
            identity, deadline: duration.map(|d| Instant::now() + d), excluded: plans.iter().map(|p| p.id.clone()).collect(),
        };
        if require_digest && request.preview_digest.as_deref() != Some(scope_digest(&grant).as_str()) {
            return Err("The approval scope changed or was not reviewed. Review the connection, repository and destinations again.".into());
        }
        self.grants.push(grant);
        Ok(())
    }
    pub fn matching(&self, plan: &Plan) -> Option<GrantRecord> {
        if plan.preview.is_none() || plan.steps.is_empty() || !matches!(plan.status.as_str(), "ready" | "pending_review") { return None; }
        self.grants.iter().find(|grant| {
            let r = &grant.record;
            r.client_id == plan.client_id && (r.all_repositories || r.repository == plan.repository)
                && !grant.excluded.contains(&plan.id)
                && grant.deadline.is_none_or(|d| Instant::now() < d)
                && (r.all_repositories || git::ai::repository_identity(Path::new(&r.repository)).ok().as_ref() == Some(&grant.identity))
                && (r.session_wide || classifier::matches(r, plan))
        }).map(|g| {
            let mut snapshot = g.record.clone();
            snapshot.resolved_repository = Some(plan.repository.clone());
            if g.record.operations.contains(&Operation::Push) {
                snapshot.resolved_push_url = g.record.remote.as_deref().and_then(|remote| classifier::push_url(&plan.repository, remote));
            }
            snapshot
        })
    }
    pub fn active(&self, record: &GrantRecord, plan: &Plan) -> bool {
        self.grants.iter().any(|g| g.record.id == record.id && g.record.revision == record.revision
            && g.record.client_id == plan.client_id && (g.record.all_repositories || g.record.repository == plan.repository)
            && record.resolved_repository.as_deref().is_none_or(|root| root == plan.repository)
            && !g.excluded.contains(&plan.id) && g.deadline.is_none_or(|d| Instant::now() < d)
            && (g.record.all_repositories || git::ai::repository_identity(Path::new(&plan.repository)).ok().as_ref() == Some(&g.identity))
            && (g.record.session_wide || classifier::matches(record, plan)))
    }
}

pub(super) fn dispatch(shared: &Shared, id: &str) {
    let candidate = {
        let Ok(mut b) = shared.lock() else { return; };
        if !b.auto_candidates.remove(id) { return; }
        let Some(p) = b.journal.plans.iter().find(|p| p.id == id && p.status == "pending_review") else { return; };
        b.auto_approval.matching(p).map(|grant| (p.clone(), grant))
    };
    let Some((plan, grant)) = candidate else {
        notify_manual(shared, id, None);
        return;
    };
    let request = Native::Approve { plan_id: plan.id.clone(), digest: plan.digest, through: plan.steps.len(), acknowledge: true,
        acknowledged_risks: plan.preview.as_ref().map(|p| p.risks.iter().map(|r| r.code.clone()).collect()).unwrap_or_default() };
    if let Err(reason) = Broker::native_authorized(shared, request, Some(grant)) {
        notify_manual(shared, id, Some(reason));
    }
}

fn notify_manual(shared: &Shared, id: &str, reason: Option<String>) {
    if let Ok(mut b) = shared.lock() {
        let Ok(p) = b.plan_mut(id) else { return; };
        if !matches!(p.status.as_str(), "pending_review" | "needs_review" | "blocked") { return; }
        if let Some(reason) = reason { p.issue = Some(reason); }
        b.focus_plan = b.journal.auto_open.then(|| id.to_string());
        let _ = b.save();
        if review_signals::should_dispatch(true, b.journal.auto_open, b.journal.notifications) {
            if let Some(notify) = &b.notifier { notify(id); }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn expired_grants_and_new_broker_sessions_have_no_authority() {
        let mut store = Store::default();
        assert!(store.status().is_empty());
        store.grants.push(Grant {
            record: GrantRecord { id: "expired".into(), revision: 1, client_id: "client".into(), repository: "/repo".into(), all_repositories: false, resolved_repository: None, resolved_push_url: None, session_wide: true, operations: vec![], remote: None, remote_url: None, branch: None, github_repository: None, base: None, granted_at: now(), expires_at: Some(now() + 3600) },
            identity: "identity".into(), deadline: Some(Instant::now() - Duration::from_secs(1)), excluded: HashSet::new(),
        });
        assert!(store.status().is_empty());
        assert!(Store::default().status().is_empty());
    }
}
