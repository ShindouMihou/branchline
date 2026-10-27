//! Versioned, deliberately small stdio adapter. No approval/execute route exists here.
use super::*;
const VERSIONS: [&str; 3] = ["2025-11-25", "2025-06-18", "2025-03-26"];
fn object(properties: Value, required: &[&str]) -> Value {
    json!({"type":"object","properties":properties,"required":required,"additionalProperties":false})
}
pub fn tools() -> Value {
    let explanation = object(
        json!({"summary":{"type":"string","minLength":1,"maxLength":1000,"description":"The outcome in a short everyday sentence. No jargon."},"reason":{"type":"string","minLength":1,"maxLength":1000,"description":"Why this helps with the user's request."},"impact":{"type":"string","minLength":1,"maxLength":1000,"description":"What changes for the user, including what gets published."},"uncertainties":{"type":"array","maxItems":8,"items":{"type":"string","maxLength":1000},"description":"What you cannot know yet. Be candid; use an empty list only when appropriate."}}),
        &["summary", "reason", "impact", "uncertainties"],
    );
    let mut tools = Vec::new();
    for(name,description,schema,read_only)in[
        ("branchline_command_catalog","Browse the installed Git/GitHub command inventory. Any exact command can be requested for whole native approval, including commands absent from this inventory. This catalog never executes.",object(json!({"program":{"type":"string","enum":["git","gh"]},"command":{"type":"string","maxLength":120}}),&[]),true),
        ("branchline_capabilities","Learn supported Git operations and how to explain a plan clearly. Connection does not authorize mutations.",object(json!({}),&[]),true),
        ("branchline_repositories","List known repositories. This global connection can also use any local repository by absolute repositoryPath. Choose repositoryId for inspection and plans.",object(json!({}),&[]),true),
        ("branchline_inspect","Read an authorized repository selected by repositoryId. Source diffs require a separate content grant. No network requests or mutations.",object(json!({"repositoryPath":{"type":"string","description":"Absolute path to any local Git working repository. No repository enrollment is required."},"repositoryId":{"type":"string","description":"Choose an authorized ID from branchline_repositories. Required when more than one repository is available."},"query":{"enum":["status","history","staged_diff"],"type":"string"},"skip":{"type":"integer","minimum":0,"maximum":10000}}),&["query"]),true),
        ("branchline_propose_command","Prepare one exact shell command or chain for whole-request native approval. All Git and GitHub CLI combinations may be submitted. The command runs unchanged via /bin/sh -c only after human approval. Structured details assist review but do not restrict or independently verify effects. No partial approval. Noninteractive execution; output is withheld from receipts because it may contain secrets.",object(json!({"repositoryPath":{"type":"string","description":"Absolute starting Git repository directory; commands may explicitly target other paths or repositories."},"repositoryId":{"type":"string"},"schemaVersion":{"const":1,"type":"integer"},"idempotencyKey":{"type":"string","minLength":1,"maxLength":120},"reportedAi":{"type":"string","maxLength":120},"explanation":explanation,"command":{"type":"string","minLength":1,"maxLength":131070,"description":"Exact POSIX shell command including arguments, quotes, &&, ||, pipes, or other shell syntax. Preserved unchanged. Prefer explicit commit message, PR title/body/head/base so the user can review them."},"stdin":{"type":"string","maxLength":524288},"timeoutSeconds":{"type":"integer","minimum":1,"maximum":600,"default":120}}),&["schemaVersion","idempotencyKey","explanation","command"]),false),
        ("branchline_request_review","Queue this plan in Branchline for the human to review. Returns promptly without approval. Next call branchline_wait_for_result on the same plan to await the actual outcome.",object(json!({"planId":{"type":"string"}}),&["planId"]),false),
        ("branchline_wait_for_result","Wait for an owned plan to finish or need attention. Read-only; never approves, executes, cancels, or raises a window. Default 20 seconds, maximum 25 seconds. A timed_out result is still waiting, never approval or success. Wait again on the same plan; report actual per-step results and the verified pullRequestUrl when present.",object(json!({"planId":{"type":"string","minLength":1,"maxLength":128},"timeoutMs":{"type":"integer","minimum":0,"maximum":25000,"default":20000}}),&["planId"]),true),
        ("branchline_get_plan","Read an owned plan, native approval scope, per-step results, timestamps, and command history. A pending plan has not run.",object(json!({"planId":{"type":"string"}}),&["planId"]),true),
        ("branchline_cancel_plan","Stop your queued plan or request cancellation before the next command. Completed or uncertain effects are not undone.",object(json!({"planId":{"type":"string"}}),&["planId"]),false),
    ]{tools.push(json!({"name":name,"description":description,"inputSchema":schema,"annotations":{"readOnlyHint":read_only,"destructiveHint":false,"openWorldHint":!read_only}}))}
    if let Some(tool) = tools
        .iter_mut()
        .find(|tool| tool["name"] == "branchline_wait_for_result")
    {
        tool["outputSchema"] = object(
            json!({"schemaVersion":{"const":1},"planId":{"type":"string"},"status":{"type":"string"},"waitOutcome":{"enum":["finished","needs_attention","timed_out"]},"timedOut":{"type":"boolean"},"terminal":{"type":"boolean"},"succeeded":{"type":"boolean"},"effectsVerified":{"type":"boolean"},"grantsApproval":{"const":false},"waitedMs":{"type":"integer","minimum":0},"pullRequestUrl":{"type":["string","null"]},"message":{"type":"string"},"plan":{"type":"object"}}),
            &[
                "schemaVersion",
                "planId",
                "status",
                "waitOutcome",
                "timedOut",
                "terminal",
                "succeeded",
                "effectsVerified",
                "grantsApproval",
                "waitedMs",
                "pullRequestUrl",
                "message",
                "plan",
            ],
        );
    }
    json!({"tools":tools})
}
fn rpc(id: Value, result: Value) -> Value {
    json!({"jsonrpc":"2.0","id":id,"result":result})
}
fn error(id: Value, code: i32, message: &str) -> Value {
    json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}})
}
pub fn run(path: &Path) -> Result<(), String> {
    let meta = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.permissions().mode() & 0o077 != 0 {
        return Err("Credential file must be a private regular file (0600).".into());
    }
    let credential: Credential =
        serde_json::from_slice(&fs::read(path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let stdin = std::io::stdin();
    let mut reader = stdin.lock();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let mut initialized = false;
    let mut ready = false;
    loop {
        let mut line = Vec::new();
        let count = reader
            .by_ref()
            .take(1_048_577)
            .read_until(b'\n', &mut line)
            .map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        if line.len() > 1_048_576 {
            return Err("MCP request exceeds 1 MiB.".into());
        }
        let result = match serde_json::from_slice::<Value>(&line) {
            Err(_) => Some(error(Value::Null, -32700, "Invalid JSON")),
            Ok(v) => {
                let id = v.get("id").cloned();
                let method = v["method"].as_str().unwrap_or("");
                if v["jsonrpc"] != "2.0" || method.is_empty() {
                    Some(error(
                        id.unwrap_or(Value::Null),
                        -32600,
                        "Invalid JSON-RPC request",
                    ))
                } else if let Some(id) = id {
                    let requested =
                        v["params"]["_meta"]["io.modelcontextprotocol/protocolVersion"].as_str();
                    if requested.is_some_and(|s| !VERSIONS.contains(&s)) {
                        Some(
                            json!({"jsonrpc":"2.0","id":id,"error":{"code":-32022,"message":"Unsupported protocol version","data":{"supported":VERSIONS,"requested":requested}}}),
                        )
                    } else if method == "initialize" {
                        if initialized {
                            Some(error(id, -32600, "Already initialized"))
                        } else {
                            initialized = true;
                            let proposed = v["params"]["protocolVersion"].as_str().unwrap_or("");
                            let version = if VERSIONS.contains(&proposed) {
                                proposed
                            } else {
                                VERSIONS[0]
                            };
                            Some(rpc(
                                id,
                                json!({"protocolVersion":version,"capabilities":{"tools":{}},"serverInfo":{"name":"branchline","version":env!("CARGO_PKG_VERSION")},"instructions":"Use Branchline for visible human approval of Git plans. One global connection can access multiple allowed repositories; pass an absolute repositoryPath, or list branchline_repositories and choose a known repositoryId before inspecting or proposing. Explain outcome, reason and impact in everyday language; name uncertainty. Never claim that submitting or requesting review grants approval. After requesting review, call branchline_wait_for_result until finished or needing attention. timed_out means still waiting, never approval or completion; wait again on the same plan. Report actual step results and the verified pullRequestUrl, never a guessed URL. If waiting must stop, say the plan remains pending. Only the user can approve in the native app. Submit the complete unchanged command with branchline_propose_command. Native approval covers the whole command only. Include explicit commit messages and PR title/body/head/base for structured review. Commands run using /bin/sh and the user account; their effects are not independently verified. Report actual exit results; stdout/stderr are withheld and URLs must not be guessed. Denial reasons require a revised new request."}),
                            ))
                        }
                    } else if method == "ping" {
                        Some(rpc(id, json!({})))
                    } else if !ready {
                        Some(error(id,-32000,"Initialize the 2025 protocol and send notifications/initialized first."))
                    } else {
                        Some(match method {
                            "tools/list" => rpc(id, tools()),
                            "tools/call" => {
                                let params = &v["params"];
                                let name = params["name"].as_str().unwrap_or("");
                                let args = params.get("arguments").cloned().unwrap_or(json!({}));
                                let result = forward(&credential, name, args);
                                match result {
                                    Ok(value) => rpc(
                                        id,
                                        json!({"content":[{"type":"text","text":serde_json::to_string(&value).unwrap()}],"structuredContent":value,"isError":false}),
                                    ),
                                    Err(e) => rpc(
                                        id,
                                        json!({"content":[{"type":"text","text":e}],"isError":true}),
                                    ),
                                }
                            }
                            _ => error(id, -32601, "Method not found"),
                        })
                    }
                } else {
                    if method == "notifications/initialized" && initialized {
                        ready = true
                    }
                    None
                }
            }
        };
        if let Some(result) = result {
            serde_json::to_writer(&mut out, &result).map_err(|e| e.to_string())?;
            out.write_all(b"\n").map_err(|e| e.to_string())?;
            out.flush().map_err(|e| e.to_string())?
        }
    }
    Ok(())
}
pub(super) fn forward(credential: &Credential, name: &str, args: Value) -> Result<Value, String> {
    let mut stream = connect_or_open(credential)?;
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(65)))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(std::time::Duration::from_secs(5)))
        .map_err(|e| e.to_string())?;
    serde_json::to_writer(
        &mut stream,
        &json!({"credential":credential,"name":name,"arguments":args}),
    )
    .map_err(|e| e.to_string())?;
    stream.write_all(b"\n").map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    BufReader::new(stream)
        .take(4_194_305)
        .read_until(b'\n', &mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 4_194_304 {
        return Err("Connector result exceeds the supported size.".into());
    }
    let response: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    if let Some(error) = response["error"].as_str() {
        Err(error.to_string())
    } else {
        Ok(response["result"].clone())
    }
}

fn connect_or_open(credential: &Credential) -> Result<UnixStream, String> {
    if let Ok(stream) = UnixStream::connect(&credential.socket) {
        return Ok(stream);
    }
    let policy = Path::new(&credential.socket)
        .parent()
        .ok_or("Invalid connector location")?
        .join("launch-policy.json");
    let policy: Value = serde_json::from_slice(
        &fs::read(policy).map_err(|_| "Open Branchline manually to enable request prompts")?,
    )
    .map_err(|_| "Invalid launch policy")?;
    if !super::review_signals::launch_allowed(
        policy["enabled"] == true,
        policy["autoOpen"] == true,
        policy["notifications"] == true,
        policy["activeClientIds"].as_array().is_some_and(|ids| {
            ids.iter().any(|id| id.as_str() == Some(&credential.client_id))
        }),
    )
    {
        return Err("Automatic opening is disabled or this connection was revoked. Open Branchline manually.".into());
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let bundle = exe
        .ancestors()
        .find(|p| p.extension().is_some_and(|x| x == "app"))
        .ok_or("Open the Branchline desktop app manually")?;
    let status = std::process::Command::new("/usr/bin/open")
        .arg("-g")
        .arg("-a")
        .arg(bundle)
        .args(["--args", "--mcp-background"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err("Branchline could not be opened".into());
    }
    for _ in 0..80 {
        if let Ok(stream) = UnixStream::connect(&credential.socket) {
            return Ok(stream);
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    Err("Branchline did not become available. Open it manually.".into())
}
