mod objects;
mod selected_files;
mod review;
mod save;
mod snapshot;
mod unstaged;
#[cfg(test)]
mod tests;
use super::*;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Request {
    List {
        expected_root: String,
    },
    Preview {
        expected_root: String,
        scope: String,
        paths: Vec<String>,
        include_untracked: bool,
    },
    Save {
        expected_root: String,
        scope: String,
        paths: Vec<String>,
        include_untracked: bool,
        message: String,
        expected: String,
    },
    Files {
        expected_root: String,
        oid: String,
        staged: bool,
    },
    Diff {
        expected_root: String,
        oid: String,
        path: String,
        staged: bool,
    },
    Image {
        expected_root: String,
        oid: String,
        path: String,
        staged: bool,
    },
    RestorePreview {
        expected_root: String,
        reference: String,
        oid: String,
    },
    Restore {
        expected_root: String,
        reference: String,
        oid: String,
        pop: bool,
        restore_index: bool,
        expected: String,
    },
    Drop {
        expected_root: String,
        reference: String,
        oid: String,
    },
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Entry {
    reference: String,
    oid: String,
    message: String,
    branch: String,
    timestamp: u64,
}

fn entries(root: &Path) -> Result<Vec<Entry>, String> {
    let exists = git(
        root,
        &["show-ref", "--verify", "--quiet", "refs/stash"],
        None,
    )?;
    if exists.0 == 1 {
        return Ok(Vec::new());
    }
    if exists.0 != 0 {
        return Err(exists.2);
    }
    let raw = text(run(
        root,
        &[
            "reflog",
            "show",
            "--format=%H%x00%gs%x00%ct%x00",
            "refs/stash",
        ],
        None,
    )?)?;
    let fields = raw.split('\0').collect::<Vec<_>>();
    fields
        .chunks(3)
        .filter(|fields| fields.len() == 3)
        .enumerate()
        .map(|(index, fields)| {
            let oid = fields[0].trim().to_string();
            validate_oid(&oid)?;
            let subject = fields[1];
            let label = subject
                .strip_prefix("On ")
                .or_else(|| subject.strip_prefix("WIP on "));
            let (branch, message) = label
                .and_then(|label| label.split_once(": "))
                .unwrap_or(("", subject));
            Ok(Entry {
                reference: format!("stash@{{{index}}}"),
                oid,
                message: message.into(),
                branch: branch.into(),
                timestamp: fields[2].parse().map_err(|_| "Invalid stash timestamp")?,
            })
        })
        .collect()
}

fn selected(root: &Path, reference: &str, oid: &str) -> Result<Entry, String> {
    validate_oid(oid)?;
    entries(root)?
        .into_iter()
        .find(|entry| entry.reference == reference && entry.oid == oid)
        .ok_or("The stash list changed. Refresh and select the stash again.".into())
}

fn backup(root: &Path, oid: &str) -> Result<String, String> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let reference = format!("refs/branchline-backups/stash-{stamp}");
    run(
        root,
        &["update-ref", &reference, oid, &"0".repeat(oid.len())],
        None,
    )?;
    Ok(reference)
}

fn ready(root: &Path) -> Result<(), String> {
    if operations::operation(root).is_some()
        || !run(root, &["ls-files", "--unmerged", "-z"], None)?.is_empty()
    {
        return Err(
            "Finish the current Git operation and resolve conflicts before using a stash.".into(),
        );
    }
    if operations::head(root).is_empty() {
        return Err("Create an initial commit before using stashes.".into());
    }
    Ok(())
}

pub(crate) fn handle(root: &Path, request: Request) -> Result<Value, String> {
    let expected_root = match &request {
        Request::List { expected_root }
        | Request::Preview { expected_root, .. }
        | Request::Save { expected_root, .. }
        | Request::Files { expected_root, .. }
        | Request::Diff { expected_root, .. }
        | Request::Image { expected_root, .. }
        | Request::RestorePreview { expected_root, .. }
        | Request::Restore { expected_root, .. }
        | Request::Drop { expected_root, .. } => expected_root,
    };
    if Path::new(expected_root) != root {
        return Err("The repository changed. Open Stashes again.".into());
    }
    match request {
        Request::List { .. } => Ok(json!(entries(root)?)),
        Request::Preview {
            scope,
            paths,
            include_untracked,
            ..
        } => save::preview(root, &scope, &paths, include_untracked),
        Request::Save {
            scope,
            paths,
            include_untracked,
            message,
            expected,
            ..
        } => save::save(root, &scope, &paths, include_untracked, &message, &expected),
        Request::Files { oid, staged, .. } => review::files(root, &oid, staged),
        Request::Diff {
            oid, path, staged, ..
        } => review::diff(root, &oid, &path, staged),
        Request::Image {
            oid, path, staged, ..
        } => review::image(root, &oid, &path, staged),
        Request::RestorePreview { reference, oid, .. } => {
            ready(root)?;
            selected(root, &reference, &oid)?;
            Ok(json!({"token":snapshot::token(root)?,"dirty":!files(root)?.is_empty()}))
        }
        Request::Restore {
            reference,
            oid,
            pop,
            restore_index,
            expected,
            ..
        } => {
            ready(root)?;
            selected(root, &reference, &oid)?;
            if snapshot::token(root)? != expected {
                return Err("Your files or branch changed. Review the restore again.".into());
            }
            let recovery = backup(root, &oid)?;
            let before_list = entries(root)?
                .into_iter()
                .map(|entry| (entry.oid, entry.message))
                .collect::<Vec<_>>();
            let mut args = vec!["-c", "stash.index=false", "stash", "apply"];
            if restore_index {
                args.push("--index");
            }
            args.push(&oid);
            let (code, out, err) = match git(root, &args, None) {
                Ok(result) => result,
                Err(error) => {
                    return Ok(
                        json!({"ok":false,"outcome":"restore_failed","message":format!("Restore stopped: {error}. Inspect your files before retrying. The stash was kept."),"recoveryRef":recovery}),
                    )
                }
            };
            let conflicts = files(root)?
                .into_iter()
                .filter(|file| {
                    file.index == 'U'
                        || file.worktree == 'U'
                        || file.index == 'A' && file.worktree == 'A'
                })
                .map(|file| file.path)
                .collect::<Vec<_>>();
            if code != 0 {
                return Ok(
                    json!({"ok":false,"outcome":"restore_failed","message":format!("The restore did not complete. The stash was kept. {}",if err.trim().is_empty(){String::from_utf8_lossy(&out).trim().to_string()}else{err.trim().to_string()}),"conflicts":conflicts,"recoveryRef":recovery}),
                );
            }
            if pop {
                let after_list = entries(root)?
                    .into_iter()
                    .map(|entry| (entry.oid, entry.message))
                    .collect::<Vec<_>>();
                if before_list != after_list || selected(root, &reference, &oid).is_err() {
                    return Ok(
                        json!({"ok":true,"outcome":"applied_kept","message":"Changes restored. The stash list changed, so the stash was kept.","recoveryRef":recovery}),
                    );
                }
                if let Err(error) = run(root, &["stash", "drop", &reference], None) {
                    return Ok(
                        json!({"ok":true,"outcome":"applied_kept","message":format!("Changes restored, but the stash could not be removed: {error}"),"recoveryRef":recovery}),
                    );
                }
            }
            Ok(
                json!({"ok":true,"outcome":if pop {"popped"}else{"applied"},"message":if pop {"Changes restored and stash removed."}else{"Changes restored. The stash was kept."},"recoveryRef":recovery}),
            )
        }
        Request::Drop { reference, oid, .. } => {
            selected(root, &reference, &oid)?;
            let recovery = backup(root, &oid)?;
            selected(root, &reference, &oid)?;
            if let Err(error) = run(root, &["stash", "drop", &reference], None) {
                return Ok(
                    json!({"ok":false,"outcome":"delete_failed","message":error,"recoveryRef":recovery}),
                );
            }
            Ok(
                json!({"ok":true,"outcome":"deleted","message":"Stash removed from the list. A recovery copy is kept locally.","recoveryRef":recovery}),
            )
        }
    }
}
