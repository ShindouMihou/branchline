//! Counts correspond to the staged and unstaged diffs shown in Changes.
use super::*;
use std::collections::HashSet;
#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LineStats {
    additions: u64,
    deletions: u64,
    binary_files: usize,
    skipped_files: usize,
}
fn numstat(bytes: &[u8], stats: &mut LineStats, binary: &mut HashSet<Vec<u8>>) {
    let mut records = bytes.split(|b| *b == 0);
    while let Some(record) = records.next() {
        if record.is_empty() {
            continue;
        }
        let mut fields = record.splitn(3, |b| *b == b'\t');
        let (Some(added), Some(removed), Some(path)) =
            (fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        let path = if path.is_empty() {
            let _old = records.next();
            records.next().unwrap_or_default()
        } else {
            path
        };
        if added == b"-" || removed == b"-" {
            binary.insert(path.to_vec());
            continue;
        }
        if let (Ok(a), Ok(d)) = (
            String::from_utf8_lossy(added).parse::<u64>(),
            String::from_utf8_lossy(removed).parse::<u64>(),
        ) {
            stats.additions = stats.additions.saturating_add(a);
            stats.deletions = stats.deletions.saturating_add(d)
        }
    }
}
fn untracked(path: &Path) -> Result<Option<u64>, String> {
    let metadata = std::fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if metadata.file_type().is_symlink() {
        return Ok(Some(1));
    }
    if !metadata.is_file() || metadata.len() > 10 * 1024 * 1024 {
        return Err("Line count unavailable for a large or special file".into());
    }
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut buffer = [0u8; 8192];
    let mut count = 0u64;
    let mut last = None;
    let mut read = 0usize;
    loop {
        let n = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        read += n;
        if read > 10 * 1024 * 1024 {
            return Err("File changed while counting".into());
        }
        if buffer[..n].contains(&0) {
            return Ok(None);
        }
        count += buffer[..n].iter().filter(|b| **b == b'\n').count() as u64;
        last = Some(buffer[n - 1]);
    }
    Ok(Some(count + u64::from(last.is_some_and(|b| b != b'\n'))))
}
pub fn count(root: &Path, files: &[File]) -> Result<LineStats, String> {
    let mut result = LineStats::default();
    let mut binary = HashSet::new();
    for cached in [true, false] {
        let mut args = vec!["diff", "--numstat", "-z", "--no-ext-diff", "--no-textconv"];
        if cached {
            args.push("--cached")
        }
        numstat(&run(root, &args, None)?, &mut result, &mut binary);
    }
    for file in files.iter().filter(|f| f.index == '?' && f.worktree == '?') {
        match untracked(&root.join(&file.path)) {
            Ok(Some(n)) => result.additions = result.additions.saturating_add(n),
            Ok(None) => {
                binary.insert(file.path.as_bytes().to_vec());
            }
            Err(_) => result.skipped_files += 1,
        }
    }
    result.binary_files = binary.len();
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn handles_binary_rename_and_tab_or_newline_paths() {
        let mut s = LineStats::default();
        let mut binary = HashSet::new();
        numstat(
            b"2\t3\tname\twith\nspace\0-\t-\tbinary\0-\t-\tbinary\00\t0\t\0old\0new\0",
            &mut s,
            &mut binary,
        );
        assert_eq!((s.additions, s.deletions, binary.len()), (2, 3, 1));
    }
    #[test]
    fn untracked_counts_final_line_and_skips_binary_or_large_files() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("new");
        std::fs::write(&p, b"first\nlast").unwrap();
        assert_eq!(untracked(&p).unwrap(), Some(2));
        std::fs::write(&p, b"\0binary").unwrap();
        assert_eq!(untracked(&p).unwrap(), None);
        let f = std::fs::File::create(&p).unwrap();
        f.set_len(11 * 1024 * 1024).unwrap();
        assert!(untracked(&p).is_err());
    }
}
