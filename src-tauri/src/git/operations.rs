use super::*;
use std::collections::HashSet;

pub(super) fn head(root: &Path) -> String {
    git(root, &["rev-parse", "--verify", "HEAD"], None)
        .ok()
        .filter(|r| r.0 == 0)
        .map(|r| String::from_utf8_lossy(&r.1).trim().to_string())
        .unwrap_or_default()
}
pub(super) fn branch_ref(root: &Path) -> String {
    git(root, &["symbolic-ref", "--quiet", "HEAD"], None)
        .ok()
        .filter(|r| r.0 == 0)
        .map(|r| String::from_utf8_lossy(&r.1).trim().to_string())
        .unwrap_or_default()
}
pub(super) fn operation(root: &Path) -> Option<String> {
    for (path, name) in [
        ("rebase-merge", "rebase"),
        ("rebase-apply", "rebase"),
        ("MERGE_HEAD", "merge"),
        ("CHERRY_PICK_HEAD", "cherry-pick"),
        ("REVERT_HEAD", "revert"),
    ] {
        if let Ok(raw) = run(root, &["rev-parse", "--git-path", path], None) {
            let path = PathBuf::from(String::from_utf8_lossy(&raw).trim());
            if root.join(path).exists() {
                return Some(name.into());
            }
        }
    }
    None
}
fn idle(root: &Path) -> Result<(), String> {
    if let Some(op) = operation(root) {
        return Err(format!(
            "A {op} is already in progress. Finish or abort it before this operation."
        ));
    }
    Ok(())
}
fn clean(root: &Path) -> Result<(), String> {
    idle(root)?;
    if !files(root)?.is_empty() {
        return Err("Commit or stash your working changes before this operation. Untracked files must also be moved or added.".into());
    }
    Ok(())
}
pub(super) fn snapshot(root: &Path) -> String {
    format!("{}:{}", head(root), branch_ref(root))
}
fn same_head(root: &Path, expected: &str) -> Result<(), String> {
    if snapshot(root) != expected {
        return Err(
            "The branch or HEAD changed since this action was opened. Refresh and try again."
                .into(),
        );
    }
    Ok(())
}
pub(super) fn valid_branch(root: &Path, name: &str) -> Result<(), String> {
    if name.starts_with('-') || name == "HEAD" || name.is_empty() {
        return Err("Enter a valid branch name".into());
    }
    run(
        root,
        &["check-ref-format", &format!("refs/heads/{name}")],
        None,
    )
    .map(|_| ())
}
pub(super) fn remotes(root: &Path) -> Result<Vec<String>, String> {
    Ok(text(run(root, &["remote"], None)?)?
        .lines()
        .map(str::to_string)
        .collect())
}
pub(super) fn tracking(root: &Path) -> Value {
    let up = git(
        root,
        &[
            "rev-parse",
            "--abbrev-ref",
            "--symbolic-full-name",
            "@{upstream}",
        ],
        None,
    )
    .ok()
    .filter(|r| r.0 == 0)
    .map(|r| String::from_utf8_lossy(&r.1).trim().to_string());
    let counts = git(
        root,
        &["rev-list", "--left-right", "--count", "HEAD...@{upstream}"],
        None,
    )
    .ok()
    .filter(|r| r.0 == 0)
    .map(|r| {
        String::from_utf8_lossy(&r.1)
            .split_whitespace()
            .filter_map(|v| v.parse::<usize>().ok())
            .collect::<Vec<_>>()
    })
    .unwrap_or_default();
    let reference = branch_ref(root);
    let branch = reference.strip_prefix("refs/heads/").unwrap_or("");
    let config = |key: &str| {
        git(root, &["config", "--get", key], None)
            .ok()
            .filter(|r| r.0 == 0)
            .map(|r| String::from_utf8_lossy(&r.1).trim().to_string())
    };
    json!({"upstream":up,"ahead":counts.first().copied().unwrap_or(0),"behind":counts.get(1).copied().unwrap_or(0),"remote":config(&format!("branch.{branch}.remote")),"merge":config(&format!("branch.{branch}.merge")).map(|s|s.strip_prefix("refs/heads/").unwrap_or(&s).to_string())})
}
fn branches(root: &Path) -> Result<Value, String> {
    let raw = text(run(
        root,
        &[
            "for-each-ref",
            "--sort=refname",
            "--format=%(refname)%00%(objectname)%00%(upstream:short)%00%(symref)",
            "refs/heads/",
            "refs/remotes/",
        ],
        None,
    )?)?;
    let current = branch_ref(root);
    let rows:Vec<_>=raw.lines().filter_map(|line|{
        let fields:Vec<_>=line.split('\0').collect();
        if fields.len()!=4 || !fields[3].is_empty(){return None;}
        Some(json!({"reference":fields[0],"name":fields[0].strip_prefix("refs/heads/").or_else(||fields[0].strip_prefix("refs/remotes/")).unwrap_or(fields[0]),"oid":fields[1],"upstream":fields[2],"remote":fields[0].starts_with("refs/remotes/"),"current":fields[0]==current}))
    }).collect();
    Ok(json!(rows))
}
#[derive(Serialize)]
struct Plan {
    kind: String,
    head: String,
    branch: String,
    selected: Vec<String>,
    affected: Vec<String>,
    todo: String,
    base: Option<String>,
    message: String,
}
fn plan(root: &Path, kind: &str, oids: &[String]) -> Result<Plan, String> {
    clean(root)?;
    if branch_ref(root).is_empty() {
        return Err("Check out a local branch before rewriting history.".into());
    }
    if !matches!(kind, "squash" | "reword") {
        return Err("Unknown history action".into());
    }
    let selected: HashSet<_> = oids.iter().collect();
    if selected.len() != oids.len()
        || (kind == "squash" && oids.len() < 2)
        || (kind == "reword" && oids.len() != 1)
    {
        return Err("Select at least two commits to squash, or one commit to reword.".into());
    }
    for oid in oids {
        validate_oid(oid)?;
    }
    let chain = text(run(root, &["rev-list", "--first-parent", "HEAD"], None)?)?;
    let chain: Vec<_> = chain.lines().collect();
    let mut indices = Vec::new();
    for oid in oids {
        indices.push(
            chain
                .iter()
                .position(|c| c == oid)
                .ok_or("Selected commits must be on the current branch's first-parent history.")?,
        );
    }
    indices.sort_unstable();
    if indices.windows(2).any(|w| w[1] != w[0] + 1) {
        return Err("Select consecutive commits to squash (use Shift-click).".into());
    }
    let oldest = *indices.last().unwrap();
    let affected: Vec<String> = chain[..=oldest]
        .iter()
        .rev()
        .map(|s| s.to_string())
        .collect();
    for oid in &affected {
        let parents = text(run(root, &["rev-list", "--parents", "-n", "1", oid], None)?)?;
        if parents.split_whitespace().count() > 2 {
            return Err("This rewrite crosses a merge commit. Rewriting merge history is not supported yet.".into());
        }
        let published = run(
            root,
            &[
                "for-each-ref",
                "--format=%(refname)",
                &format!("--contains={oid}"),
                "refs/remotes/",
            ],
            None,
        )?;
        if !published.is_empty() {
            return Err("This rewrite includes commits present in fetched remote history. Only unpublished history can be rewritten in this version.".into());
        }
    }
    let ordered: Vec<String> = affected
        .iter()
        .filter(|oid| selected.contains(oid))
        .cloned()
        .collect();
    let todo = affected
        .iter()
        .map(|oid| {
            format!(
                "{} {}\n",
                if kind == "reword" && selected.contains(oid) {
                    "reword"
                } else if kind == "squash" && ordered.iter().skip(1).any(|s| s == oid) {
                    "squash"
                } else {
                    "pick"
                },
                oid
            )
        })
        .collect();
    let messages: Result<Vec<_>, _> = ordered
        .iter()
        .map(|oid| text(run(root, &["show", "-s", "--format=%B", oid], None)?))
        .collect();
    Ok(Plan {
        kind: kind.into(),
        head: head(root),
        branch: branch_ref(root),
        selected: ordered,
        affected,
        todo,
        base: chain.get(oldest + 1).map(|s| s.to_string()),
        message: messages?.join("\n"),
    })
}
fn plan_token(plan: &Plan) -> String {
    format!("{:x}", Sha256::digest(serde_json::to_vec(plan).unwrap()))
}
fn backup(root: &Path) -> Result<String, String> {
    let oid = head(root);
    validate_oid(&oid)?;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let reference = format!("refs/branchline-backups/{nanos}-{}", &oid[..8]);
    run(
        root,
        &[
            "update-ref",
            "-m",
            "Branchline: before history action",
            &reference,
            &oid,
            "",
        ],
        None,
    )?;
    Ok(reference)
}
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}
fn rewrite(root: &Path, plan: &Plan, message: &str) -> Result<String, String> {
    use std::os::unix::fs::PermissionsExt;
    if message.trim().is_empty() {
        return Err("Enter a commit message".into());
    }
    let temp = tempfile::tempdir().map_err(|e| e.to_string())?;
    let editor = temp.path().join("editor.sh");
    let todo = temp.path().join("todo");
    let msg = temp.path().join("message");
    std::fs::write(&editor, "#!/bin/sh\n/bin/cat \"$1\" > \"$2\"\n").map_err(|e| e.to_string())?;
    std::fs::set_permissions(&editor, std::fs::Permissions::from_mode(0o700))
        .map_err(|e| e.to_string())?;
    std::fs::write(&todo, &plan.todo).map_err(|e| e.to_string())?;
    std::fs::write(&msg, message).map_err(|e| e.to_string())?;
    let seq = format!(
        "{} {}",
        shell_quote(&editor.to_string_lossy()),
        shell_quote(&todo.to_string_lossy())
    );
    let edit = format!(
        "{} {}",
        shell_quote(&editor.to_string_lossy()),
        shell_quote(&msg.to_string_lossy())
    );
    let reference = backup(root)?;
    let mut args = vec![
        "-c",
        "rebase.updateRefs=false",
        "-c",
        "rebase.autoSquash=false",
        "-c",
        "rebase.autoStash=false",
        "rebase",
        "-i",
        "--force-rebase",
        "--keep-empty",
        "--empty=keep",
        "--no-update-refs",
        "--no-autosquash",
        "--no-autostash",
    ];
    if let Some(base) = &plan.base {
        args.push(base);
    } else {
        args.push("--root");
    }
    let result = git_env(
        root,
        &args,
        None,
        &[("GIT_SEQUENCE_EDITOR", &seq), ("GIT_EDITOR", &edit)],
    );
    let failure = match result {
        Ok((0, _, _)) => None,
        Ok((_, _, err)) => Some(err),
        Err(err) => Some(err),
    };
    if let Some(err) = failure {
        if operation(root).as_deref() == Some("rebase") {
            let _ = run(root, &["rebase", "--abort"], None);
        }
        let restored =
            head(root) == plan.head && branch_ref(root) == plan.branch && operation(root).is_none();
        return Err(format!(
            "History rewrite failed. {} Backup: {reference}\n{err}",
            if restored {
                "The original branch was restored."
            } else {
                "Automatic abort did not complete; use Abort in the operation banner or inspect Git state."
            }
        ));
    }
    Ok(reference)
}
impl Service {
    pub(super) fn operation(&mut self, request: Request) -> Result<Value, String> {
        let root = self.root.as_ref().ok_or("Open a repository first")?.clone();
        match request {
            Request::Branches { expected_root } => {
                if expected_root
                    .as_deref()
                    .is_some_and(|expected| Path::new(expected) != root)
                {
                    return Err(
                        "The active repository changed before branches could be read.".into(),
                    );
                }
                branches(&root)
            }
            Request::SwitchBranch {
                reference,
                create,
                expected_head,
            } => {
                idle(&root)?;
                same_head(&root, &expected_head)?;
                let available = branches(&root)?;
                let found = available
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|b| b["reference"] == reference);
                let mut args = vec!["switch", "--no-guess"];
                if let Some(name) = &create {
                    valid_branch(&root, name)?;
                    if !found {
                        validate_oid(&reference)?;
                        run(
                            &root,
                            &["cat-file", "-e", &format!("{reference}^{{commit}}")],
                            None,
                        )?;
                    }
                    if reference.starts_with("refs/remotes/") {
                        args.push("--track");
                    }
                    args.extend(["-c", name, &reference]);
                } else {
                    if !found || !reference.starts_with("refs/heads/") {
                        return Err("Select a local branch or create a tracking branch from a remote branch.".into());
                    }
                    args.push(reference.strip_prefix("refs/heads/").unwrap());
                }
                run(&root, &args, None)?;
                self.status()
            }
            Request::BranchAction {
                kind,
                reference,
                expected_oid,
                name,
                remote,
                branch,
                expected_head,
            } => {
                idle(&root)?;
                same_head(&root, &expected_head)?;
                validate_oid(&expected_oid)?;
                let local = reference
                    .strip_prefix("refs/heads/")
                    .ok_or("Select a local branch for this action")?;
                valid_branch(&root, local)?;
                let actual = text(run(&root, &["rev-parse", "--verify", &reference], None)?)?;
                if actual.trim() != expected_oid {
                    return Err(
                        "The selected branch changed. Reopen Branches and review it again.".into(),
                    );
                }
                match kind.as_str() {
                    "rename" => {
                        valid_branch(&root, &name)?;
                        let worktree = text(run(
                            &root,
                            &["for-each-ref", "--format=%(worktreepath)", &reference],
                            None,
                        )?)?;
                        if !worktree.trim().is_empty()
                            && std::fs::canonicalize(worktree.trim()).ok().as_ref() != Some(&root)
                        {
                            return Err(
                                "This branch is open in another worktree. Rename it there.".into(),
                            );
                        }
                        run(&root, &["branch", "-m", local, &name], None)?;
                    }
                    "delete" => {
                        if branch_ref(&root) == reference {
                            return Err("Switch away from this branch before deleting it.".into());
                        }
                        // -d refuses unmerged work and branches used by a worktree; never force-delete.
                        run(&root, &["branch", "-d", "--", local], None)?;
                    }
                    "push" => {
                        if !remotes(&root)?.contains(&remote) {
                            return Err("Select a configured remote".into());
                        }
                        valid_branch(&root, &branch)?;
                        run(
                            &root,
                            &[
                                "-c",
                                "push.followTags=false",
                                "push",
                                "--porcelain",
                                "--no-recurse-submodules",
                                "--",
                                &remote,
                                &format!("{expected_oid}:refs/heads/{branch}"),
                            ],
                            None,
                        )?;
                    }
                    _ => return Err("Unknown branch action".into()),
                }
                self.status()
            }
            Request::Network {
                kind,
                remote,
                branch,
                expected_head,
            } => {
                if !remotes(&root)?.contains(&remote) {
                    return Err("Select a configured remote".into());
                }
                if kind != "fetch" {
                    idle(&root)?;
                    same_head(&root, &expected_head)?;
                    valid_branch(&root, &branch)?;
                    if branch_ref(&root).is_empty() {
                        return Err("Check out a local branch first".into());
                    }
                }
                match kind.as_str() {
                    "fetch" => {
                        run(
                            &root,
                            &["fetch", "--no-recurse-submodules", "--", &remote],
                            None,
                        )?;
                    }
                    "pull" => {
                        clean(&root)?;
                        run(
                            &root,
                            &[
                                "pull",
                                "--ff-only",
                                "--no-rebase",
                                "--no-autostash",
                                "--no-recurse-submodules",
                                "--",
                                &remote,
                                &branch,
                            ],
                            None,
                        )?;
                    }
                    "push" => {
                        run(
                            &root,
                            &[
                                "-c",
                                "push.followTags=false",
                                "push",
                                "--porcelain",
                                "--set-upstream",
                                "--",
                                &remote,
                                &format!("HEAD:refs/heads/{branch}"),
                            ],
                            None,
                        )?;
                    }
                    _ => return Err("Unknown network operation".into()),
                }
                self.status()
            }
            Request::RewritePreview { kind, oids } => {
                let p = plan(&root, &kind, &oids)?;
                Ok(json!({"token":plan_token(&p),"plan":p}))
            }
            Request::Rewrite {
                kind,
                oids,
                message,
                expected,
            } => {
                let p = plan(&root, &kind, &oids)?;
                if plan_token(&p) != expected {
                    return Err(
                        "The history plan changed. Close this dialog and review the action again."
                            .into(),
                    );
                }
                let reference = rewrite(&root, &p, &message)?;
                let mut status = self.status()?;
                status["operationResult"] =
                    json!("History updated. Original commits saved locally.");
                status["backupRef"] = json!(reference);
                Ok(status)
            }
            Request::CommitAction {
                kind,
                oid,
                expected_head,
            } => {
                validate_oid(&oid)?;
                clean(&root)?;
                same_head(&root, &expected_head)?;
                if branch_ref(&root).is_empty() {
                    return Err("Check out a local branch first".into());
                }
                if !matches!(kind.as_str(), "cherry-pick" | "revert") {
                    return Err("Unknown commit action".into());
                }
                let reference = backup(&root)?;
                let result = run(&root, &[&kind, "--no-edit", &oid], None);
                if let Err(err) = result {
                    let abort = if operation(&root).is_some() {
                        run(&root, &[&kind, "--abort"], None)
                    } else {
                        Ok(vec![])
                    };
                    return Err(format!(
                        "{kind} failed. {} Backup: {reference}\n{err}",
                        if abort.is_ok() {
                            "Original state preserved."
                        } else {
                            "Abort did not complete. Inspect the operation banner."
                        }
                    ));
                }
                let mut status = self.status()?;
                status["operationResult"] =
                    json!(format!("{kind} completed. Original commits saved locally."));
                status["backupRef"] = json!(reference);
                Ok(status)
            }
            Request::AbortOperation { kind } => {
                if operation(&root).as_deref() != Some(&kind)
                    || !matches!(kind.as_str(), "rebase" | "merge" | "cherry-pick" | "revert")
                {
                    return Err("Operation changed. Refresh before aborting.".into());
                }
                run(&root, &[&kind, "--abort"], None)?;
                self.status()
            }
            _ => Err("Unsupported repository operation".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    fn fixture() -> (tempfile::TempDir, Service) {
        let dir = tempfile::tempdir().unwrap();
        run(dir.path(), &["init", "-b", "main"], None).unwrap();
        for (key, value) in [
            ("user.name", "Operations Test"),
            ("user.email", "test@example.invalid"),
            ("commit.gpgsign", "false"),
        ] {
            run(dir.path(), &["config", key, value], None).unwrap();
        }
        let mut s = Service::default();
        s.handle(Request::Open {
            path: dir.path().to_string_lossy().into_owned(),
        })
        .unwrap();
        (dir, s)
    }
    fn branch_action(
        service: &mut Service,
        kind: &str,
        reference: &str,
        oid: &str,
        name: &str,
        remote: &str,
        destination: &str,
    ) -> Result<Value, String> {
        let expected_head = snapshot(service.root.as_ref().unwrap());
        service.handle(Request::BranchAction {
            kind: kind.into(),
            reference: reference.into(),
            expected_oid: oid.into(),
            name: name.into(),
            remote: remote.into(),
            branch: destination.into(),
            expected_head,
        })
    }
    #[test]
    fn selected_branch_push_rename_and_stale_checks_preserve_current_work() {
        let (dir, mut service) = fixture();
        let root = dir.path();
        let initial = commit_file(root, "a", "before", "Initial");
        run(root, &["branch", "feature"], None).unwrap();
        let current = commit_file(root, "a", "after", "Main change");
        let bare = tempfile::tempdir().unwrap();
        run(bare.path(), &["init", "--bare"], None).unwrap();
        run(
            root,
            &["remote", "add", "origin", bare.path().to_str().unwrap()],
            None,
        )
        .unwrap();
        branch_action(
            &mut service,
            "push",
            "refs/heads/feature",
            &initial,
            "",
            "origin",
            "feat/test",
        )
        .unwrap();
        assert_eq!(head(root), current);
        assert_eq!(branch_ref(root), "refs/heads/main");
        assert_eq!(
            text(run(bare.path(), &["rev-parse", "feat/test"], None).unwrap())
                .unwrap()
                .trim(),
            initial
        );
        run(root, &["update-ref", "refs/heads/feature", &current], None).unwrap();
        assert!(branch_action(
            &mut service,
            "rename",
            "refs/heads/feature",
            &initial,
            "renamed",
            "",
            ""
        )
        .is_err());
        assert!(branch_action(
            &mut service,
            "rename",
            "refs/heads/main",
            &current,
            "feature",
            "",
            ""
        )
        .is_err());
        fs::write(root.join("a"), "Uncommitted work").unwrap();
        branch_action(
            &mut service,
            "rename",
            "refs/heads/main",
            &current,
            "renamed-main",
            "",
            "",
        )
        .unwrap();
        assert_eq!(branch_ref(root), "refs/heads/renamed-main");
        assert_eq!(
            fs::read_to_string(root.join("a")).unwrap(),
            "Uncommitted work"
        );
    }
    #[test]
    fn local_branch_delete_refuses_current_and_unmerged_branches() {
        let (dir, mut service) = fixture();
        let root = dir.path();
        let initial = commit_file(root, "a", "a", "Initial");
        assert!(branch_action(
            &mut service,
            "delete",
            "refs/heads/main",
            &initial,
            "",
            "",
            ""
        )
        .is_err());
        run(root, &["switch", "-c", "unmerged"], None).unwrap();
        let tip = commit_file(root, "b", "b", "Unmerged");
        run(root, &["switch", "main"], None).unwrap();
        assert!(branch_action(
            &mut service,
            "delete",
            "refs/heads/unmerged",
            &tip,
            "",
            "",
            ""
        )
        .is_err());
        assert!(run(root, &["rev-parse", "refs/heads/unmerged"], None).is_ok());
        run(root, &["branch", "merged"], None).unwrap();
        branch_action(
            &mut service,
            "delete",
            "refs/heads/merged",
            &initial,
            "",
            "",
            "",
        )
        .unwrap();
        assert!(run(root, &["rev-parse", "--verify", "refs/heads/merged"], None).is_err());
    }
    #[test]
    fn branch_snapshot_is_bound_to_the_requested_repository() {
        let (dir, mut service) = fixture();
        commit_file(dir.path(), "a", "a", "Initial");
        run(dir.path(), &["branch", "feature"], None).unwrap();
        let root = service.root.as_ref().unwrap().to_string_lossy().to_string();
        let rows = service
            .handle(Request::Branches {
                expected_root: Some(root),
            })
            .unwrap();
        let rows = rows.as_array().unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows.iter().filter(|b| b["current"] == true).count(), 1);
        assert!(service
            .handle(Request::Branches {
                expected_root: Some("/different/repository".into())
            })
            .is_err());
    }
    fn commit_file(root: &Path, name: &str, content: &str, message: &str) -> String {
        fs::write(root.join(name), content).unwrap();
        run(root, &["add", "--", name], None).unwrap();
        run(root, &["commit", "-m", message], None).unwrap();
        head(root)
    }
    fn preview(s: &mut Service, kind: &str, oids: Vec<String>) -> Value {
        s.handle(Request::RewritePreview {
            kind: kind.into(),
            oids,
        })
        .unwrap()
    }
    fn apply(
        s: &mut Service,
        kind: &str,
        oids: Vec<String>,
        message: &str,
        p: &Value,
    ) -> Result<Value, String> {
        s.handle(Request::Rewrite {
            kind: kind.into(),
            oids,
            message: message.into(),
            expected: p["token"].as_str().unwrap().into(),
        })
    }
    #[test]
    fn squash_middle_range_replays_descendants_preserves_tree_and_backup() {
        let (d, mut s) = fixture();
        let root = d.path();
        commit_file(root, "base", "a", "Base");
        let a = commit_file(root, "a", "a", "First");
        let b = commit_file(root, "b", "b", "Second");
        let tip = commit_file(root, "c", "c", "Later");
        let tree = run(root, &["rev-parse", "HEAD^{tree}"], None).unwrap();
        let oids = vec![b, a];
        let p = preview(&mut s, "squash", oids.clone());
        assert_eq!(p["plan"]["affected"].as_array().unwrap().len(), 3);
        let status = apply(&mut s, "squash", oids, "Combined", &p).unwrap();
        assert!(status["backupRef"]
            .as_str()
            .unwrap()
            .contains("refs/branchline-backups/"));
        assert_eq!(
            tree,
            run(root, &["rev-parse", "HEAD^{tree}"], None).unwrap()
        );
        assert_ne!(head(root), tip);
        let log = text(run(root, &["log", "--format=%s"], None).unwrap()).unwrap();
        assert_eq!(
            log.lines().collect::<Vec<_>>(),
            vec!["Later", "Combined", "Base"]
        );
        let refs = text(
            run(
                root,
                &[
                    "for-each-ref",
                    "--format=%(objectname)",
                    "refs/branchline-backups/",
                ],
                None,
            )
            .unwrap(),
        )
        .unwrap();
        assert!(refs.contains(&tip));
    }
    #[test]
    fn squash_root_and_reword_older_commit() {
        let (d, mut s) = fixture();
        let root = d.path();
        let a = commit_file(root, "a", "a", "A");
        let b = commit_file(root, "b", "b", "B");
        commit_file(root, "c", "c", "C");
        let oids = vec![a, b];
        let p = preview(&mut s, "squash", oids.clone());
        apply(&mut s, "squash", oids, "Root combined", &p).unwrap();
        let older = text(run(root, &["rev-parse", "HEAD~1"], None).unwrap())
            .unwrap()
            .trim()
            .to_string();
        let p = preview(&mut s, "reword", vec![older.clone()]);
        apply(&mut s, "reword", vec![older], "New root message", &p).unwrap();
        assert_eq!(
            text(run(root, &["log", "--format=%s"], None).unwrap())
                .unwrap()
                .lines()
                .collect::<Vec<_>>(),
            vec!["C", "New root message"]
        );
    }
    #[test]
    fn rewrite_rejects_dirty_published_nonconsecutive_and_stale_plans() {
        let (d, mut s) = fixture();
        let root = d.path();
        let a = commit_file(root, "a", "a", "A");
        let b = commit_file(root, "b", "b", "B");
        let c = commit_file(root, "c", "c", "C");
        assert!(s
            .handle(Request::RewritePreview {
                kind: "squash".into(),
                oids: vec![a.clone(), c.clone()]
            })
            .unwrap_err()
            .contains("consecutive"));
        fs::write(root.join("untracked"), "x").unwrap();
        assert!(s
            .handle(Request::RewritePreview {
                kind: "reword".into(),
                oids: vec![b.clone()]
            })
            .is_err());
        fs::remove_file(root.join("untracked")).unwrap();
        let p = preview(&mut s, "reword", vec![b.clone()]);
        commit_file(root, "d", "d", "D");
        assert!(apply(&mut s, "reword", vec![b.clone()], "Changed", &p)
            .unwrap_err()
            .contains("plan changed"));
        run(root, &["update-ref", "refs/remotes/origin/main", &c], None).unwrap();
        assert!(s
            .handle(Request::RewritePreview {
                kind: "reword".into(),
                oids: vec![b]
            })
            .unwrap_err()
            .contains("remote history"));
        assert_eq!(
            text(run(root, &["log", "-1", "--format=%s"], None).unwrap())
                .unwrap()
                .trim(),
            "D"
        );
    }
    #[test]
    fn branches_switch_create_track_and_reject_conflicting_worktree() {
        let (d, mut s) = fixture();
        let root = d.path();
        let first = commit_file(root, "a", "base", "Base");
        s.handle(Request::SwitchBranch {
            reference: first.clone(),
            create: Some("feature/test".into()),
            expected_head: snapshot(root),
        })
        .unwrap();
        commit_file(root, "a", "feature", "Feature");
        s.handle(Request::SwitchBranch {
            reference: "refs/heads/main".into(),
            create: None,
            expected_head: snapshot(root),
        })
        .unwrap();
        fs::write(root.join("a"), "dirty").unwrap();
        assert!(s
            .handle(Request::SwitchBranch {
                reference: "refs/heads/feature/test".into(),
                create: None,
                expected_head: snapshot(root)
            })
            .is_err());
        assert_eq!(fs::read_to_string(root.join("a")).unwrap(), "dirty");
        assert!(branches(root)
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .any(|b| b["name"] == "feature/test"));
        assert!(s
            .handle(Request::SwitchBranch {
                reference: first,
                create: Some("--bad".into()),
                expected_head: snapshot(root)
            })
            .is_err());
    }
    #[test]
    fn fetch_pull_push_and_remote_tracking_use_real_bare_remote() {
        let (d, mut s) = fixture();
        let root = d.path();
        commit_file(root, "a", "a", "Initial");
        let remote = tempfile::tempdir().unwrap();
        run(remote.path(), &["init", "--bare", "-b", "main"], None).unwrap();
        run(
            root,
            &["remote", "add", "origin", remote.path().to_str().unwrap()],
            None,
        )
        .unwrap();
        s.handle(Request::Network {
            kind: "push".into(),
            remote: "origin".into(),
            branch: "main".into(),
            expected_head: snapshot(root),
        })
        .unwrap();
        assert_eq!(tracking(root)["upstream"], "origin/main");
        let other = tempfile::tempdir().unwrap();
        run(
            other.path(),
            &["clone", remote.path().to_str().unwrap(), "."],
            None,
        )
        .unwrap();
        for (k, v) in [
            ("user.name", "Remote Test"),
            ("user.email", "remote@example.invalid"),
            ("commit.gpgsign", "false"),
        ] {
            run(other.path(), &["config", k, v], None).unwrap();
        }
        let remote_tip = commit_file(other.path(), "b", "b", "Remote change");
        run(other.path(), &["push"], None).unwrap();
        s.handle(Request::Network {
            kind: "fetch".into(),
            remote: "origin".into(),
            branch: String::new(),
            expected_head: snapshot(root),
        })
        .unwrap();
        assert_eq!(tracking(root)["behind"], 1);
        s.handle(Request::Network {
            kind: "pull".into(),
            remote: "origin".into(),
            branch: "main".into(),
            expected_head: snapshot(root),
        })
        .unwrap();
        assert_eq!(head(root), remote_tip);
        run(
            root,
            &["update-ref", "refs/remotes/origin/topic", &remote_tip],
            None,
        )
        .unwrap();
        s.handle(Request::SwitchBranch {
            reference: "refs/remotes/origin/topic".into(),
            create: Some("topic".into()),
            expected_head: snapshot(root),
        })
        .unwrap();
        assert_eq!(tracking(root)["upstream"], "origin/topic");
    }
    #[test]
    fn diverged_pull_and_push_do_not_rewrite_or_merge() {
        let (d, mut s) = fixture();
        let root = d.path();
        commit_file(root, "a", "a", "Base");
        let remote = tempfile::tempdir().unwrap();
        run(remote.path(), &["init", "--bare", "-b", "main"], None).unwrap();
        run(
            root,
            &["remote", "add", "origin", remote.path().to_str().unwrap()],
            None,
        )
        .unwrap();
        run(root, &["push", "-u", "origin", "main"], None).unwrap();
        let other = tempfile::tempdir().unwrap();
        run(
            other.path(),
            &["clone", remote.path().to_str().unwrap(), "."],
            None,
        )
        .unwrap();
        for (k, v) in [
            ("user.name", "Other"),
            ("user.email", "other@example.invalid"),
            ("commit.gpgsign", "false"),
        ] {
            run(other.path(), &["config", k, v], None).unwrap();
        }
        commit_file(other.path(), "remote", "remote", "Remote");
        run(other.path(), &["push"], None).unwrap();
        let local = commit_file(root, "local", "local", "Local");
        assert!(s
            .handle(Request::Network {
                kind: "pull".into(),
                remote: "origin".into(),
                branch: "main".into(),
                expected_head: snapshot(root)
            })
            .is_err());
        assert_eq!(head(root), local);
        assert!(operation(root).is_none());
        assert!(s
            .handle(Request::Network {
                kind: "push".into(),
                remote: "origin".into(),
                branch: "main".into(),
                expected_head: snapshot(root)
            })
            .is_err());
        assert_eq!(head(root), local);
    }
    #[test]
    fn cherry_pick_revert_and_conflict_restore() {
        let (d, mut s) = fixture();
        let root = d.path();
        commit_file(root, "a", "base", "Base");
        run(root, &["switch", "-c", "feature"], None).unwrap();
        let feature = commit_file(root, "a", "feature", "Feature");
        run(root, &["switch", "main"], None).unwrap();
        s.handle(Request::CommitAction {
            kind: "cherry-pick".into(),
            oid: feature.clone(),
            expected_head: snapshot(root),
        })
        .unwrap();
        assert_eq!(fs::read_to_string(root.join("a")).unwrap(), "feature");
        let applied = head(root);
        s.handle(Request::CommitAction {
            kind: "revert".into(),
            oid: applied.clone(),
            expected_head: snapshot(root),
        })
        .unwrap();
        assert_eq!(fs::read_to_string(root.join("a")).unwrap(), "base");
        let local = commit_file(root, "a", "conflict", "Local");
        assert!(s
            .handle(Request::CommitAction {
                kind: "cherry-pick".into(),
                oid: feature,
                expected_head: snapshot(root)
            })
            .unwrap_err()
            .contains("Original state preserved"));
        assert_eq!(head(root), local);
        assert_eq!(fs::read_to_string(root.join("a")).unwrap(), "conflict");
        assert!(operation(root).is_none());
    }
    #[test]
    fn merge_rewrite_is_rejected_and_all_branch_log_contains_parents() {
        let (d, mut s) = fixture();
        let root = d.path();
        commit_file(root, "base", "a", "Base");
        run(root, &["switch", "-c", "topic"], None).unwrap();
        let topic = commit_file(root, "topic", "a", "Topic");
        run(root, &["switch", "main"], None).unwrap();
        commit_file(root, "main", "a", "Main");
        run(
            root,
            &["merge", "--no-ff", "topic", "-m", "Merge topic"],
            None,
        )
        .unwrap();
        assert!(s
            .handle(Request::RewritePreview {
                kind: "reword".into(),
                oids: vec![head(root)]
            })
            .unwrap_err()
            .contains("merge commit"));
        let history = s.handle(Request::History { skip: 0, all: true }).unwrap();
        assert_eq!(history[0]["parents"].as_array().unwrap().len(), 2);
        assert!(history
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["oid"] == topic));
    }
    #[test]
    fn stale_branch_snapshot_and_failed_rebase_hook_preserve_state() {
        use std::os::unix::fs::PermissionsExt;
        let (d, mut service) = fixture();
        let root = d.path();
        commit_file(root, "a", "base", "Base");
        let expected = snapshot(root);
        run(root, &["switch", "-c", "other"], None).unwrap();
        assert!(service
            .handle(Request::SwitchBranch {
                reference: "refs/heads/main".into(),
                create: None,
                expected_head: expected
            })
            .unwrap_err()
            .contains("branch or HEAD changed"));
        let a = commit_file(root, "b", "one", "One");
        let b = commit_file(root, "c", "two", "Two");
        let oids = vec![a, b];
        let p = preview(&mut service, "squash", oids.clone());
        let old = head(root);
        let hook = root.join(".git/hooks/pre-rebase");
        fs::write(&hook, "#!/bin/sh\necho 'Reject rebase' >&2\nexit 1\n").unwrap();
        fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(apply(&mut service, "squash", oids, "Combined", &p)
            .unwrap_err()
            .contains("original branch was restored"));
        assert_eq!(head(root), old);
        assert!(files(root).unwrap().is_empty());
        assert!(operation(root).is_none());
    }
}
