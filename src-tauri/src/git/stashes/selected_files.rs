use super::objects::{commit, index, input};
use super::*;
use std::collections::BTreeSet;

pub(super) fn save(
    root: &Path,
    files: &[File],
    message: &str,
    expected: &str,
) -> Result<Value, String> {
    let tracked = files
        .iter()
        .filter(|file| file.index != '?')
        .flat_map(|file| std::iter::once(file.path.clone()).chain(file.old_path.clone()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let untracked = files
        .iter()
        .filter(|file| file.index == '?')
        .map(|file| file.path.clone())
        .collect::<Vec<_>>();
    let head = operations::head(root);
    let temp = tempfile::tempdir().map_err(|e| e.to_string())?;
    let temporary_index = temp.path().join("index");
    index(root, &temporary_index, &["read-tree", &head], None)?;
    if !tracked.is_empty() {
        let mut args = vec![
            "diff",
            "--cached",
            "--binary",
            "--full-index",
            "--no-renames",
            "--no-ext-diff",
            "--no-textconv",
            &head,
            "--",
        ];
        args.extend(tracked.iter().map(String::as_str));
        let patch = run(root, &args, None)?;
        if !patch.is_empty() {
            index(
                root,
                &temporary_index,
                &["apply", "--cached", "--binary", "--whitespace=nowarn"],
                Some(&patch),
            )?;
        }
    }
    let staged_tree = text(index(root, &temporary_index, &["write-tree"], None)?)?
        .trim()
        .to_string();
    validate_oid(&staged_tree)?;
    let staged = commit(root, &staged_tree, &[&head], "Branchline selected index\n")?;
    index(root, &temporary_index, &["read-tree", &head], None)?;
    if !tracked.is_empty() {
        let mut args = vec!["ls-tree", "-r", "--name-only", "-z", &head, "--"];
        args.extend(tracked.iter().map(String::as_str));
        let original = run(root, &args, None)?;
        let original = original.split(|byte| *byte == 0).collect::<BTreeSet<_>>();
        let working = tracked
            .iter()
            .filter(|path| {
                original.contains(path.as_bytes())
                    || std::fs::symlink_metadata(root.join(path)).is_ok()
            })
            .cloned()
            .collect::<Vec<_>>();
        if !working.is_empty() {
            index(
                root,
                &temporary_index,
                &[
                    "add",
                    "--all",
                    "--pathspec-from-file=-",
                    "--pathspec-file-nul",
                ],
                Some(&input(&working)),
            )?;
        }
    }
    let tree = text(index(root, &temporary_index, &["write-tree"], None)?)?
        .trim()
        .to_string();
    validate_oid(&tree)?;
    let mut parents = vec![head.as_str(), staged.as_str()];
    let mut new_files = String::new();
    if !untracked.is_empty() {
        index(root, &temporary_index, &["read-tree", "--empty"], None)?;
        index(
            root,
            &temporary_index,
            &["add", "--pathspec-from-file=-", "--pathspec-file-nul"],
            Some(&input(&untracked)),
        )?;
        let tree = text(index(root, &temporary_index, &["write-tree"], None)?)?
            .trim()
            .to_string();
        validate_oid(&tree)?;
        new_files = commit(root, &tree, &[], "Branchline selected new files\n")?;
        parents.push(&new_files);
    }
    let branch = text(run(root, &["rev-parse", "--abbrev-ref", "HEAD"], None)?)?
        .trim()
        .to_string();
    let label = format!("On {branch}: {message}");
    let stash = commit(root, &tree, &parents, &format!("{label}\n"))?;
    if snapshot::token(root)? != expected {
        return Err("Your files changed. Review the stash again.".into());
    }
    let cleanup_token = snapshot::worktree(root)?;
    run(root, &["stash", "store", "-m", &label, &stash], None)?;
    let cleanup = (|| {
        if snapshot::worktree(root)? != cleanup_token {
            return Err(
                "Files changed after the stash was saved. Working changes were left in place."
                    .into(),
            );
        }
        if !tracked.is_empty() {
            run(
                root,
                &[
                    "restore",
                    &format!("--source={head}"),
                    "--staged",
                    "--worktree",
                    "--pathspec-from-file=-",
                    "--pathspec-file-nul",
                ],
                Some(&input(&tracked)),
            )?;
        }
        objects::remove_untracked(root, &untracked, &new_files)
    })();
    Ok(match cleanup {
        Ok(()) => {
            json!({"ok":true,"outcome":"saved","message":"Selected files saved. Your other changes are still in place.","stashOid":stash})
        }
        Err(error) => {
            json!({"ok":false,"outcome":"saved_cleanup_incomplete","message":format!("Your stash was saved, but some working changes remain: {error}"),"stashOid":stash})
        }
    })
}
