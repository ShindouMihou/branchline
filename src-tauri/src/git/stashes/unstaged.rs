use super::*;

use super::objects::oid;

pub(super) fn save(
    root: &Path,
    message: &str,
    include_untracked: bool,
    expected: &str,
) -> Result<Value, String> {
    let snapshot = text(run(
        root,
        &["stash", "create", "Branchline worktree snapshot"],
        None,
    )?)?
    .trim()
    .to_string();
    let (index, tree) = if snapshot.is_empty() {
        let tree = oid(root, &["write-tree"], None)?;
        let head = operations::head(root);
        let index = oid(
            root,
            &[
                "-c",
                "commit.gpgsign=false",
                "commit-tree",
                &tree,
                "-p",
                &head,
            ],
            Some(b"Branchline index snapshot\n"),
        )?;
        (index, tree)
    } else {
        validate_oid(&snapshot)?;
        (
            oid(root, &["rev-parse", &format!("{snapshot}^2")], None)?,
            oid(root, &["rev-parse", &format!("{snapshot}^{{tree}}")], None)?,
        )
    };
    let index_tree = oid(root, &["rev-parse", &format!("{index}^{{tree}}")], None)?;
    // A standard stash is W(H, I). Using W(I, J), with J's tree equal to I,
    // makes Git's native three-way restore apply only the unstaged I-to-W delta.
    let staged = oid(
        root,
        &[
            "-c",
            "commit.gpgsign=false",
            "commit-tree",
            &index_tree,
            "-p",
            &index,
        ],
        Some(b"Branchline saved index\n"),
    )?;
    let untracked = files(root)?
        .into_iter()
        .filter(|f| include_untracked && f.index == '?')
        .map(|f| f.path)
        .collect::<Vec<_>>();
    let temp = tempfile::tempdir().map_err(|e| e.to_string())?;
    let temporary_index = temp.path().join("index");
    let temporary_index = temporary_index
        .to_str()
        .ok_or("Invalid temporary index path")?;
    let mut parents = vec![
        "-c",
        "commit.gpgsign=false",
        "commit-tree",
        &tree,
        "-p",
        &index,
        "-p",
        &staged,
    ];
    let mut untracked_commit = String::new();
    if !untracked.is_empty() {
        let env = [("GIT_INDEX_FILE", temporary_index)];
        let exec = |args: &[&str], input: Option<&[u8]>| -> Result<Vec<u8>, String> {
            let (code, out, err) = git_env(root, args, input, &env)?;
            if code != 0 {
                return Err(err);
            }
            Ok(out)
        };
        exec(&["read-tree", "--empty"], None)?;
        let input = untracked
            .iter()
            .flat_map(|p| p.bytes().chain([0]))
            .collect::<Vec<_>>();
        exec(
            &["add", "--pathspec-from-file=-", "--pathspec-file-nul"],
            Some(&input),
        )?;
        let tree = text(exec(&["write-tree"], None)?)?.trim().to_string();
        untracked_commit = oid(
            root,
            &["-c", "commit.gpgsign=false", "commit-tree", &tree],
            Some(b"Branchline new files\n"),
        )?;
        parents.extend(["-p", &untracked_commit]);
    }
    let stash = oid(
        root,
        &parents,
        Some(format!("Branchline unstaged snapshot\n\n{message}\n").as_bytes()),
    )?;
    let patch = run(
        root,
        &[
            "diff",
            "--binary",
            "--full-index",
            "--no-renames",
            "--no-ext-diff",
            "--no-textconv",
            &index,
            &stash,
        ],
        None,
    )?;
    if patch.is_empty() && untracked.is_empty() {
        return Err("There are no unstaged changes to save.".into());
    }
    if !patch.is_empty() {
        run(
            root,
            &["apply", "--reverse", "--check", "--binary"],
            Some(&patch),
        )?;
    }
    if snapshot::token(root)? != expected {
        return Err("Your files changed. Review the stash again.".into());
    }
    let cleanup_token = snapshot::worktree(root)?;
    let branch = text(run(root, &["rev-parse", "--abbrev-ref", "HEAD"], None)?)?
        .trim()
        .to_string();
    run(
        root,
        &[
            "stash",
            "store",
            "-m",
            &format!("On {branch}: {message}"),
            &stash,
        ],
        None,
    )?;
    let cleanup = (|| {
        if snapshot::worktree(root)? != cleanup_token {
            return Err(
                "Files changed after the stash was saved. Working changes were left in place."
                    .into(),
            );
        }
        if !patch.is_empty() {
            run(
                root,
                &["apply", "--reverse", "--binary", "--whitespace=nowarn"],
                Some(&patch),
            )?;
        }
        objects::remove_untracked(root, &untracked, &untracked_commit)?;
        Ok::<_, String>(())
    })();
    Ok(match cleanup {
        Ok(()) => {
            json!({"ok":true,"outcome":"saved","message":"Unstaged changes saved. Staged changes are still ready to commit.","stashOid":stash})
        }
        Err(error) => {
            json!({"ok":false,"outcome":"saved_cleanup_incomplete","message":format!("Your stash was saved, but some working changes remain: {error}"),"stashOid":stash})
        }
    })
}
