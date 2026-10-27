use super::*;
const LIMIT: u64 = 1_000_000;
fn blob(root: &Path, spec: &str) -> Result<Option<String>, String> {
    let size = git(root, &["cat-file", "-s", spec], None)?;
    if size.0 != 0 {
        return Ok(None);
    }
    if text(size.1)?
        .trim()
        .parse::<u64>()
        .map_err(|_| "Unknown source size")?
        > LIMIT
    {
        return Ok(None);
    }
    Ok(
        String::from_utf8(run(root, &["cat-file", "blob", spec], None)?)
            .ok()
            .filter(|s| !s.contains('\0')),
    )
}
pub fn preview(root: &Path, path: &str, staged: bool, oid: Option<&str>) -> Result<Value, String> {
    if path.is_empty()
        || Path::new(path)
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return Err("Choose a repository file".into());
    }
    if let Some(oid) = oid {
        validate_oid(oid)?;
        return Ok(
            json!({"before":blob(root,&format!("{oid}^:{path}"))?,"after":blob(root,&format!("{oid}:{path}"))?}),
        );
    }
    let before = blob(
        root,
        &format!("{}:{path}", if staged { "HEAD" } else { "" }),
    )?;
    let after = if staged {
        blob(root, &format!(":{path}"))?
    } else {
        let full = root.join(path);
        match std::fs::symlink_metadata(&full) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.to_string()),
            Ok(meta) => {
                if !meta.is_file() || meta.len() > LIMIT {
                    None
                } else {
                    let canonical = std::fs::canonicalize(full).map_err(|e| e.to_string())?;
                    if !canonical.starts_with(root) {
                        return Err("Source is outside this repository".into());
                    }
                    let mut bytes = Vec::new();
                    std::fs::File::open(canonical)
                        .map_err(|e| e.to_string())?
                        .take(LIMIT + 1)
                        .read_to_end(&mut bytes)
                        .map_err(|e| e.to_string())?;
                    if bytes.len() as u64 > LIMIT {
                        None
                    } else {
                        String::from_utf8(bytes).ok().filter(|s| !s.contains('\0'))
                    }
                }
            }
        }
    };
    Ok(json!({"before":before,"after":after}))
}
