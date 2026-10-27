//! Narrow primitives for the AI executor. No general Git command is exposed to MCP.
mod github;
mod command_preview;
use command_preview::command_preview;
pub mod commands;
pub mod command_details;
pub mod review_diff;
use super::*;
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum Step {
    #[serde(rename = "command.execute")]
    Command {
        program: String,
        argv: Vec<String>,
        #[serde(default)]
        stdin: Option<String>,
        #[serde(default, rename = "timeoutSeconds")]
        timeout_seconds: Option<u64>,
    },
    #[serde(rename = "commit.create")]
    Commit { message: String },
    #[serde(rename = "remote.push")]
    Push { remote: String, branch: String },
    #[serde(rename = "pr.create")]
    PullRequest {
        remote: String,
        head: String,
        base: String,
        title: String,
        body: String,
        draft: bool,
    },
}
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RiskFlag {
    pub code: String,
    pub title: String,
    pub detail: String,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub root: String,
    pub git_dir: String,
    pub common_dir: String,
    pub head: String,
    pub branch: String,
    pub staged_token: String,
    pub patch: String,
    pub files: Vec<String>,
    pub warnings: Vec<String>,
    #[serde(default)]
    pub risks: Vec<RiskFlag>,
    pub push: Option<PushPreview>,
    #[serde(default)]
    pub pull_request: Option<github::Preview>,
    pub fingerprint: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command_workflow: Option<commands::Manifest>,
    #[serde(default)]
    pub command_details: Vec<command_details::Detail>,
}
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PushPreview {
    pub remote: String,
    pub branch: String,
    pub destination: String,
    pub expected_oid: String,
    pub commits: Vec<String>,
}
pub type CommandLog<'a> =
    dyn FnMut(&str, &[String], Option<&[u8]>, bool, Option<&str>) -> Result<(), String> + 'a;
fn checked(
    root: &Path,
    args: &[&str],
    input: Option<&[u8]>,
    log: &mut CommandLog<'_>,
) -> Result<Vec<u8>, String> {
    let mut argv = vec!["-c".to_string(), "core.fsmonitor=false".to_string()];
    argv.extend(args.iter().map(|s| s.to_string()));
    log("git", &argv, input, false, None)?;
    let result = run(root, args, input);
    // Do not log stdout, stderr, credentials, or arbitrary hook output.
    log(
        "git",
        &argv,
        input,
        true,
        Some(if result.is_ok() {
            "succeeded"
        } else {
            "failed_or_uncertain"
        }),
    )?;
    result.map_err(|_|"Git did not confirm this command completed. Check the activity record and repository before retrying; remote effects may already have happened. Raw Git output is omitted to keep credentials out of the journal.".into())
}
fn txt(root: &Path, args: &[&str]) -> Result<String, String> {
    text(run(root, args, None)?).map(|s| s.trim_end_matches('\n').to_string())
}
fn staged_patch(root: &Path) -> Result<String, String> {
    txt(
        root,
        &[
            "diff",
            "--cached",
            "--binary",
            "--full-index",
            "--no-ext-diff",
            "--no-textconv",
            "--no-renames",
        ],
    )
}
fn identity(root: &Path) -> Result<(String, String), String> {
    use std::os::unix::fs::MetadataExt;
    let identify = |path: String| -> Result<String, String> {
        let m = std::fs::metadata(&path).map_err(|e| e.to_string())?;
        Ok(format!("{}@{}:{}", path, m.dev(), m.ino()))
    };
    Ok((
        identify(txt(root, &["rev-parse", "--absolute-git-dir"])?)?,
        identify(txt(
            root,
            &["rev-parse", "--path-format=absolute", "--git-common-dir"],
        )?)?,
    ))
}
pub fn repository_identity(root: &Path) -> Result<String, String> {
    let (a, b) = identity(root)?;
    Ok(format!("{a}|{b}"))
}
fn helpers(root: &Path) -> Result<(), String> {
    // A first connector release cannot safely explain arbitrary repository-executed code.
    let hooks = txt(root, &["rev-parse", "--git-path", "hooks"])?;
    let hooks = PathBuf::from(hooks);
    let hooks = if hooks.is_absolute() {
        hooks
    } else {
        root.join(hooks)
    };
    if let Ok(entries) = std::fs::read_dir(hooks) {
        for entry in entries.flatten() {
            use std::os::unix::fs::PermissionsExt;
            let name = entry.file_name();
            if name.to_string_lossy().ends_with(".sample") {
                continue;
            }
            if entry
                .metadata()
                .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
                .unwrap_or(true)
            {
                return Err("This repository has active Git hooks. Their effects cannot be previewed by the connector yet. Run this action in Branchline or Git, then ask the AI for a new plan.".into());
            }
        }
    }
    let config=git(root,&["config","--get-regexp","^(filter\\.|core\\.sshcommand$|core\\.gitproxy$|remote\\..*\\.(receivepack|uploadpack)$|url\\.|include)"],None)?;
    let lines = String::from_utf8_lossy(&config.1);
    if !lines.trim().is_empty() {
        return Err("Custom Git filters, signing, includes, or transport helpers need a manual review. The connector will not run this plan until that execution policy is supported.".into());
    }
    for key in ["commit.gpgsign", "push.gpgsign"] {
        let effective = git(root, &["config", "--get", key], None)?;
        let value = String::from_utf8_lossy(&effective.1).trim().to_lowercase();
        if effective.0 == 0 && !matches!(value.as_str(), "false" | "no" | "0" | "off") {
            return Err("Signed Git actions require a manual step in this connector version. Your signing settings have not been changed.".into());
        }
    }
    let local=git(root,&["config","--local","--get-regexp","^(credential\\.|core\\.sshcommand$|core\\.gitproxy$|remote\\..*\\.(receivepack|uploadpack)$)"],None)?;
    if local.0 == 0 {
        return Err("Repository-specific credential or transport helpers require a manual review. This connector only uses your ordinary Git authentication settings.".into());
    }
    Ok(())
}
fn safe_url(raw: &str) -> Result<String, String> {
    if raw.contains('\n') || raw.starts_with('-') || raw.contains("::") || raw.starts_with("ext:") {
        return Err(
            "Unsupported remote transport. Use a normal SSH, HTTPS, or local Git remote.".into(),
        );
    }
    if raw.contains("://") {
        if !["https://", "ssh://", "file://"]
            .iter()
            .any(|p| raw.starts_with(p))
        {
            return Err("This remote protocol is not supported by the AI connector.".into());
        }
        let authority = raw
            .split("://")
            .nth(1)
            .unwrap_or("")
            .split('/')
            .next()
            .unwrap_or("");
        if authority.contains('@') || raw.contains('?') || raw.contains('#') {
            return Err("Use a remote URL without embedded credentials, query parameters, or fragments before connecting an AI.".into());
        }
    }
    Ok(raw.to_string())
}
fn push_preview(
    root: &Path,
    remote: &str,
    branch: &str,
    head: &str,
) -> Result<PushPreview, String> {
    if !operations::remotes(root)?.contains(&remote.to_string()) {
        return Err("Choose a configured remote.".into());
    }
    operations::valid_branch(root, branch)?;
    let urls = txt(root, &["remote", "get-url", "--push", "--all", remote])?;
    if urls.lines().count() != 1 {
        return Err("This remote pushes to multiple destinations. Use Git for this action; the connector requires one exact destination.".into());
    }
    let destination = safe_url(&urls)?;
    let reference = format!("refs/remotes/{remote}/{branch}");
    let old = git(root, &["rev-parse", "--verify", &reference], None)?;
    // An absent cached ref means a create-only push intent, not proof of absence.
    // The server must confirm absence after approval; the empty lease makes creation atomic.
    let expected_oid = if old.0 == 0 {
        text(old.1)?.trim().to_string()
    } else {
        String::new()
    };
    if !expected_oid.is_empty() {
        validate_oid(&expected_oid)?;
        if git(
            root,
            &["merge-base", "--is-ancestor", &expected_oid, head],
            None,
        )?
        .0 != 0
        {
            return Err("The local branch and destination have diverged. The connector will not rewrite remote history.".into());
        }
    }
    let range = format!("{expected_oid}..{head}");
    let remote_refs = format!("--remotes={remote}");
    let args = if expected_oid.is_empty() {
        vec![
            "log",
            "--max-count=201",
            "--format=%H %s",
            head,
            "--not",
            &remote_refs,
        ]
    } else {
        vec!["log", "--max-count=201", "--format=%H %s", &range]
    };
    let commits: Vec<String> = txt(root, &args)?.lines().map(str::to_string).collect();
    if commits.len() > 200 {
        return Err("This push includes more than 200 commits. Review the publication in Git before requesting a smaller plan.".into());
    }
    Ok(PushPreview {
        remote: remote.into(),
        branch: branch.into(),
        destination,
        expected_oid,
        commits,
    })
}
pub fn preview(root: &Path, steps: &[Step]) -> Result<Preview, String> {
    if steps.iter().any(|step| matches!(step, Step::Command { .. })) {
        return command_preview(root, steps);
    }
    if steps.is_empty() || steps.len() > 3 {
        return Err("Use a commit, a push, and optionally a pull request, in that order.".into());
    }
    let mut seen_commit = false;
    let mut seen_push = false;
    let mut seen_pr = false;
    for step in steps {
        match step {
            Step::Command { .. } => unreachable!(),
            Step::Commit { message } => {
                if seen_commit
                    || seen_push
                    || seen_pr
                    || message.trim().is_empty()
                    || message.len() > 10000
                {
                    return Err("Use one nonempty commit message before any push.".into());
                }
                seen_commit = true;
            }
            Step::Push { .. } => {
                if seen_push || seen_pr {
                    return Err("Use only one push destination in a plan.".into());
                }
                seen_push = true;
            }
            Step::PullRequest {
                remote,
                head,
                base,
                title,
                body,
                ..
            } => {
                if seen_pr
                    || !seen_push
                    || title.trim().is_empty()
                    || title.len() > 1000
                    || title.chars().any(char::is_control)
                    || body.contains('\0')
                    || body.len() > 60000
                    || head == base
                {
                    return Err("Place one pull request after its push, with a title and different source and target branches.".into());
                }
                operations::valid_branch(root, head)?;
                operations::valid_branch(root, base)?;
                if !steps
                    .iter()
                    .any(|s| matches!(s, Step::Push {remote:r,branch:b} if r==remote && b==head))
                {
                    return Err("The pull request must use the exact branch and remote from the preceding push.".into());
                }
                seen_pr = true;
            }
        }
    }
    helpers(root)?;
    if operations::operation(root).is_some() {
        return Err(
            "Finish the Git operation already in progress, then request a new plan.".into(),
        );
    }
    let branch = operations::branch_ref(root);
    if branch.is_empty() {
        return Err("Check out a local branch before requesting an AI action.".into());
    }
    let head = operations::head(root);
    if head.is_empty() {
        return Err(
            "Create the repository's first commit in Branchline before using this connector."
                .into(),
        );
    }
    validate_oid(&head)?;
    let staged_token = staged_token(root)?;
    if seen_commit && staged_token.is_empty() {
        return Err("Stage the changes you want in Branchline, then ask the AI for a new commit plan. This connector never silently stages other files.".into());
    }
    let patch = if seen_commit {
        staged_patch(root)?
    } else {
        String::new()
    };
    if patch.len() > 200000 {
        return Err("This staged diff is too large for a reliable connector preview. Split the commit or review and commit it in Branchline first.".into());
    }
    let files = if seen_commit {
        txt(root, &["diff", "--cached", "--name-only", "-z"])?
            .split('\0')
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect()
    } else {
        Vec::new()
    };
    let mut warnings = Vec::new();
    if patch.contains("GIT binary patch") || patch.contains("Binary files ") {
        warnings.push("Binary contents cannot be read in this preview. Review those files separately before approving.".into())
    }
    let push = steps
        .iter()
        .find_map(|s| {
            if let Step::Push { remote, branch } = s {
                Some((remote, branch))
            } else {
                None
            }
        })
        .map(|(r, b)| push_preview(root, r, b, &head))
        .transpose()?;
    if push.is_some() {
        warnings.push("Publishing contacts the remote using your Git credentials. Existing outgoing commits are included. The remote will be checked again before sending.".into())
    }
    let pull_request = steps
        .iter()
        .find_map(|s| {
            if let Step::PullRequest { .. } = s {
                Some(s)
            } else {
                None
            }
        })
        .map(|step| github::preview(root, step, push.as_ref().ok_or("Missing push preview")?))
        .transpose()?;
    if pull_request.is_some() {
        warnings.push("The pull request title and description will be sent to GitHub using your GitHub CLI account.".into());
    }
    let mut risks = Vec::new();
    if let Some(target) = &push {
        let source = branch.trim_start_matches("refs/heads/");
        if matches!(
            target.branch.to_ascii_lowercase().as_str(),
            "main" | "master"
        ) {
            risks.push(RiskFlag{code:"primary_branch_target".into(),title:format!("Publishing to {}",target.branch),detail:format!("This updates {}/{} directly. Changes may affect everyone using this primary branch.",target.remote,target.branch)});
        }
        if source != target.branch {
            risks.push(RiskFlag{code:"branch_mismatch".into(),title:"Source and destination branches differ".into(),detail:format!("Commits from {source} will be published to {}/{}. This does not switch your local branch.",target.remote,target.branch)});
        }
    }
    let (git_dir, common_dir) = identity(root)?;
    let mut preview = Preview {
        root: root.to_string_lossy().into_owned(),
        git_dir,
        common_dir,
        head,
        branch,
        staged_token,
        patch,
        files,
        warnings,
        risks,
        push,
        pull_request,
        fingerprint: String::new(),
        command_workflow: None,
        command_details: Vec::new(),
    };
    preview.fingerprint = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&preview).map_err(|e| e.to_string())?)
    );
    Ok(preview)
}
pub fn execute(
    root: &Path,
    step: &Step,
    expected: &Preview,
    source_oid: &str,
    log: &mut CommandLog<'_>,
) -> Result<Value, String> {
    if let Step::Command { program, argv, stdin, timeout_seconds } = step {
        return commands::execute(root, &commands::CommandStep {
            program: program.clone(), argv: argv.clone(), stdin: stdin.clone(), timeout_seconds: *timeout_seconds,
        }, expected.command_workflow.as_ref().ok_or("Missing approved command manifest")?, log);
    }
    helpers(root)?;
    let (git_dir, common_dir) = identity(root)?;
    if git_dir != expected.git_dir
        || common_dir != expected.common_dir
        || operations::branch_ref(root) != expected.branch
        || operations::head(root) != source_oid
    {
        return Err("The repository or branch changed. Completed steps remain completed; request a new review for the remainder.".into());
    }
    match step {
        Step::Command { .. } => unreachable!(),
        Step::Commit { message } => {
            if staged_token(root)? != expected.staged_token || staged_patch(root)? != expected.patch
            {
                return Err("The staged changes changed after review. Nothing was committed; request a new plan.".into());
            }
            checked(
                root,
                &[
                    "-c",
                    "core.commentChar=#",
                    "commit",
                    "--cleanup=verbatim",
                    "-F",
                    "-",
                ],
                Some(message.as_bytes()),
                log,
            )?;
            let oid = operations::head(root);
            validate_oid(&oid)?;
            let parents = txt(root, &["show", "-s", "--format=%P", &oid])?;
            let actual = txt(
                root,
                &[
                    "diff",
                    "--binary",
                    "--full-index",
                    "--no-ext-diff",
                    "--no-textconv",
                    "--no-renames",
                    source_oid,
                    &oid,
                ],
            )?;
            let actual_message = txt(root, &["show", "-s", "--format=%B", &oid])?;
            if parents != source_oid
                || actual != expected.patch
                || actual_message.trim_end() != message.trim_end()
                || operations::branch_ref(root) != expected.branch
            {
                return Err("Git created a commit, but its result differs from the approved preview. Review HEAD in Branchline; publication was stopped.".into());
            }
            Ok(json!({"commitOid":oid,"message":message}))
        }
        Step::Push { remote, branch } => {
            let p = expected.push.as_ref().ok_or("Missing push preview")?;
            let current = push_preview(root, remote, branch, source_oid)?;
            if current.destination != p.destination || current.expected_oid != p.expected_oid {
                return Err("The push destination or cached remote branch changed. Review a new plan before publishing.".into());
            }
            let target = format!("refs/heads/{branch}");
            let observed = checked(
                root,
                &["ls-remote", "--refs", "--", remote, &target],
                None,
                log,
            )?;
            let observed = String::from_utf8_lossy(&observed)
                .split_whitespace()
                .next()
                .unwrap_or("")
                .to_string();
            if observed != p.expected_oid {
                return Err("The remote branch changed since the preview. Fetch in Branchline, then request a new plan; nothing was pushed.".into());
            }
            // The exact lease is combined with an ancestry check. It cannot authorize rewriting history.
            if !p.expected_oid.is_empty()
                && git(
                    root,
                    &["merge-base", "--is-ancestor", &p.expected_oid, source_oid],
                    None,
                )?
                .0 != 0
            {
                return Err("This push would rewrite remote history. It is not supported.".into());
            }
            let lease = format!("--force-with-lease={target}:{}", p.expected_oid);
            let spec = format!("{source_oid}:{target}");
            checked(
                root,
                &[
                    "-c",
                    "push.followTags=false",
                    "-c",
                    "push.gpgSign=false",
                    "push",
                    "--porcelain",
                    "--no-recurse-submodules",
                    &lease,
                    "--",
                    remote,
                    &spec,
                ],
                None,
                log,
            )?;
            let observed = checked(
                root,
                &["ls-remote", "--refs", "--", remote, &target],
                None,
                log,
            )?;
            if String::from_utf8_lossy(&observed).split_whitespace().next() != Some(source_oid) {
                return Err("The push was sent but its remote result could not be confirmed. Inspect the remote before retrying.".into());
            }
            Ok(
                json!({"publishedOid":source_oid,"remote":remote,"branch":branch,"createdBranch":p.expected_oid.is_empty()}),
            )
        }
        Step::PullRequest { .. } => github::execute(
            root,
            step,
            expected
                .pull_request
                .as_ref()
                .ok_or("Missing pull request preview")?,
            source_oid,
            log,
        ),
    }
}

/// Resolve a repository without status, filters, hooks, or external helper execution.
pub fn locate(path: &Path) -> Result<PathBuf, String> {
    let path = std::fs::canonicalize(path).map_err(|e| e.to_string())?;
    let root = txt(&path, &["rev-parse", "--show-toplevel"])?;
    std::fs::canonicalize(root).map_err(|e| e.to_string())
}
pub fn metadata(root: &Path) -> Result<Value, String> {
    let branch = operations::branch_ref(root);
    let head = operations::head(root);
    let files = txt(
        root,
        &[
            "diff",
            "--cached",
            "--name-only",
            "-z",
            "--no-ext-diff",
            "--no-textconv",
        ],
    )?;
    Ok(
        json!({"root":root,"name":root.file_name().unwrap_or_default().to_string_lossy(),"branch":branch.trim_start_matches("refs/heads/"),"head":head,"stagedToken":staged_token(root)?,"stagedFiles":files.split('\0').filter(|s|!s.is_empty()).collect::<Vec<_>>(),"remotes":operations::remotes(root)?,"notice":"Only staged-file metadata is listed. Review and stage working-tree changes in Branchline."}),
    )
}
pub fn history(root: &Path, skip: usize) -> Result<Value, String> {
    let mut service = Service {
        root: Some(root.to_path_buf()),
        watch: watch::RepoWatch::default(),
    };
    service.handle(Request::History { skip, all: false })
}
