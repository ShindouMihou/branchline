use std::{fs, io::Read, os::unix::fs::{MetadataExt, OpenOptionsExt}, path::{Path, PathBuf}};

fn read_file(requested: PathBuf, boundary: Option<&Path>) -> Result<String, String> {
    let metadata = fs::symlink_metadata(&requested)
        .map_err(|_| "The message file is not currently readable.")?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > 64_000 {
        return Err("Message previews require regular files up to 64 KB; symbolic links and special files are not read.".into());
    }
    let path = fs::canonicalize(&requested).map_err(|_| "The message file is not currently readable.")?;
    if boundary.is_some_and(|root| !path.starts_with(root)) {
        return Err("Commit message files are previewed only inside this repository.".into());
    }
    let mut file = fs::OpenOptions::new().read(true).custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(&path).map_err(|_| "The message file changed or cannot be opened safely.")?;
    let opened = file.metadata().map_err(|_| "Cannot identify the opened message file.")?;
    if !opened.is_file() || metadata.dev() != opened.dev() || metadata.ino() != opened.ino() {
        return Err("The message file changed during preview.".into());
    }
    let mut bytes = Vec::new();
    (&mut file).take(64_001).read_to_end(&mut bytes).map_err(|_| "Cannot read the message file.")?;
    let finished = file.metadata().map_err(|_| "Cannot identify the opened message file.")?;
    let current = fs::metadata(&requested).map_err(|_| "The message file changed during preview.")?;
    if bytes.len() > 64_000 || opened.len() != bytes.len() as u64
        || opened.len() != finished.len() || opened.mtime() != finished.mtime()
        || opened.mtime_nsec() != finished.mtime_nsec() || opened.ctime() != finished.ctime()
        || opened.ctime_nsec() != finished.ctime_nsec()
        || current.dev() != opened.dev() || current.ino() != opened.ino()
    {
        return Err("The message file changed during preview or exceeds 64 KB.".into());
    }
    String::from_utf8(bytes).map_err(|_| "The message file is not UTF-8 text.".into())
}

pub(super) fn read_text(root: Option<&Path>, value: &str) -> Result<String, String> {
    let root = root.ok_or("File contents are not read after a directory or configuration change; review the file at execution time.")?;
    if value == "-" { return Err("The message is read from stdin when this command runs.".into()); }
    let root = fs::canonicalize(root).map_err(|_| "Cannot identify the repository for the message preview.")?;
    read_file(root.join(value), Some(&root))
}

pub(super) fn read_body(root: Option<&Path>, value: &str) -> Result<String, String> {
    if value == "-" { return Err("The pull request description is read from stdin when this command runs.".into()); }
    let path = Path::new(value);
    if path.is_absolute() { return read_file(path.to_path_buf(), None); }
    read_text(root, value)
}
