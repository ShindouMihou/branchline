use super::safe_path;
use std::{
    fs,
    io::{Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
};
pub(super) const START: &str = "<!-- branchline:integration:start -->";
pub(super) const END: &str = "<!-- branchline:integration:end -->";
pub(super) struct Document {
    pub text: String,
    identity: Option<(u64, u64)>,
    permissions: Option<fs::Permissions>,
}
pub(super) fn read(home: &Path, path: &Path) -> Result<Document, String> {
    safe_path(home, path)?;
    let file = match fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Document {
                text: String::new(),
                identity: None,
                permissions: None,
            })
        }
        Err(error) => return Err(format!("Cannot read instructions: {error}")),
    };
    let meta = file.metadata().map_err(|error| error.to_string())?;
    if !meta.is_file() || meta.nlink() != 1 || meta.len() > 1_000_000 {
        return Err("Instructions must be a regular, unshared UTF-8 file up to 1 MB.".into());
    }
    let mut text = String::new();
    file.take(1_000_001)
        .read_to_string(&mut text)
        .map_err(|_| "Instructions must contain valid UTF-8 text.")?;
    if text.len() > 1_000_000 {
        return Err("Instructions exceed the 1 MB limit.".into());
    }
    Ok(Document {
        text,
        identity: Some((meta.dev(), meta.ino())),
        permissions: Some(meta.permissions()),
    })
}
pub(super) fn write(home: &Path, path: &Path, before: &Document, text: &str) -> Result<(), String> {
    safe_path(home, path)?;
    let parent = path.parent().ok_or("Invalid instruction path")?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|error| error.to_string())?;
    temp.write_all(text.as_bytes())
        .map_err(|error| error.to_string())?;
    if let Some(permissions) = &before.permissions {
        temp.as_file()
            .set_permissions(permissions.clone())
            .map_err(|error| error.to_string())?;
    }
    temp.as_file()
        .sync_all()
        .map_err(|error| error.to_string())?;
    let current = read(home, path)?;
    if current.text != before.text || current.identity != before.identity {
        return Err("Instructions changed while preparing this update. Try again.".into());
    }
    if before.identity.is_some() {
        temp.persist(path).map_err(|error| error.to_string())?;
    } else {
        temp.persist_noclobber(path)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}
pub(super) fn span(text: &str) -> Result<Option<std::ops::Range<usize>>, String> {
    let starts = text
        .match_indices(START)
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let ends = text
        .match_indices(END)
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    if starts.is_empty() && ends.is_empty() && !text.contains("branchline:integration") {
        return Ok(None);
    }
    if starts.len() != 1 || ends.len() != 1 || starts[0] >= ends[0] {
        return Err(
            "Existing Branchline markers need manual review; instructions were kept.".into(),
        );
    }
    let start = starts[0];
    let mut end = ends[0] + END.len();
    if start > 0 && !text[..start].ends_with('\n')
        || !text[start + START.len()..].starts_with(['\n', '\r'])
        || !text[..ends[0]].ends_with('\n')
        || end < text.len() && !text[end..].starts_with(['\n', '\r'])
    {
        return Err("Branchline markers must be on separate lines; instructions were kept.".into());
    }
    if text[end..].starts_with("\r\n") {
        end += 2;
    } else if text[end..].starts_with('\n') {
        end += 1;
    }
    Ok(Some(start..end))
}
pub(super) fn newline(text: &str) -> &'static str {
    if text.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}
