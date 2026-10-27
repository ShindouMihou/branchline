use super::*;
use base64::Engine;
const LIMIT: u64 = 10 * 1024 * 1024;
fn supported(path: &str) -> bool {
    matches!(
        path.rsplit('.')
            .next()
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str(),
        "svg" | "png" | "jpg" | "jpeg" | "gif" | "webp" | "avif" | "bmp" | "ico"
    )
}
fn image(path: &str, bytes: Vec<u8>) -> Option<Value> {
    let mime = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        "image/png"
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        "image/jpeg"
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        "image/gif"
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        "image/webp"
    } else if bytes.get(4..8) == Some(b"ftyp")
        && [b"avif".as_slice(), b"avis".as_slice()]
            .iter()
            .any(|brand| {
                bytes
                    .get(8..32.min(bytes.len()))
                    .unwrap_or_default()
                    .windows(4)
                    .any(|v| v == *brand)
            })
    {
        "image/avif"
    } else if bytes.starts_with(b"BM") {
        "image/bmp"
    } else if bytes.starts_with(b"\0\0\x01\0") {
        "image/x-icon"
    } else if path.to_ascii_lowercase().ends_with(".svg")
        && std::str::from_utf8(&bytes)
            .ok()
            .is_some_and(|s| s.chars().take(4096).collect::<String>().contains("<svg"))
    {
        "image/svg+xml"
    } else {
        return None;
    };
    Some(
        json!({"dataUrl":format!("data:{mime};base64,{}",base64::engine::general_purpose::STANDARD.encode(&bytes)),"bytes":bytes.len()}),
    )
}
fn blob(root: &Path, spec: &str, path: &str) -> Result<Option<Value>, String> {
    let size = git(root, &["cat-file", "-s", spec], None)?;
    if size.0 != 0 {
        return Ok(None);
    }
    let size = text(size.1)?
        .trim()
        .parse::<u64>()
        .map_err(|_| "Unknown image size")?;
    if size > LIMIT {
        return Err("Image preview is limited to 10 MB per version.".into());
    }
    Ok(image(path, run(root, &["cat-file", "blob", spec], None)?))
}
pub fn preview(root: &Path, path: &str, staged: bool, oid: Option<&str>) -> Result<Value, String> {
    if !supported(path)
        || Path::new(path)
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return Err("Choose a supported repository image file.".into());
    }
    let versions = (|| {
        if let Some(oid) = oid {
            validate_oid(oid)?;
            return Ok((
                blob(root, &format!("{oid}^:{path}"), path)?,
                blob(root, &format!("{oid}:{path}"), path)?,
            ));
        }
        let before = blob(
            root,
            &format!("{}:{path}", if staged { "HEAD" } else { "" }),
            path,
        )?;
        let after = if staged {
            blob(root, &format!(":{path}"), path)?
        } else {
            let full = root.join(path);
            match std::fs::symlink_metadata(&full) {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                Err(e) => return Err(e.to_string()),
                Ok(meta) => {
                    if !meta.is_file() || meta.len() > LIMIT {
                        return Err(
                            "Image preview needs a regular file no larger than 10 MB.".into()
                        );
                    }
                    let canonical = std::fs::canonicalize(&full).map_err(|e| e.to_string())?;
                    if !canonical.starts_with(root) {
                        return Err("Image is outside this repository.".into());
                    }
                    let mut bytes = Vec::new();
                    std::fs::File::open(canonical)
                        .map_err(|e| e.to_string())?
                        .take(LIMIT + 1)
                        .read_to_end(&mut bytes)
                        .map_err(|e| e.to_string())?;
                    if bytes.len() as u64 > LIMIT {
                        return Err("Image grew beyond the preview limit.".into());
                    }
                    image(path, bytes)
                }
            }
        };
        Ok((before, after))
    })();
    match versions {
        Ok((before, after)) => Ok(
            json!({"before":before,"after":after,"notice":if before.is_none()&&after.is_none(){Some("No supported image could be decoded. Use Code to inspect the Git diff.")}else{None}}),
        ),
        Err(reason) => Ok(json!({"before":null,"after":null,"notice":reason})),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_image_bytes_and_does_not_inline_svg_markup() {
        let svg = b"<svg xmlns=\"http://www.w3.org/2000/svg\"><script>alert(1)</script></svg>";
        let rendered = image("icon.svg", svg.to_vec()).unwrap();
        assert!(rendered["dataUrl"]
            .as_str()
            .unwrap()
            .starts_with("data:image/svg+xml;base64,"));
        assert!(!rendered["dataUrl"].as_str().unwrap().contains("<script>"));
        assert!(image("fake.png", b"not an image".to_vec()).is_none());
    }
}

pub fn raster(path: &str) -> bool {
    supported(path) && !path.to_ascii_lowercase().ends_with(".svg")
}
