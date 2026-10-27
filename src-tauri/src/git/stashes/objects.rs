use super::*;

pub(super) fn oid(root: &Path, args: &[&str], input: Option<&[u8]>) -> Result<String, String> {
    let oid = text(run(root, args, input)?)?.trim().to_string();
    validate_oid(&oid)?;
    Ok(oid)
}

pub(super) fn commit(
    root: &Path,
    tree: &str,
    parents: &[&str],
    message: &str,
) -> Result<String, String> {
    let mut args = vec!["-c", "commit.gpgsign=false", "commit-tree", tree];
    for parent in parents {
        args.extend(["-p", parent]);
    }
    oid(root, &args, Some(message.as_bytes()))
}

pub(super) fn index(
    root: &Path,
    path: &Path,
    args: &[&str],
    input: Option<&[u8]>,
) -> Result<Vec<u8>, String> {
    let path = path.to_str().ok_or("Invalid temporary index path")?;
    let (code, out, error) = git_env(root, args, input, &[("GIT_INDEX_FILE", path)])?;
    if code != 0 {
        return Err(error);
    }
    Ok(out)
}

pub(super) fn input(paths: &[String]) -> Vec<u8> {
    paths
        .iter()
        .flat_map(|path| path.bytes().chain([0]))
        .collect()
}

pub(super) fn remove_untracked(root: &Path, paths: &[String], commit: &str) -> Result<(), String> {
    for file in paths {
        let path = snapshot::path(root, file)?;
        let meta = std::fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
        let actual = if meta.file_type().is_symlink() {
            let target = std::fs::read_link(&path).map_err(|e| e.to_string())?;
            oid(
                root,
                &["hash-object", "--stdin"],
                Some(target.as_os_str().as_encoded_bytes()),
            )?
        } else {
            oid(
                root,
                &["hash-object", &format!("--path={file}"), "--", file],
                None,
            )?
        };
        let saved = oid(root, &["rev-parse", &format!("{commit}:{file}")], None)?;
        if actual != saved {
            return Err(format!(
                "{file} changed after capture and was left in place."
            ));
        }
        run(root, &["clean", "-f", "--", file], None)?;
        if std::fs::symlink_metadata(path).is_ok() {
            return Err(format!("{file} was saved but could not be removed."));
        }
    }
    Ok(())
}
