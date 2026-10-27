use super::{
    command_details::{Detail, FileSource},
    commands::Manifest,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    ffi::CString,
    fs::{self, File, OpenOptions},
    io::Read,
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::fs::{MetadataExt, OpenOptionsExt},
    },
    path::{Component, Path},
};

const LIMIT: usize = 200_000;

fn read_git(root: &Path, args: &[&str]) -> Result<(i32, Vec<u8>, String), String> {
    super::super::git_env(
        root,
        args,
        None,
        &[("GIT_NO_REPLACE_OBJECTS", "1"), ("GIT_NO_LAZY_FETCH", "1")],
    )
}

fn valid_path(path: &str) -> bool {
    !path.is_empty()
        && !path.contains('\0')
        && Path::new(path)
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn blob(root: &Path, spec: &str) -> Result<Option<Vec<u8>>, String> {
    let (code, bytes, _) = read_git(root, &["rev-parse", "--verify", spec])?;
    if code != 0 {
        return Ok(None);
    }
    let oid = String::from_utf8(bytes).map_err(|_| "Git returned an invalid object identity.")?;
    let oid = oid.trim();
    super::super::validate_oid(oid)?;
    let (code, size, _) = read_git(root, &["cat-file", "-s", oid])?;
    if code != 0 {
        return Err("The file object is no longer available.".into());
    }
    let size = String::from_utf8(size)
        .ok()
        .and_then(|s| s.trim().parse::<usize>().ok())
        .ok_or("Cannot determine file size.")?;
    if size > LIMIT {
        return Err("This file exceeds the 200 KB review preview limit.".into());
    }
    let (code, bytes, _) = read_git(root, &["cat-file", "blob", oid])?;
    if code != 0 || bytes.len() > LIMIT {
        return Err("Cannot read a bounded file snapshot from Git.".into());
    }
    Ok(Some(bytes))
}

fn worktree(root: &Path, path: &str, manifest: &Manifest) -> Result<Option<Vec<u8>>, String> {
    let mut directory = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
        .open(root)
        .map_err(|_| "The repository directory changed or is unavailable.")?;
    let root_metadata = directory
        .metadata()
        .map_err(|_| "Cannot identify the opened repository.")?;
    if root_metadata.dev() != manifest.root_device || root_metadata.ino() != manifest.root_inode {
        return Err("The repository directory was replaced.".into());
    }
    let parts = Path::new(path).components().collect::<Vec<_>>();
    for (index, component) in parts.iter().enumerate() {
        let Component::Normal(part) = component else {
            return Err("Choose a listed repository file.".into());
        };
        let part = CString::new(part.as_encoded_bytes()).map_err(|_| "Invalid file name.")?;
        let final_part = index + 1 == parts.len();
        let flags = libc::O_RDONLY
            | libc::O_NOFOLLOW
            | libc::O_CLOEXEC
            | if final_part {
                libc::O_NONBLOCK
            } else {
                libc::O_DIRECTORY
            };
        let fd = unsafe { libc::openat(directory.as_raw_fd(), part.as_ptr(), flags) };
        if fd < 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::NotFound {
                return Ok(None);
            }
            return Err(
                "Cannot preview symbolic links or inaccessible paths; no link targets were read."
                    .into(),
            );
        }
        let file = unsafe { File::from_raw_fd(fd) };
        if final_part {
            let metadata = file
                .metadata()
                .map_err(|_| "Cannot identify the selected file.")?;
            if !metadata.is_file() {
                return Err("Only regular files can be previewed here.".into());
            }
            if metadata.len() > LIMIT as u64 {
                return Err("This file exceeds the 200 KB review preview limit.".into());
            }
            let mut bytes = Vec::new();
            file.take((LIMIT + 1) as u64)
                .read_to_end(&mut bytes)
                .map_err(|_| "Cannot read the selected file.")?;
            if bytes.len() > LIMIT {
                return Err("The file grew beyond the review preview limit.".into());
            }
            return Ok(Some(bytes));
        }
        directory = file;
    }
    Err("Choose a listed repository file.".into())
}

pub fn inspect(
    root: &Path,
    detail: &Detail,
    path: &str,
    manifest: &Manifest,
) -> Result<Value, String> {
    if !valid_path(path)
        || !detail
            .files
            .as_ref()
            .is_some_and(|files| files.iter().any(|file| file == path))
    {
        return Err("Choose a file listed in this request detail.".into());
    }
    let selection = detail
        .file_previews
        .iter()
        .find(|selection| selection.path == path)
        .ok_or(
            "No file snapshot source was recorded. Prepare a new request to review this file.",
        )?;
    let root = fs::canonicalize(root).map_err(|_| "The repository is unavailable.")?;
    let metadata = fs::metadata(&root).map_err(|_| "Cannot identify the repository.")?;
    if root.to_string_lossy() != manifest.root
        || metadata.dev() != manifest.root_device
        || metadata.ino() != manifest.root_inode
        || super::repository_identity(&root)? != manifest.repository_identity
    {
        return Err("The request's repository identity changed. Prepare a new review.".into());
    }
    let baseline = if detail.kind == "index.stage" { "index" } else { "head" };
    let before = blob(&root, &format!("{}:{path}", if baseline == "index" { "" } else { "HEAD" }))?;
    let after = match selection.source {
        FileSource::Index => blob(&root, &format!(":{path}"))?,
        FileSource::Worktree => worktree(&root, path, manifest)?,
    };
    let source = match selection.source {
        FileSource::Index => "index",
        FileSource::Worktree => "worktree",
    };
    let snapshot = json!({"path":path,"source":source,"baseline":baseline,"before":before.as_ref().map(|bytes| format!("{:x}",Sha256::digest(bytes))),"after":after.as_ref().map(|bytes| format!("{:x}",Sha256::digest(bytes)))});
    let snapshot_id = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&snapshot).map_err(|e| e.to_string())?)
    );
    let mut notice = "Current snapshot loaded when you selected this file. It is not a frozen approval preview; earlier commands, Git filters, or later edits may change the eventual result. File modes and rename metadata are not represented.".to_string();
    notice.push_str(if baseline == "index" { " This shows changes from the current index to the working file." } else { " This compares the selected content with HEAD." });
    let binary = before
        .iter()
        .chain(after.iter())
        .any(|bytes| bytes.contains(&0) || std::str::from_utf8(bytes).is_err());
    let patch = if binary {
        notice.push_str(" Binary file contents are not shown.");
        String::new()
    } else {
        let temporary =
            tempfile::tempdir().map_err(|_| "Cannot create a private diff snapshot.")?;
        if let Some(bytes) = &before {
            fs::write(temporary.path().join("before"), bytes)
                .map_err(|_| "Cannot save the before snapshot.")?;
        }
        if let Some(bytes) = &after {
            fs::write(temporary.path().join("after"), bytes)
                .map_err(|_| "Cannot save the after snapshot.")?;
        }
        if before.is_none() && after.is_none() {
            String::new()
        } else {
            let (code, bytes, _) = read_git(
                temporary.path(),
                &[
                    "diff",
                    "--no-index",
                    "--no-ext-diff",
                    "--no-textconv",
                    "--no-color",
                    "--",
                    if before.is_some() {
                        "before"
                    } else {
                        "/dev/null"
                    },
                    if after.is_some() {
                        "after"
                    } else {
                        "/dev/null"
                    },
                ],
            )?;
            if !matches!(code, 0 | 1) {
                return Err("Cannot render this current file comparison.".into());
            }
            if bytes.len() > LIMIT {
                notice.push_str(" The diff exceeds the 200 KB display limit.");
                String::new()
            } else {
                String::from_utf8(bytes).map_err(|_| "The diff is not UTF-8 text.")?
            }
        }
    };
    let added = patch
        .lines()
        .skip_while(|line| !line.starts_with("@@ "))
        .filter(|line| line.starts_with('+'))
        .count();
    let removed = patch
        .lines()
        .skip_while(|line| !line.starts_with("@@ "))
        .filter(|line| line.starts_with('-'))
        .count();
    Ok(
        json!({"path":path,"source":source,"baseline":baseline,"patch":patch,"snapshotId":snapshot_id,"added":added,"removed":removed,"notice":notice,"before":if binary {None}else{before.as_ref().and_then(|bytes| std::str::from_utf8(bytes).ok())},"after":if binary {None}else{after.as_ref().and_then(|bytes| std::str::from_utf8(bytes).ok())}}),
    )
}

#[cfg(test)]
mod tests;
