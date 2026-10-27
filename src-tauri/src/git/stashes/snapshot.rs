use super::*;
use std::io::Read;

pub(super) fn path(root: &Path, path: &str) -> Result<PathBuf, String> {
    if path.is_empty()
        || path.contains('\0')
        || Path::new(path)
            .components()
            .any(|p| !matches!(p, std::path::Component::Normal(_)))
    {
        return Err("Choose a file inside this repository.".into());
    }
    let full = root.join(path);
    let mut parent = full.parent();
    while let Some(current) = parent {
        if current == root {
            break;
        }
        if std::fs::symlink_metadata(current).is_ok_and(|meta| meta.file_type().is_symlink()) {
            return Err("Stashing through a symbolic-link directory is not supported.".into());
        }
        parent = current.parent();
    }
    Ok(full)
}

pub(super) fn token(root: &Path) -> Result<String, String> {
    fingerprint(root, true)
}

pub(super) fn worktree(root: &Path) -> Result<String, String> {
    fingerprint(root, false)
}

fn fingerprint(root: &Path, stash: bool) -> Result<String, String> {
    let mut hash = Sha256::new();
    let mut commands = vec![
        vec!["rev-parse", "HEAD"],
        vec!["symbolic-ref", "--quiet", "HEAD"],
        vec!["status", "--porcelain=v1", "-z", "--untracked-files=all"],
        vec!["diff", "--binary", "--no-ext-diff", "--no-textconv"],
        vec![
            "diff",
            "--cached",
            "--binary",
            "--no-ext-diff",
            "--no-textconv",
        ],
    ];
    if stash {
        commands.push(vec!["rev-parse", "--verify", "refs/stash"]);
    }
    for args in commands {
        let (code, bytes, _) = git(root, &args, None)?;
        hash.update(code.to_le_bytes());
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
    }
    let mut paths = std::collections::BTreeSet::new();
    for file in files(root)? {
        paths.insert(file.path);
        if let Some(old) = file.old_path {
            paths.insert(old);
        }
    }
    for file in paths {
        hash.update((file.len() as u64).to_le_bytes());
        hash.update(file.as_bytes());
        hash.update([0]);
        let full = path(root, &file)?;
        let meta = match std::fs::symlink_metadata(&full) {
            Ok(meta) => meta,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                hash.update(b"missing");
                continue;
            }
            Err(error) => return Err(error.to_string()),
        };
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            hash.update(meta.mode().to_le_bytes());
        }
        if meta.file_type().is_symlink() {
            let target = std::fs::read_link(&full).map_err(|e| e.to_string())?;
            let bytes = target.as_os_str().as_encoded_bytes();
            hash.update((bytes.len() as u64).to_le_bytes());
            hash.update(bytes);
        } else if meta.is_file() {
            hash.update(meta.len().to_le_bytes());
            let mut input = std::fs::File::open(&full).map_err(|e| e.to_string())?;
            let mut buffer = [0; 65536];
            loop {
                let count = input.read(&mut buffer).map_err(|e| e.to_string())?;
                if count == 0 {
                    break;
                }
                hash.update(&buffer[..count]);
            }
            let after = input.metadata().map_err(|e| e.to_string())?;
            if after.len() != meta.len() || after.modified().ok() != meta.modified().ok() {
                return Err("A file changed while preparing the stash. Try again.".into());
            }
        } else {
            return Err(
                "Stash management does not save changes inside submodules or special files.".into(),
            );
        }
    }
    Ok(format!("{:x}", hash.finalize()))
}
