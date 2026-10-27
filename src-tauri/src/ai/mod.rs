//! The native broker is the only approval authority. MCP can never call `native`.
pub mod mcp;
mod wait;
mod review_signals;
mod output;
use crate::git::{
    self,
    ai::{Preview, Step},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::os::unix::{
    fs::{OpenOptionsExt, PermissionsExt},
    io::AsRawFd,
    net::{UnixListener, UnixStream},
};
use std::{
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};
const SCHEMA: u32 = 1;
const JOURNAL_SCHEMA: u32 = 3;
fn other() -> String {
    "other".into()
}
fn yes() -> bool {
    true
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn random() -> Result<String, String> {
    let mut b = [0u8; 32];
    File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut b))
        .map_err(|e| e.to_string())?;
    Ok(b.iter().map(|x| format!("{x:02x}")).collect())
}
fn persist(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let mut file = tempfile::NamedTempFile::new_in(path.parent().ok_or("Missing data directory")?)
        .map_err(|e| e.to_string())?;
    file.as_file()
        .set_permissions(fs::Permissions::from_mode(0o600))
        .map_err(|e| e.to_string())?;
    serde_json::to_writer(&mut file, value).map_err(|e| e.to_string())?;
    file.flush().map_err(|e| e.to_string())?;
    file.as_file().sync_all().map_err(|e| e.to_string())?;
    file.persist(path).map_err(|e| e.to_string())?;
    File::open(path.parent().unwrap())
        .and_then(|f| f.sync_all())
        .map_err(|e| e.to_string())
}
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Client {
    id: String,
    name: String,
    #[serde(default, skip_serializing)]
    root: String,
    #[serde(default, skip_serializing)]
    identity: String,
    token_hash: String,
    #[serde(default, skip_serializing)]
    content: bool,
    #[serde(default)]
    repositories: Vec<RepositoryAccess>,
    #[serde(default)]
    read_source: bool,
    #[serde(default = "other")]
    provider: String,
    active: bool,
    created_at: u64,
}
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct RepositoryAccess {
    id: String,
    root: String,
    identity: String,
    content: bool,
}
fn access(root: &str, content: bool) -> Result<RepositoryAccess, String> {
    let root = git::ai::locate(Path::new(root))?
        .to_string_lossy()
        .into_owned();
    let identity = git::ai::repository_identity(Path::new(&root))?;
    Ok(RepositoryAccess {
        id: random()?,
        root,
        identity,
        content,
    })
}
fn migrate(journal: &mut Journal) -> Result<(), String> {
    if journal.schema_version == 1 {
        for c in &mut journal.clients {
            if c.repositories.is_empty() && !c.root.is_empty() {
                c.repositories.push(RepositoryAccess {
                    id: random()?,
                    root: c.root.clone(),
                    identity: c.identity.clone(),
                    content: c.content,
                });
            }
            c.root.clear();
            c.identity.clear();
            c.content = false;
        }
        journal.schema_version = 2;
    }
    if journal.schema_version == 2 {
        for c in &mut journal.clients {
            c.read_source = false;
            for r in &mut c.repositories {
                r.content = false;
            }
        }
        journal.schema_version = 3;
    }
    if journal.schema_version != JOURNAL_SCHEMA {
        return Err("This connector journal needs a newer Branchline version.".into());
    }
    Ok(())
}
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Explanation {
    pub summary: String,
    pub reason: String,
    pub impact: String,
    #[serde(default)]
    pub uncertainties: Vec<String>,
}
fn default_program() -> String {
    "git".into()
}
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Command {
    #[serde(default = "default_program")]
    pub program: String,
    pub argv: Vec<String>,
    pub started_at: u64,
    pub finished_at: Option<u64>,
    pub outcome: Option<String>,
    pub stdin_digest: Option<String>,
}
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub sequence: usize,
    pub at: u64,
    pub kind: String,
    pub detail: String,
}
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Execution {
    pub kind: String,
    pub status: String,
    pub result: Option<Value>,
    pub error: Option<String>,
    pub commands: Vec<Command>,
}
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Approval {
    pub at: u64,
    pub expires_at: u64,
    pub through: usize,
    pub digest: String,
    pub authority: String,
    #[serde(default)]
    pub acknowledged_risks: Vec<String>,
}
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub schema_version: u32,
    pub id: String,
    pub version: u32,
    pub client_id: String,
    pub client_name: String,
    #[serde(default = "other")]
    pub client_provider: String,
    pub reported_ai: Option<String>,
    pub repository: String,
    pub idempotency_key: String,
    pub input_digest: String,
    pub explanation: Explanation,
    pub requested_steps: Vec<Value>,
    pub steps: Vec<Step>,
    pub preview: Option<Preview>,
    pub digest: String,
    pub status: String,
    pub issue: Option<String>,
    pub created_at: u64,
    pub approval: Option<Approval>,
    pub execution: Vec<Execution>,
    pub events: Vec<Event>,
    pub cancel_requested: bool,
}
impl Plan {
    fn event(&mut self, kind: &str, detail: &str) {
        self.events.push(Event {
            sequence: self.events.len() + 1,
            at: now(),
            kind: kind.into(),
            detail: detail.into(),
        });
    }
}
#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct Journal {
    schema_version: u32,
    enabled: bool,
    #[serde(default = "yes")]
    auto_open: bool,
    #[serde(default = "yes")]
    notifications: bool,
    clients: Vec<Client>,
    plans: Vec<Plan>,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Credential {
    pub socket: String,
    pub client_id: String,
    pub token: String,
}
pub struct Broker {
    dir: PathBuf,
    journal: Journal,
    _lock: File,
    pub operation_gate: Arc<Mutex<()>>,
    notifier: Option<Arc<dyn Fn(&str) + Send + Sync>>,
    focus_plan: Option<String>,
    output: Arc<Mutex<output::OutputStore>>,
}
pub type Shared = Arc<Mutex<Broker>>;
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Native {
    Status {
        plan_id: Option<String>,
    },
    ClearActivity {
        confirmed: bool,
    },
    Export {
        plan_id: String,
        path: String,
    },
    Enable {
        enabled: bool,
    },
    Register {
        name: String,
        root: String,
        content: bool,
        #[serde(default)]
        provider: Option<String>,
    },
    ClientType {
        client_id: String,
        provider: String,
    },
    ReadSource {
        client_id: String,
        enabled: bool,
    },
    Revoke {
        client_id: String,
    },
    GrantRepository {
        client_id: String,
        root: String,
        content: bool,
    },
    RemoveRepository {
        client_id: String,
        repository_id: String,
    },
    Configuration {
        client_id: String,
    },
    AutoOpen {
        enabled: bool,
    },
    Notifications {
        enabled: bool,
    },
    ReviewDiff {
        plan_id: String,
        detail_index: usize,
        path: String,
    },
    Approve {
        plan_id: String,
        digest: String,
        through: usize,
        acknowledge: bool,
        #[serde(default)]
        acknowledged_risks: Vec<String>,
    },
    Decline {
        plan_id: String,
        #[serde(default)]
        reason: Option<String>,
    },
    Cancel {
        plan_id: String,
    },
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Proposal {
    #[serde(default)]
    repository_id: Option<String>,
    #[serde(default)]
    repository_path: Option<String>,
    schema_version: u32,
    idempotency_key: String,
    explanation: Explanation,
    steps: Vec<Value>,
    #[serde(default)]
    reported_ai: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CommandProposal {
    schema_version: u32,
    repository_path: Option<String>,
    repository_id: Option<String>,
    idempotency_key: String,
    explanation: Explanation,
    reported_ai: Option<String>,
    command: String,
    stdin: Option<String>,
    timeout_seconds: Option<u64>,
}
fn recover(journal: &mut Journal) {
    for plan in &mut journal.plans {
        if ["running", "approved"].contains(&plan.status.as_str()) {
            plan.status = "needs_review".into();
            plan.issue=Some("Branchline restarted. Check the recorded commands and repository before submitting a new plan; an interrupted action may have completed.".into());
            plan.approval = None;
            for step in &mut plan.execution {
                if step.status == "running" {
                    step.status = "outcome_unknown".into()
                }
            }
            plan.event(
                "restart",
                "Unstarted actions stopped; approval was not restored.",
            );
        }
    }
}

fn cancellation_finished(status: &str) -> bool {
    [
        "succeeded",
        "failed",
        "partially_completed",
        "cancelled",
        "blocked",
        "needs_review",
        "outcome_unknown",
    ]
    .contains(&status)
}

impl Broker {
    pub fn has_active_execution(&self) -> bool {
        self.journal.plans.iter().any(|plan| matches!(plan.status.as_str(), "approved" | "running"))
    }

    pub fn start(dir: PathBuf) -> Result<Shared, String> {
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
        let lock = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .mode(0o600)
            .open(dir.join("broker.lock"))
            .map_err(|e| e.to_string())?;
        if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err("Another Branchline connector is already running.".into());
        }
        let path = dir.join("journal-v1.json");
        let mut journal: Journal = if path.exists() {
            serde_json::from_slice(&fs::read(&path).map_err(|e| e.to_string())?).map_err(|e| {
                format!("Connector journal could not be read; no actions will run: {e}")
            })?
        } else {
            Journal {
                schema_version: JOURNAL_SCHEMA,
                auto_open: true,
                notifications: true,
                ..Default::default()
            }
        };
        migrate(&mut journal)?;
        recover(&mut journal);
        let socket = dir.join("b.sock");
        if socket.exists() {
            fs::remove_file(&socket).map_err(|e| e.to_string())?
        }
        let listener = UnixListener::bind(&socket).map_err(|e| e.to_string())?;
        fs::set_permissions(&socket, fs::Permissions::from_mode(0o600))
            .map_err(|e| e.to_string())?;
        let broker = Arc::new(Mutex::new(Self {
            dir,
            journal,
            _lock: lock,
            operation_gate: Arc::new(Mutex::new(())),
            notifier: None,
            focus_plan: None,
            output: Arc::new(Mutex::new(output::OutputStore::default())),
        }));
        broker.lock().map_err(|_| "Connector unavailable")?.save()?;
        let shared = broker.clone();
        std::thread::spawn(move || {
            let connections = Arc::new(std::sync::atomic::AtomicUsize::new(0));
            for stream in listener.incoming().flatten() {
                use std::sync::atomic::Ordering;
                if connections.fetch_add(1, Ordering::SeqCst) >= 16 {
                    connections.fetch_sub(1, Ordering::SeqCst);
                    continue;
                }
                let b = shared.clone();
                let n = connections.clone();
                std::thread::spawn(move || {
                    let _ = serve(stream, b);
                    n.fetch_sub(1, Ordering::SeqCst);
                });
            }
        });
        Ok(broker)
    }
    fn save(&self) -> Result<(), String> {
        persist(&self.dir.join("journal-v1.json"), &self.journal)?;
        persist(
            &self.dir.join("launch-policy.json"),
            &json!({"enabled":self.journal.enabled,"autoOpen":self.journal.auto_open,"notifications":self.journal.notifications,"activeClientIds":self.journal.clients.iter().filter(|c|c.active).map(|c|&c.id).collect::<Vec<_>>()}),
        )
    }
    pub fn review_repository(&self, id: &str) -> Option<String> {
        self.journal
            .plans
            .iter()
            .find(|p| p.id == id)
            .map(|p| p.repository.clone())
    }
    pub fn review_alert_options(&self, id: &str) -> Option<(bool, bool)> {
        self.journal.plans.iter().find(|plan| plan.id == id
            && matches!(plan.status.as_str(), "pending_review" | "blocked")
            && self.journal.enabled
            && self.journal.clients.iter().any(|client| client.id == plan.client_id && client.active))
            .map(|_| (self.journal.auto_open, self.journal.notifications))
    }
    fn plan_mut(&mut self, id: &str) -> Result<&mut Plan, String> {
        self.journal
            .plans
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| "Request not found".into())
    }
    fn client(&self, credential: &Credential) -> Result<Client, String> {
        if !self.journal.enabled {
            return Err("AI Connector is turned off in Branchline.".into());
        }
        self.journal
            .clients
            .iter()
            .find(|c| {
                c.active
                    && c.id == credential.client_id
                    && c.token_hash == digest(credential.token.as_bytes())
            })
            .cloned()
            .ok_or_else(|| "This connection is not authorized. Pair it in Branchline.".into())
    }
    fn redacted(plan: &Plan, content: bool) -> Value {
        let mut v = serde_json::to_value(plan).unwrap();
        if !content {
            if let Some(p) = v.get_mut("preview").and_then(Value::as_object_mut) {
                p.remove("patch");
                p.remove("commandDetails");
            }
        }
        v
    }
    fn native_status(&self, selected: Option<&str>) -> Value {
        let clients=self.journal.clients.iter().map(|c|json!({"id":c.id,"name":c.name,"repositories":c.repositories,"readSource":c.read_source,"provider":c.provider,"active":c.active,"createdAt":c.created_at})).collect::<Vec<_>>();
        let command_outputs = self.output.lock().ok().and_then(|store| selected.map(|id| serde_json::to_value(store.snapshots(id)).unwrap_or(json!([])))).unwrap_or(json!([]));
        json!({"commandOutputs":command_outputs,"schemaVersion":SCHEMA,"enabled":self.journal.enabled,"autoOpen":self.journal.auto_open,"notifications":self.journal.notifications,"focusPlanId":self.focus_plan,"clients":clients,"plans":self.journal.plans.iter().rev().map(|p|{let mut v=Self::redacted(p,false); if selected!=Some(p.id.as_str()){v["preview"]=Value::Null; v["execution"]=json!([]);v["events"]=json!([]);} v}).collect::<Vec<_>>(),"selectedPlan":selected.and_then(|id|self.journal.plans.iter().find(|p|p.id==id)),"pending":self.journal.plans.iter().filter(|p|p.status=="pending_review").count()})
    }
    pub fn set_notifier(&mut self, notify: Arc<dyn Fn(&str) + Send + Sync>) {
        self.notifier = Some(notify)
    }
    fn configuration(&self, id: &str) -> Result<Value, String> {
        if !self.journal.clients.iter().any(|c| c.id == id && c.active) {
            return Err("Connection not found".into());
        }
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let file = self.dir.join(format!("client-{}.json", &id[..12]));
        Ok(
            json!({"configuration":{"mcpServers":{"branchline":{"command":exe,"args":["--mcp","--credential-file",file]}}},"clientId":id,"credentialFile":file}),
        )
    }
    pub fn native(shared: &Shared, request: Native) -> Result<Value, String> {
        let mut b = shared.lock().map_err(|_| "Connector unavailable")?;
        match request {
            Native::Status { plan_id } => return Ok(b.native_status(plan_id.as_deref())),
            Native::ClearActivity { confirmed } => {
                if !confirmed {
                    return Err("Confirm before clearing request activity.".into());
                }
                let previous = b.journal.plans.clone();
                let previous_focus = b.focus_plan.clone();
                b.journal.plans.retain(|plan| !cancellation_finished(&plan.status));
                let cleared = previous.len() - b.journal.plans.len();
                if b.focus_plan.as_ref().is_some_and(|id| !b.journal.plans.iter().any(|plan| &plan.id == id)) {
                    b.focus_plan = None;
                }
                if let Err(error) = persist(&b.dir.join("journal-v1.json"), &b.journal) {
                    b.journal.plans = previous;
                    b.focus_plan = previous_focus;
                    return Err(error);
                }
                let retained = b.journal.plans.iter().map(|plan| plan.id.clone()).collect::<Vec<_>>();
                if let Ok(mut output) = b.output.lock() { output.retain(&retained); }
                return Ok(json!({"cleared":cleared}));
            }
            Native::ReviewDiff { plan_id, detail_index, path } => {
                let plan = b.journal.plans.iter().find(|plan| plan.id == plan_id).cloned().ok_or("Request not found")?;
                drop(b);
                let preview = plan.preview.as_ref().ok_or("This request has no file preview.")?;
                let manifest = preview.command_workflow.as_ref().ok_or("This request has no command snapshot.")?;
                let detail = preview.command_details.get(detail_index).ok_or("Request detail not found")?;
                return git::ai::review_diff::inspect(Path::new(&plan.repository), detail, &path, manifest);
            },
            Native::Export { plan_id, path } => {
                let p = b
                    .journal
                    .plans
                    .iter()
                    .find(|p| p.id == plan_id)
                    .ok_or("Request not found")?;
                persist(Path::new(&path), p)?;
                return Ok(json!({"exported":true}));
            }
            Native::ClientType {
                client_id,
                provider,
            } => {
                if ![
                    "claude-code",
                    "claude-desktop",
                    "codex",
                    "chatgpt-desktop",
                    "cursor",
                    "other",
                ]
                .contains(&provider.as_str())
                {
                    return Err("Choose a supported AI client type".into());
                }
                let client = b
                    .journal
                    .clients
                    .iter_mut()
                    .find(|c| c.id == client_id && c.active)
                    .ok_or("Connection not found")?;
                client.provider = provider;
            }
            Native::ReadSource { client_id, enabled } => {
                let client = b
                    .journal
                    .clients
                    .iter_mut()
                    .find(|c| c.id == client_id && c.active)
                    .ok_or("Connection not found")?;
                client.read_source = enabled;
            }
            Native::AutoOpen { enabled } => {
                b.journal.auto_open = enabled;
            }
            Native::Notifications { enabled } => {
                b.journal.notifications = enabled;
            }
            Native::Configuration { client_id } => return b.configuration(&client_id),
            Native::GrantRepository {
                client_id,
                root,
                content,
            } => {
                let granted = access(&root, content)?;
                let c = b
                    .journal
                    .clients
                    .iter_mut()
                    .find(|c| c.id == client_id && c.active)
                    .ok_or("Connection not found")?;
                if let Some(old) = c.repositories.iter_mut().find(|r| r.root == granted.root) {
                    let id = if old.identity == granted.identity {
                        old.id.clone()
                    } else {
                        granted.id.clone()
                    };
                    *old = RepositoryAccess { id, ..granted };
                } else {
                    if c.repositories.len() >= 50 {
                        return Err("Repository access limit reached".into());
                    }
                    c.repositories.push(granted);
                }
            }
            Native::RemoveRepository {
                client_id,
                repository_id,
            } => {
                let c = b
                    .journal
                    .clients
                    .iter_mut()
                    .find(|c| c.id == client_id)
                    .ok_or("Connection not found")?;
                let root = c
                    .repositories
                    .iter()
                    .find(|r| r.id == repository_id)
                    .ok_or("Repository access not found")?
                    .root
                    .clone();
                c.repositories.retain(|r| r.id != repository_id);
                for p in &mut b.journal.plans {
                    if p.client_id == client_id
                        && p.repository == root
                        && ["running", "approved", "ready", "pending_review"]
                            .contains(&p.status.as_str())
                    {
                        p.cancel_requested = true;
                        p.event(
                            "access_removed",
                            "Repository access removed. Remaining actions stopped.",
                        );
                        if p.status != "running" {
                            p.status = "cancelled".into()
                        }
                    }
                }
            }
            Native::Enable { enabled } => {
                b.journal.enabled = enabled;
                if !enabled {
                    for p in &mut b.journal.plans {
                        if ["approved", "running", "pending_review", "ready"]
                            .contains(&p.status.as_str())
                        {
                            p.cancel_requested = true;
                            p.event(
                                "connector_disabled",
                                "Remaining actions stopped by the user.",
                            );
                            if p.status != "running" {
                                p.status = "cancelled".into()
                            }
                        }
                    }
                }
            }
            Native::Register {
                name,
                root,
                content,
                provider,
            } => {
                if !b.journal.enabled {
                    return Err("Enable AI Connector first.".into());
                }
                if name.trim().is_empty() || name.len() > 80 {
                    return Err("Enter a connection name (up to 80 characters).".into());
                }
                if b.journal.clients.len() >= 50 {
                    return Err("Connection limit reached.".into());
                }
                let repositories = if root.trim().is_empty() {
                    Vec::new()
                } else {
                    vec![access(&root, content)?]
                };
                let id = random()?;
                let token = random()?;
                let credential = Credential {
                    socket: b.dir.join("b.sock").to_string_lossy().into_owned(),
                    client_id: id.clone(),
                    token: token.clone(),
                };
                let file = b.dir.join(format!("client-{}.json", &id[..12]));
                persist(&file, &credential)?;
                b.journal.clients.push(Client {
                    id: id.clone(),
                    name: name.trim().into(),
                    root: String::new(),
                    identity: String::new(),
                    repositories,
                    token_hash: digest(token.as_bytes()),
                    content: false,
                    read_source: content,
                    provider: provider
                        .filter(|p| {
                            [
                                "claude-code",
                                "claude-desktop",
                                "codex",
                                "chatgpt-desktop",
                                "cursor",
                                "other",
                            ]
                            .contains(&p.as_str())
                        })
                        .unwrap_or_else(other),
                    active: true,
                    created_at: now(),
                });
                b.save()?;
                return b.configuration(&id);
            }
            Native::Revoke { client_id } => {
                let c = b
                    .journal
                    .clients
                    .iter_mut()
                    .find(|c| c.id == client_id)
                    .ok_or("Connection not found")?;
                c.active = false;
                let credential = b.dir.join(format!("client-{}.json", &client_id[..12]));
                if credential.exists() {
                    fs::remove_file(credential).map_err(|e| e.to_string())?;
                }
                for p in &mut b.journal.plans {
                    if p.client_id == client_id {
                        p.cancel_requested = true;
                        p.event("revoked", "Connection revoked by the user.");
                        if ["approved", "pending_review", "ready"].contains(&p.status.as_str()) {
                            p.status = "cancelled".into()
                        }
                    }
                }
            }
            Native::Decline { plan_id, reason } => {
                if reason.as_ref().is_some_and(|s| s.trim().is_empty() || s.len() > 2000) {
                    return Err("Use a nonempty denial reason up to 2,000 characters.".into());
                }
                let p = b.plan_mut(&plan_id)?;
                if !["ready", "pending_review"].contains(&p.status.as_str()) {
                    return Err("Only a request waiting for review can be denied.".into());
                }
                p.cancel_requested = true;
                p.status = "cancelled".into();
                p.issue = reason.clone();
                p.event("denied", reason.as_deref().unwrap_or("User denied this request. No actions were approved."));
            }
            Native::Cancel { plan_id } => {
                let p = b.plan_mut(&plan_id)?;
                if cancellation_finished(&p.status) {
                    return Err("This request has finished. Its recorded outcome cannot be changed by cancelling it.".into());
                }
                p.cancel_requested = true;
                p.event(
                    "cancelled",
                    "User stopped this request. Completed actions remain completed.",
                );
                if p.status != "running" {
                    p.status = "cancelled".into()
                }
            }
            Native::Approve {
                plan_id,
                digest,
                through,
                acknowledge,
                acknowledged_risks,
            } => {
                if !b.journal.enabled {
                    return Err("AI Connector is turned off.".into());
                }
                let p = b
                    .journal
                    .plans
                    .iter()
                    .find(|p| p.id == plan_id)
                    .ok_or("Request not found")?
                    .clone();
                if !b
                    .journal
                    .clients
                    .iter()
                    .any(|c| c.id == p.client_id && c.active)
                {
                    return Err("This connection was revoked.".into());
                }
                if p.status != "pending_review"
                    || p.digest != digest
                    || through == 0
                    || through != p.steps.len()
                {
                    return Err("This request changed or is no longer waiting for approval.".into());
                }
                let reviewed = p.preview.as_ref().ok_or("No verifiable preview")?;
                let exact_commands = reviewed.command_workflow.is_some();
                let binary = reviewed.warnings.iter().any(|w| w.starts_with("Binary"));
                let publishes = p
                    .steps
                    .iter()
                    .take(through)
                    .any(|step| matches!(step, Step::Push { .. } | Step::PullRequest { .. }));
                if (binary || publishes || exact_commands) && !acknowledge {
                    return Err(
                        "Review the files and publishing destination before approval.".into(),
                    );
                }
                if (publishes || exact_commands)
                    && reviewed
                        .risks
                        .iter()
                        .any(|risk| !acknowledged_risks.contains(&risk.code))
                {
                    return Err(
                        "Acknowledge each publishing red flag before approving this push.".into(),
                    );
                }
                let current = match git::ai::preview(Path::new(&p.repository), &p.steps) {
                    Ok(preview) => preview,
                    Err(reason) => {
                        let p = b.plan_mut(&plan_id)?;
                        p.status = "blocked".into();
                        p.issue = Some(reason.clone());
                        p.event("validation_stopped","The reviewed plan could no longer be verified. No action was approved.");
                        b.save()?;
                        return Err(reason);
                    }
                };
                if current.fingerprint != p.preview.as_ref().unwrap().fingerprint {
                    let p = b.plan_mut(&plan_id)?;
                    p.status = "needs_review".into();
                    p.issue=Some("The repository changed after this preview. Ask the AI for a new plan so you can review the actual changes.".into());
                    p.event(
                        "stale",
                        "Approval refused because the verified effects changed.",
                    );
                    b.save()?;
                    return Err("The repository changed. This plan was not approved.".into());
                }
                let p = b.plan_mut(&plan_id)?;
                p.approval = Some(Approval {
                    at: now(),
                    expires_at: now() + 600,
                    through,
                    digest,
                    authority: "native_user".into(),
                    acknowledged_risks,
                });
                p.status = "approved".into();
                for step in p.execution.iter_mut().take(through) {
                    step.status = "approved".into();
                }
                p.event(
                    "approved",
                    &format!("Native user approved the first {through} step(s)."),
                );
                b.save()?;
                drop(b);
                let shared = shared.clone();
                std::thread::spawn(move || {
                    if let Err(e) = execute(&shared, &plan_id) {
                        if let Ok(mut b) = shared.lock() {
                            if let Ok(p) = b.plan_mut(&plan_id) {
                                if !cancellation_finished(&p.status) {
                                    p.status = "needs_review".into();
                                    p.issue = Some(e);
                                    p.event("executor_stopped","Execution stopped; inspect the activity record before retrying.");
                                }
                            }
                            let _ = b.save();
                        }
                    }
                });
                return Ok(json!({"accepted":true}));
            }
        }
        b.save()?;
        Ok(b.native_status(None))
    }
    pub fn tool(
        &mut self,
        credential: &Credential,
        name: &str,
        args: Value,
    ) -> Result<Value, String> {
        if name == "branchline_propose_command" {
            let proposal: CommandProposal = serde_json::from_value(args)
                .map_err(|e| format!("Provide the exact command, repository, explanation, and idempotency key: {e}"))?;
            if proposal.command.trim().is_empty() || proposal.command.len() > 131_070 {
                return Err("Provide a nonempty command up to 128 KiB.".into());
            }
            return self.tool(credential, "branchline_propose_plan", json!({
                "schemaVersion": proposal.schema_version,
                "repositoryPath": proposal.repository_path,
                "repositoryId": proposal.repository_id,
                "idempotencyKey": proposal.idempotency_key,
                "explanation": proposal.explanation,
                "reportedAi": proposal.reported_ai,
                "steps": [{"kind":"command.execute","program":"shell","argv":["-c",proposal.command],"stdin":proposal.stdin,"timeoutSeconds":proposal.timeout_seconds}]
            }));
        }
        let mut client = self.client(credential)?;
        if ["branchline_inspect", "branchline_propose_plan"].contains(&name) {
            let grant = if let Some(path) = args["repositoryPath"].as_str() {
                access(path, client.read_source)?
            } else if let Some(id) = args["repositoryId"].as_str() {
                let old = client
                    .repositories
                    .iter()
                    .find(|r| r.id == id)
                    .ok_or("Unknown repositoryId; pass an absolute repositoryPath instead.")?;
                let current = access(&old.root, client.read_source)?;
                RepositoryAccess {
                    id: old.id.clone(),
                    ..current
                }
            } else if client.repositories.len() == 1 {
                access(&client.repositories[0].root, client.read_source)?
            } else {
                return Err("Pass repositoryPath (an absolute local Git repository path), or choose a known repositoryId from branchline_repositories.".into());
            };
            client.root = grant.root.clone();
            client.identity = grant.identity.clone();
            client.content = client.read_source;
            let stored = self
                .journal
                .clients
                .iter_mut()
                .find(|c| c.id == client.id)
                .ok_or("Connection unavailable")?;
            if !stored.repositories.iter().any(|r| r.root == grant.root) {
                stored.repositories.push(grant);
                self.save()?;
            }
        }
        let allowed: &[&str] = match name {
            "branchline_capabilities" | "branchline_repositories" => &[],
            "branchline_command_catalog" => &["program", "command"],
            "branchline_inspect" => &["query", "skip", "repositoryId", "repositoryPath"],
            "branchline_propose_plan" => &[
                "schemaVersion",
                "idempotencyKey",
                "explanation",
                "steps",
                "reportedAi",
                "repositoryId",
                "repositoryPath",
            ],
            _ => &["planId"],
        };
        if !args.is_object()
            || args
                .as_object()
                .unwrap()
                .keys()
                .any(|key| !allowed.contains(&key.as_str()))
        {
            return Err("Unexpected tool arguments; use the advertised versioned schema.".into());
        }
        match name {
            "branchline_repositories" => Ok(
                json!({"repositories":client.repositories.iter().map(|r|json!({"repositoryId":r.id,"name":Path::new(&r.root).file_name().unwrap_or_default(),"root":r.root,"sourceAccess":client.read_source})).collect::<Vec<_>>()}),
            ),
            "branchline_command_catalog" => {
                let mut catalog: Value = serde_json::from_str(include_str!("command-catalog.json"))
                    .map_err(|e| e.to_string())?;
                if args
                    .as_object()
                    .is_none_or(|o| o.keys().any(|k| k != "program" && k != "command"))
                {
                    return Err("Use optional program and command filters only.".into());
                }
                let program = args
                    .get("program")
                    .map(|v| v.as_str().ok_or("Program must be git or gh"))
                    .transpose()?;
                if program.is_some_and(|p| p != "git" && p != "gh") {
                    return Err("Program must be git or gh".into());
                }
                let command = args
                    .get("command")
                    .map(|v| v.as_str().ok_or("Command must be text"))
                    .transpose()?;
                if let Some(rows) = catalog["commands"].as_array_mut() {
                    for row in rows.iter_mut() {
                        row["commandRequest"] = json!({"available":true,"tool":"branchline_propose_command","approval":"whole_exact_command","effectsVerified":false});
                    }
                    rows.retain(|row| {
                        program.is_none_or(|p| row["program"] == p)
                            && command.is_none_or(|q| {
                                row["command"]
                                    .as_str()
                                    .is_some_and(|c| c == q || c.starts_with(&format!("{q} ")))
                            })
                    });
                }
                catalog["scope"] = json!("Inventory of known Git and GitHub CLI commands. Every command can be submitted as an unchanged shell command for whole-request native approval. Command execution is not independent verification of effects; installed tools, authentication and noninteractive execution requirements apply.");
                Ok(catalog)
            }
            "branchline_capabilities" => Ok(
                json!({"schemaVersion":SCHEMA,"operations":["command.execute"],"proposalTool":"branchline_propose_command","commandExecution":{"shell":"/bin/sh","preservesCommand":true,"approval":"whole_request_only","effectsVerified":false,"interactive":false,"timeoutSeconds":{"default":120,"maximum":600},"maximumCommandBytes":131070,"maximumRequestBytes":1048576,"output":"withheld; captured byte counts and digests only"},"catalogTool":"branchline_command_catalog","waitTool":"branchline_wait_for_result","waitTimeoutMs":{"default":20000,"maximum":25000},"timeoutIsApproval":false,"approval":"Only the native user can approve the whole unchanged command, deny, or deny with a reason.","explanationGuidance":"Explain the outcome, reason, targets and uncertainty in everyday language. Supply explicit commit message and PR title/body/head/base. Structured review details never rewrite the command or prove all effects."}),
            ),
            "branchline_inspect" => {
                let query = args
                    .get("query")
                    .and_then(Value::as_str)
                    .unwrap_or("status");
                match query {
                    "status" => git::ai::metadata(Path::new(&client.root)),
                    "history" => git::ai::history(
                        Path::new(&client.root),
                        args.get("skip")
                            .and_then(Value::as_u64)
                            .unwrap_or(0)
                            .min(10000) as usize,
                    ),
                    "staged_diff" => {
                        if !client.content {
                            return Err("This client has metadata access only. Allow source diffs for this repository in Connections to read them. No new MCP connection is needed.".into());
                        }
                        let p = git::ai::preview(
                            Path::new(&client.root),
                            &[Step::Commit {
                                message: "Preview staged changes".into(),
                            }],
                        )?;
                        Ok(json!({"patch":p.patch,"files":p.files,"snapshotId":p.fingerprint}))
                    }
                    _ => Err("Unsupported query. Use status, history, or staged_diff.".into()),
                }
            }
            "branchline_propose_plan" => {
                let proposal:Proposal=serde_json::from_value(args.clone()).map_err(|e|format!("Provide a versioned plan with a plain-language summary, reason, impact, uncertainties, and supported typed steps: {e}"))?;
                let _selected_repository = (
                    proposal.repository_id.as_deref(),
                    proposal.repository_path.as_deref(),
                );
                if proposal.schema_version != SCHEMA {
                    return Err("Unsupported plan schema version. Supported: 1.".into());
                }
                for text in [
                    &proposal.explanation.summary,
                    &proposal.explanation.reason,
                    &proposal.explanation.impact,
                ] {
                    if text.trim().is_empty() || text.len() > 1000 {
                        return Err("Write a short, nonempty human-friendly summary, reason, and impact (up to 1,000 characters each).".into());
                    }
                }
                if proposal.explanation.uncertainties.len() > 8
                    || proposal
                        .explanation
                        .uncertainties
                        .iter()
                        .any(|s| s.len() > 1000)
                    || proposal.reported_ai.as_ref().is_some_and(|s| s.len() > 120)
                    || proposal.steps.is_empty()
                    || proposal.steps.len() > 64
                    || proposal.steps.iter().any(|s| {
                        s.get("kind")
                            .and_then(Value::as_str)
                            .is_none_or(|kind| kind.is_empty() || kind.len() > 80)
                    })
                    || proposal.idempotency_key.is_empty()
                    || proposal.idempotency_key.len() > 120
                {
                    return Err("Plan exceeds the supported limits.".into());
                }
                let input_digest = digest(&serde_json::to_vec(&args).map_err(|e| e.to_string())?);
                if let Some(p) = self.journal.plans.iter().find(|p| {
                    p.client_id == client.id && p.idempotency_key == proposal.idempotency_key
                }) {
                    return if p.input_digest == input_digest {
                        Ok(Self::redacted(p, client.content))
                    } else {
                        Err("That idempotency key already belongs to a different plan.".into())
                    };
                }
                if self
                    .journal
                    .plans
                    .iter()
                    .filter(|p| p.client_id == client.id)
                    .count()
                    >= 100
                {
                    return Err("This connection has reached its 100-request journal limit. Review/export existing activity before enrolling another connection.".into());
                }
                let parsed = proposal
                    .steps
                    .iter()
                    .cloned()
                    .map(serde_json::from_value)
                    .collect::<Result<Vec<Step>, _>>();
                let (steps,preview,issue)=match parsed{
                    Ok(steps)=>match git::ai::preview(Path::new(&client.root),&steps){Ok(p)=>(steps,Some(p),None),Err(e)=>(steps,None,Some(e))},
                    Err(_)=>(Vec::new(),None,Some("This plan contains an operation the connector cannot preview yet. Ask for a supported typed plan, or perform the unsupported step in Branchline or Git. An AI explanation cannot replace a verifiable preview.".into()))
                };
                let plan_digest=digest(&serde_json::to_vec(&json!({"input":args,"preview":preview,"repository":client.root,"clientId":client.id})).map_err(|e|e.to_string())?);
                let execution = steps
                    .iter()
                    .map(|s| Execution {
                        kind: match s {
                            Step::Commit { .. } => "commit.create",
                            Step::Push { .. } => "remote.push",
                            Step::PullRequest { .. } => "pr.create",
                            Step::Command { .. } => "command.execute",
                        }
                        .into(),
                        status: "not_approved".into(),
                        result: None,
                        error: None,
                        commands: Vec::new(),
                    })
                    .collect();
                let mut plan = Plan {
                    schema_version: SCHEMA,
                    id: random()?,
                    version: 1,
                    client_id: client.id,
                    client_name: client.name,
                    client_provider: client.provider,
                    reported_ai: proposal.reported_ai,
                    repository: client.root,
                    idempotency_key: proposal.idempotency_key,
                    input_digest,
                    explanation: proposal.explanation,
                    requested_steps: proposal.steps,
                    steps,
                    preview,
                    digest: plan_digest,
                    status: if issue.is_some() { "blocked" } else { "ready" }.into(),
                    issue,
                    created_at: now(),
                    approval: None,
                    execution,
                    events: Vec::new(),
                    cancel_requested: false,
                };
                plan.event(
                    "proposed",
                    "Received an AI proposal. No execution was authorized.",
                );
                let result = Self::redacted(&plan, client.content);
                self.journal.plans.push(plan);
                self.save()?;
                Ok(result)
            }
            "branchline_request_review" | "branchline_get_plan" | "branchline_cancel_plan" => {
                let id = args["planId"].as_str().ok_or("Provide planId")?;
                let mut notify = false;
                let p = self.plan_mut(id)?;
                if p.client_id != client.id {
                    return Err("Request not found".into());
                }
                if name == "branchline_request_review"
                    && ["ready", "blocked"].contains(&p.status.as_str())
                    && !p
                        .events
                        .iter()
                        .any(|event| event.kind == "review_requested")
                {
                    if p.status == "ready" {
                        p.status = "pending_review".into();
                    }
                    notify = true;
                    p.event(
                        "review_requested",
                        "Waiting for the native user; no action has started.",
                    )
                }
                if name == "branchline_cancel_plan" && !cancellation_finished(&p.status)
                {
                    p.cancel_requested = true;
                    p.event(
                        "client_cancelled",
                        "Client requested cancellation. Completed actions remain completed.",
                    );
                    if p.status != "running" {
                        p.status = "cancelled".into()
                    }
                }
                let content = client.read_source;
                let result = Self::redacted(p, content);
                if name != "branchline_get_plan" {
                    self.save()?;
                }
                if notify && self.journal.auto_open {
                    self.focus_plan = Some(id.to_string());
                }
                if review_signals::should_dispatch(notify, self.journal.auto_open, self.journal.notifications) {
                    if let Some(notify) = &self.notifier {
                        notify(id)
                    }
                }
                Ok(result)
            }
            _ => Err("Unknown tool. There is no MCP approve or execute tool.".into()),
        }
    }
}
fn execute(shared: &Shared, id: &str) -> Result<(), String> {
    let gate = shared
        .lock()
        .map_err(|_| "Connector unavailable")?
        .operation_gate
        .clone();
    let _operation = gate.lock().map_err(|_| "Execution queue unavailable")?;
    let plan = {
        let mut b = shared.lock().map_err(|_| "Connector unavailable")?;
        let p = b.plan_mut(id)?;
        if p.status != "approved" {
            return Err("Approval is no longer active".into());
        }
        p.status = "running".into();
        p.event("started", "Execution started under native approval.");
        let p = p.clone();
        b.save()?;
        p
    };
    let approval = plan.approval.as_ref().ok_or("Missing native approval")?;
    let preview = plan.preview.as_ref().ok_or("Missing preview")?;
    let mut source = preview.head.clone();
    for (i, step) in plan.steps.iter().enumerate().take(approval.through) {
        {
            let mut b = shared.lock().map_err(|_| "Connector unavailable")?;
            let enabled = b.journal.enabled;
            let client_active = b
                .journal
                .clients
                .iter()
                .any(|c| c.id == plan.client_id && c.active);
            let p = b.plan_mut(id)?;
            if !enabled || !client_active || p.cancel_requested || now() > approval.expires_at {
                p.status = "cancelled".into();
                p.event("stopped", "Remaining actions stopped at a step boundary.");
                b.save()?;
                return Ok(());
            }
            p.execution[i].status = "running".into();
            p.event("step_started", &format!("Step {} started", i + 1));
            b.save()?;
        }
        let output = shared.lock().map_err(|_| "Connector unavailable")?.output.clone();
        let result = git::ai::execute_with_output(
            Path::new(&plan.repository),
            step,
            preview,
            &source,
            &mut |program, argv, input, finished, outcome| {
                let mut b = shared.lock().map_err(|_| "Connector unavailable")?;
                let p = b.plan_mut(id)?;
                if !finished {
                    if p.cancel_requested {
                        return Err("Stopped before the next Git command; inspect completed actions before retrying.".into());
                    }
                    p.execution[i].commands.push(Command {
                        program: program.to_string(),
                        argv: argv.to_vec(),
                        started_at: now(),
                        finished_at: None,
                        outcome: None,
                        stdin_digest: input.map(digest),
                    });
                } else {
                    let c = p.execution[i]
                        .commands
                        .last_mut()
                        .ok_or("Missing command intent")?;
                    c.finished_at = Some(now());
                    c.outcome = outcome.map(str::to_string);
                }
                b.save()
            },
            &mut |out, err, out_cut, err_cut| {
                if let Ok(mut store) = output.lock() {
                    store.update(id, i, out, err, out_cut, err_cut);
                }
            },
        );
        let mut b = shared.lock().map_err(|_| "Connector unavailable")?;
        let p = b.plan_mut(id)?;
        match result {
            Ok(mut value) => {
                let raw = matches!(step, Step::Command { .. });
                if raw {
                    if let Some(object) = value.as_object_mut() {
                        object.remove("stdout");
                        object.remove("stderr");
                        object.insert("outputOmitted".into(), json!(true));
                    }
                    if value["processSucceeded"] != true {
                        let uncertain = value["timedOut"] == true || value["exitCode"].is_null();
                        let message = if uncertain { "Command completion is unconfirmed. Effects may have occurred; inspect before retrying." } else { "The command exited with an error. Earlier effects may have occurred; inspect before retrying." };
                        p.execution[i].status = if uncertain { "outcome_unknown" } else { "failed" }.into();
                        p.execution[i].result = Some(value);
                        p.execution[i].error = Some(message.into());
                        p.issue = Some(message.into());
                        p.status = if i > 0 { "partially_completed" } else if uncertain { "outcome_unknown" } else { "failed" }.into();
                        p.event("stopped", message);
                        b.save()?;
                        return Ok(());
                    }
                }
                if let Some(oid) = value["commitOid"].as_str() {
                    source = oid.into()
                }
                p.execution[i].status = "succeeded".into();
                p.execution[i].result = Some(value);
                p.event(
                    "step_succeeded",
                    &format!("Step {} {}", i + 1, if raw { "exited successfully; effects are not independently verified" } else { "completed and verified" }),
                );
            }
            Err(error) => {
                let uncertain = p.execution[i].commands.iter().any(|c| {
                    c.outcome.as_deref() == Some("failed_or_uncertain") || c.finished_at.is_none()
                });
                p.execution[i].status = if uncertain {
                    "outcome_unknown"
                } else {
                    "needs_review"
                }
                .into();
                p.execution[i].error = Some(error.clone());
                p.issue = Some(error);
                p.status = if i > 0 {
                    "partially_completed"
                } else {
                    "needs_review"
                }
                .into();
                p.event("stopped","Execution stopped. No later steps were started. Inspect the commands and repository before retrying.");
                b.save()?;
                return Ok(());
            }
        }
        b.save()?;
    }
    let mut b = shared.lock().map_err(|_| "Connector unavailable")?;
    let p = b.plan_mut(id)?;
    p.status = if approval.through < plan.steps.len() {
        "partially_completed"
    } else {
        "succeeded"
    }
    .into();
    p.event("finished", if preview.command_workflow.is_some() { "The approved command request exited successfully. Its effects are not independently verified." } else { "All approved actions completed and were verified." });
    b.save()
}
fn serve(mut stream: UnixStream, shared: Shared) -> Result<(), String> {
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(std::time::Duration::from_secs(5)))
        .map_err(|e| e.to_string())?;
    // A same-user socket is a transport boundary, not proof of an AI/model identity.
    let mut uid = 0;
    let mut gid = 0;
    if unsafe { libc::getpeereid(stream.as_raw_fd(), &mut uid, &mut gid) } != 0
        || uid != unsafe { libc::geteuid() }
    {
        return Err("Invalid local peer".into());
    }
    let mut bytes = Vec::new();
    BufReader::new(&mut stream)
        .take(1_048_577)
        .read_until(b'\n', &mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 1_048_576 {
        return Err("Request too large".into());
    }
    let result = (|| {
        let v: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        let credential: Credential =
            serde_json::from_value(v["credential"].clone()).map_err(|_| "Missing credential")?;
        let name = v["name"].as_str().ok_or("Missing tool name")?;
        if !mcp::tools()["tools"].as_array().is_some_and(|tools| tools.iter().any(|tool| tool["name"] == name)) {
            return Err("Unknown tool. Submit an exact command with branchline_propose_command.".into());
        }
        if name == "branchline_wait_for_result" {
            return wait::run(&shared, &credential, v["arguments"].clone());
        }
        let mut b = shared.lock().map_err(|_| "Connector unavailable")?;
        b.tool(&credential, name, v["arguments"].clone())
    })();
    let result = match result {
        Ok(v) => json!({"result":v}),
        Err(e) => json!({"error":e}),
    };
    serde_json::to_writer(&mut stream, &result).map_err(|e| e.to_string())?;
    stream.write_all(b"\n").map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod command_tests;
#[cfg(test)]
mod blind_gate_tests;
