use super::*;

fn parents(root: &Path, oid: &str) -> Result<Vec<String>, String> {
    validate_oid(oid)?;
    let raw = text(run(root, &["rev-list", "--parents", "-n", "1", oid], None)?)?;
    let parents = raw
        .split_whitespace()
        .skip(1)
        .map(str::to_string)
        .collect::<Vec<_>>();
    if parents.len() < 2 {
        return Err("This object is not a stash.".into());
    }
    Ok(parents)
}

fn paths(bytes: Vec<u8>) -> Result<Vec<String>, String> {
    bytes
        .split(|b| *b == 0)
        .filter(|p| !p.is_empty())
        .map(|p| {
            String::from_utf8(p.to_vec()).map_err(|_| "A stash path is not valid UTF-8.".into())
        })
        .collect()
}

pub(super) fn files(root: &Path, oid: &str, staged: bool) -> Result<Value, String> {
    let parents = parents(root, oid)?;
    let target = if staged { &parents[1] } else { oid };
    let mut files = paths(run(
        root,
        &[
            "diff",
            "--name-only",
            "-z",
            "--no-renames",
            &parents[0],
            target,
            "--",
        ],
        None,
    )?)?
    .into_iter()
    .map(|path| json!({"path":path,"source":"tracked"}))
    .collect::<Vec<_>>();
    if !staged && parents.len() > 2 {
        files.extend(
            paths(run(
                root,
                &["ls-tree", "-r", "--name-only", "-z", &parents[2]],
                None,
            )?)?
            .into_iter()
            .map(|path| json!({"path":path,"source":"untracked"})),
        );
    }
    let custom = text(run(root, &["show", "-s", "--format=%s", oid], None)?)?.trim()
        == "Branchline unstaged snapshot";
    Ok(json!({"files":files,"baseline":if custom {"Saved index"}else{"Original HEAD"}}))
}

fn target(
    root: &Path,
    oid: &str,
    path: &str,
    staged: bool,
) -> Result<(Option<String>, String), String> {
    snapshot::path(root, path)?;
    let parents = parents(root, oid)?;
    let files = files(root, oid, staged)?;
    let file = files["files"]
        .as_array()
        .and_then(|files| {
            files
                .iter()
                .find(|file| file["path"].as_str() == Some(path))
        })
        .ok_or("Choose a file saved in this stash.")?;
    if file["source"] == "untracked" {
        Ok((None, parents[2].clone()))
    } else {
        Ok((
            Some(parents[0].clone()),
            if staged {
                parents[1].clone()
            } else {
                oid.into()
            },
        ))
    }
}

pub(super) fn diff(root: &Path, oid: &str, path: &str, staged: bool) -> Result<Value, String> {
    let (base, target) = target(root, oid, path, staged)?;
    let bytes = if let Some(base) = base {
        run(
            root,
            &[
                "diff",
                "--no-ext-diff",
                "--no-textconv",
                "--no-renames",
                &base,
                &target,
                "--",
                path,
            ],
            None,
        )?
    } else {
        run(
            root,
            &[
                "show",
                "--root",
                "--format=",
                "--no-ext-diff",
                "--no-textconv",
                &target,
                "--",
                path,
            ],
            None,
        )?
    };
    if bytes.len() > 1_000_000 {
        return Ok(
            json!({"patch":"","notice":"This diff is too large to display (over 1 MB). The complete file is saved in the stash."}),
        );
    }
    let source = source::preview(root, path, false, Some(&target))?;
    Ok(
        json!({"patch":String::from_utf8_lossy(&bytes),"before":source["before"],"after":source["after"]}),
    )
}

pub(super) fn image(root: &Path, oid: &str, path: &str, staged: bool) -> Result<Value, String> {
    let (_, target) = target(root, oid, path, staged)?;
    media::preview(root, path, false, Some(&target))
}
