use super::*;

fn affected(
    root: &Path,
    scope: &str,
    paths: &[String],
    include_untracked: bool,
) -> Result<Vec<File>, String> {
    if !matches!(scope, "all" | "unstaged" | "selected") {
        return Err("Choose how to save your changes.".into());
    }
    let changed = files(root)?;
    if scope == "selected"
        && (paths.is_empty() || paths.iter().any(|p| !changed.iter().any(|f| &f.path == p)))
    {
        return Err("Select changed files to stash.".into());
    }
    let result = changed
        .into_iter()
        .filter(|f| {
            (include_untracked || f.index != '?')
                && (scope != "selected" || paths.contains(&f.path))
                && (scope != "unstaged" || f.worktree != ' ')
        })
        .collect::<Vec<_>>();
    for file in &result {
        snapshot::path(root, &file.path)?;
    }
    Ok(result)
}

pub(super) fn preview(
    root: &Path,
    scope: &str,
    paths: &[String],
    include_untracked: bool,
) -> Result<Value, String> {
    ready(root)?;
    let token = snapshot::token(root)?;
    let changed = affected(root, scope, paths, include_untracked)?;
    Ok(json!({"token":token,"files":changed,"scope":scope}))
}

pub(super) fn save(
    root: &Path,
    scope: &str,
    paths: &[String],
    include_untracked: bool,
    message: &str,
    expected: &str,
) -> Result<Value, String> {
    ready(root)?;
    if message.len() > 4096 || message.contains(['\0', '\n', '\r']) {
        return Err("Use a single-line stash name under 4,096 characters.".into());
    }
    if snapshot::token(root)? != expected {
        return Err("Your files or branch changed. Review the stash again.".into());
    }
    let affected = affected(root, scope, paths, include_untracked)?;
    if affected.is_empty() {
        return Err("There are no matching changes to save.".into());
    }
    let message = if message.trim().is_empty() {
        "Saved changes"
    } else {
        message.trim()
    };
    if scope == "unstaged" {
        return super::unstaged::save(root, message, include_untracked, expected);
    }
    if scope == "selected" {
        return super::selected_files::save(root, &affected, message, expected);
    }
    let before = entries(root)?;
    let mut args = vec!["stash", "push", "-m", message];
    if include_untracked {
        args.push("--include-untracked");
    }
    let result = git(root, &args, None);
    let after = entries(root)?;
    let saved =
        after.len() != before.len() || after.iter().zip(&before).any(|(a, b)| a.oid != b.oid);
    let stash_oid = after.first().map(|entry| &entry.oid);
    match result {
        Ok((0, _, _)) if saved => Ok(
            json!({"ok":true,"outcome":"saved","message":"Changes saved in a stash.","stashOid":stash_oid}),
        ),
        result => {
            let error = match result {
                Err(error) => error,
                Ok((_, out, err)) => {
                    if err.trim().is_empty() {
                        String::from_utf8_lossy(&out).into_owned()
                    } else {
                        err
                    }
                }
            };
            if saved {
                Ok(
                    json!({"ok":false,"outcome":"saved_cleanup_incomplete","message":format!("A stash was saved, but cleanup did not complete: {error}"),"stashOid":stash_oid}),
                )
            } else {
                Err(format!("No stash was saved. {error}"))
            }
        }
    }
}
