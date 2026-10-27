pub(crate) mod ai;
mod line_stats;
mod media;
mod operations;
mod source;
mod watch;
pub(crate) mod stashes;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};
use wait_timeout::ChildExt;

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Request {
    Open {
        path: String,
    },
    OpenQuick {
        path: String,
    },
    StatusDetails {
        expected_root: String,
    },
    Status,
    Stash { request: stashes::Request },
    Revision,
    Branches {
        expected_root: Option<String>,
    },
    SwitchBranch {
        reference: String,
        create: Option<String>,
        expected_head: String,
    },
    BranchAction {
        kind: String,
        reference: String,
        expected_oid: String,
        name: String,
        remote: String,
        branch: String,
        expected_head: String,
    },
    Network {
        kind: String,
        remote: String,
        branch: String,
        expected_head: String,
    },
    RewritePreview {
        kind: String,
        oids: Vec<String>,
    },
    Rewrite {
        kind: String,
        oids: Vec<String>,
        message: String,
        expected: String,
    },
    CommitAction {
        kind: String,
        oid: String,
        expected_head: String,
    },
    AbortOperation {
        kind: String,
    },
    SourcePreview {
        path: String,
        staged: bool,
        oid: Option<String>,
        expected_root: String,
    },
    ImagePreview {
        path: String,
        staged: bool,
        oid: Option<String>,
        expected_root: String,
    },
    Diff {
        path: String,
        staged: bool,
    },
    StageFile {
        path: String,
        staged: bool,
    },
    StageHunk {
        path: String,
        staged: bool,
        patch: String,
        hunk: usize,
    },
    Commit {
        message: String,
        expected: String,
    },
    History {
        skip: usize,
        #[serde(default)]
        all: bool,
    },
    CommitFiles {
        oid: String,
    },
    CommitDiff {
        oid: String,
        path: String,
    },
}
#[derive(Default)]
pub struct Service {
    root: Option<PathBuf>,
    watch: watch::RepoWatch,
}
#[derive(Serialize)]
struct File {
    path: String,
    old_path: Option<String>,
    index: char,
    worktree: char,
}

// Drain both pipes concurrently so large diffs and verbose hooks cannot deadlock.
fn git(root: &Path, args: &[&str], input: Option<&[u8]>) -> Result<(i32, Vec<u8>, String), String> {
    git_env(root, args, input, &[])
}
fn git_env(
    root: &Path,
    args: &[&str],
    input: Option<&[u8]>,
    env: &[(&str, &str)],
) -> Result<(i32, Vec<u8>, String), String> {
    use std::os::unix::process::CommandExt;
    let mut cmd = Command::new("git");
    cmd.process_group(0);
    cmd.current_dir(root)
        .args(["-c", "core.fsmonitor=false"])
        .args(args)
        .env("GIT_LITERAL_PATHSPECS", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("LC_ALL", "C")
        .env("GIT_OPTIONAL_LOCKS", "0");
    for key in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    ] {
        cmd.env_remove(key);
    }
    cmd.envs(env.iter().copied());
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Cannot run Git: {e}"))?;
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let out_thread = std::thread::spawn(move || {
        let mut b = Vec::new();
        stdout.read_to_end(&mut b).map(|_| b)
    });
    let err_thread = std::thread::spawn(move || {
        let mut b = Vec::new();
        stderr.read_to_end(&mut b).map(|_| b)
    });
    let mut stdin = child.stdin.take().unwrap();
    let bytes = input.map(|b| b.to_vec()).unwrap_or_default();
    let writer = std::thread::spawn(move || stdin.write_all(&bytes));
    let status = match child
        .wait_timeout(Duration::from_secs(60))
        .map_err(|e| e.to_string())?
    {
        Some(status) => status,
        None => {
            unsafe {
                libc::kill(-(child.id() as i32), libc::SIGKILL);
            }
            let _ = child.kill();
            let _ = child.wait();
            return Err(
                "Git exceeded 60 seconds. Refresh the repository to inspect its state.".into(),
            );
        }
    };
    let out = out_thread
        .join()
        .map_err(|_| "Output reader failed")?
        .map_err(|e| e.to_string())?;
    let err = err_thread
        .join()
        .map_err(|_| "Error reader failed")?
        .map_err(|e| e.to_string())?;
    let _ = writer.join();
    Ok((
        status.code().unwrap_or(-1),
        out,
        String::from_utf8_lossy(&err).into_owned(),
    ))
}
fn run(root: &Path, args: &[&str], input: Option<&[u8]>) -> Result<Vec<u8>, String> {
    let (code, out, err) = git(root, args, input)?;
    if code != 0 {
        return Err(if err.trim().is_empty() {
            String::from_utf8_lossy(&out).into_owned()
        } else {
            err.trim().to_string()
        });
    }
    Ok(out)
}
fn text(bytes: Vec<u8>) -> Result<String, String> {
    String::from_utf8(bytes)
        .map_err(|_| "This content is not UTF-8. Use file-level staging for this file.".into())
}
fn files(root: &Path) -> Result<Vec<File>, String> {
    let raw = run(
        root,
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
        None,
    )?;
    let mut entries = raw.split(|b| *b == 0).filter(|b| !b.is_empty());
    let mut result = Vec::new();
    while let Some(entry) = entries.next() {
        if entry.len() < 4 {
            continue;
        }
        let index = entry[0] as char;
        let worktree = entry[1] as char;
        let path = text(entry[3..].to_vec())?;
        let old_path = if index == 'R' || index == 'C' || worktree == 'R' || worktree == 'C' {
            entries.next().map(|p| text(p.to_vec())).transpose()?
        } else {
            None
        };
        result.push(File {
            path,
            old_path,
            index,
            worktree,
        });
    }
    Ok(result)
}
fn diff(root: &Path, path: &str, staged: bool) -> Result<String, String> {
    let mut args = vec![
        "diff",
        "--no-ext-diff",
        "--no-textconv",
        "--no-color",
        "--binary",
        "--no-renames",
        "--unified=3",
    ];
    if media::raster(path) {
        args.retain(|arg| *arg != "--binary");
    }
    if staged {
        args.push("--cached");
    }
    args.extend(["--", path]);
    text(run(root, &args, None)?)
}
fn staged_patch(root: &Path) -> Result<String, String> {
    text(run(
        root,
        &[
            "diff",
            "--cached",
            "--no-ext-diff",
            "--no-textconv",
            "--no-color",
            "--binary",
            "--no-renames",
        ],
        None,
    )?)
}
fn staged_token(root: &Path) -> Result<String, String> {
    let patch = staged_patch(root)?;
    if patch.is_empty() {
        Ok(String::new())
    } else {
        Ok(format!("{:x}", Sha256::digest(patch.as_bytes())))
    }
}
fn hunk_patch(patch: &str, hunk: usize) -> Result<String, String> {
    if patch.contains("GIT binary patch")
        || patch.contains("new file mode")
        || patch.contains("deleted file mode")
        || patch.contains("old mode")
    {
        return Err(
            "Use file-level staging for binary, added, deleted, or mode-changing files.".into(),
        );
    }
    let mut start = None;
    let mut end = patch.len();
    let mut count = 0;
    let mut offset = 0;
    let mut header_end = None;
    for line in patch.split_inclusive('\n') {
        if line.starts_with("@@ ") {
            if header_end.is_none() {
                header_end = Some(offset);
            }
            if count == hunk {
                start = Some(offset);
            } else if count == hunk + 1 {
                end = offset;
                break;
            }
            count += 1;
        }
        offset += line.len();
    }
    let start = start.ok_or("Hunk no longer exists. Refresh and try again.")?;
    Ok(format!(
        "{}{}",
        &patch[..header_end.unwrap()],
        &patch[start..end]
    ))
}
fn validate_oid(oid: &str) -> Result<(), String> {
    if oid.len() != 40 && oid.len() != 64 || !oid.bytes().all(|b| b.is_ascii_hexdigit()) {
        Err("Invalid commit identifier".into())
    } else {
        Ok(())
    }
}
impl Service {
    pub fn handle(&mut self, request: Request) -> Result<Value, String> {
        if let Request::Open { path } | Request::OpenQuick { path } = &request {
            let quick = matches!(&request, Request::OpenQuick { .. });
            let path = std::fs::canonicalize(path).map_err(|e| e.to_string())?;
            let root = text(run(&path, &["rev-parse", "--show-toplevel"], None)?)?;
            let previous = self
                .root
                .replace(PathBuf::from(root.trim_end_matches('\n')));
            let previous_watch = std::mem::replace(
                &mut self.watch,
                watch::RepoWatch::start(self.root.as_ref().unwrap()),
            );
            let result = self.status_details(!quick);
            if result.is_err() {
                self.root = previous;
                self.watch = previous_watch;
            }
            return result;
        }
        let root = self.root.as_ref().ok_or("Open a repository first")?.clone();
        match request {
            Request::Open { .. } | Request::OpenQuick { .. } => unreachable!(),
            Request::StatusDetails { expected_root } => {
                if Path::new(&expected_root) != root {
                    return Err("The repository changed while details were loading".into());
                }
                self.status()
            }
            Request::Status => self.status(),
            Request::Stash { request } => stashes::handle(&root, request),
            Request::Revision => Ok(
                json!({"root":root,"revision":self.watch.revision(),"watching":self.watch.healthy()}),
            ),
            request @ (Request::Branches { .. }
            | Request::SwitchBranch { .. }
            | Request::Network { .. }
            | Request::BranchAction { .. }
            | Request::RewritePreview { .. }
            | Request::Rewrite { .. }
            | Request::CommitAction { .. }
            | Request::AbortOperation { .. }) => self.operation(request),
            Request::SourcePreview {
                path,
                staged,
                oid,
                expected_root,
            } => {
                if Path::new(&expected_root) != root {
                    return Err("The repository changed while loading source".into());
                }
                source::preview(&root, &path, staged, oid.as_deref())
            }
            Request::ImagePreview {
                path,
                staged,
                oid,
                expected_root,
            } => {
                if Path::new(&expected_root) != root {
                    return Err("The repository changed while loading an image".into());
                }
                media::preview(&root, &path, staged, oid.as_deref())
            }
            Request::Diff { path, staged } => {
                let entry = files(&root)?
                    .into_iter()
                    .find(|f| f.path == path)
                    .ok_or("File no longer changed")?;
                if entry.index == '?' {
                    let full = root.join(&path);
                    let meta = std::fs::symlink_metadata(&full).map_err(|e| e.to_string())?;
                    if !meta.is_file() || meta.len() > 2_000_000 {
                        return Ok(
                            json!({"patch":"","notice":"Preview unavailable. Stage the file to review its Git diff."}),
                        );
                    }
                    let b = std::fs::read(full).map_err(|e| e.to_string())?;
                    let content = String::from_utf8(b).ok().filter(|s| !s.contains('\0'));
                    return Ok(
                        json!({"patch":"","untracked":content,"notice":"Untracked file — stage the file to include it in your commit."}),
                    );
                }
                Ok(json!({"patch":diff(&root,&path,staged)?}))
            }
            Request::StageFile { path, staged } => {
                let entry = files(&root)?
                    .into_iter()
                    .find(|f| f.path == path)
                    .ok_or("File no longer changed")?;
                if staged {
                    let has_head = git(&root, &["rev-parse", "--verify", "HEAD"], None)?.0 == 0;
                    let mut args = if has_head {
                        vec!["reset", "-q", "HEAD", "--"]
                    } else {
                        vec!["rm", "--cached", "-q", "--"]
                    };
                    args.push(&path);
                    if let Some(old) = &entry.old_path {
                        args.push(old);
                    }
                    run(&root, &args, None)?;
                } else {
                    let mut args = vec!["add", "--", &path];
                    if let Some(old) = &entry.old_path {
                        args.push(old);
                    }
                    run(&root, &args, None)?;
                }
                self.status()
            }
            Request::StageHunk {
                path,
                staged,
                patch,
                hunk,
            } => {
                let current = diff(&root, &path, staged)?;
                if current != patch {
                    return Err(
                        "The file changed since this diff was opened. Refresh before staging."
                            .into(),
                    );
                }
                let selected = hunk_patch(&current, hunk)?;
                let mut args = vec!["apply", "--cached", "--whitespace=nowarn"];
                if staged {
                    args.push("--reverse");
                }
                run(&root, &args, Some(selected.as_bytes()))?;
                self.status()
            }
            Request::Commit { message, expected } => {
                if message.trim().is_empty() {
                    return Err("Enter a commit message".into());
                }
                let actual = staged_token(&root)?;
                if actual.is_empty() {
                    return Err("Stage changes before committing".into());
                }
                if expected != actual {
                    return Err(
                        "Staged changes changed since your last review. Refresh before committing."
                            .into(),
                    );
                }
                run(
                    &root,
                    &[
                        "-c",
                        "core.commentChar=#",
                        "commit",
                        "--cleanup=whitespace",
                        "-F",
                        "-",
                    ],
                    Some(message.as_bytes()),
                )?;
                self.status()
            }
            Request::History { skip, all } => {
                if operations::head(&root).is_empty() {
                    return Ok(json!([]));
                }
                let skip_arg = format!("--skip={skip}");
                let mut args = vec![
                    "log",
                    "--no-show-signature",
                    "--topo-order",
                    "-n",
                    "100",
                    &skip_arg,
                    "--format=%H%x00%h%x00%an%x00%at%x00%s%x00%P%x00%D%x00",
                    "HEAD",
                ];
                if all {
                    args.extend(["--branches", "--remotes", "--tags"]);
                }
                let raw = text(run(&root, &args, None)?)?;
                let parts: Vec<_> = raw.split('\0').collect();
                let rows: Vec<_> = parts.chunks(7).filter(|r|r.len()==7).map(|r|json!({"oid":r[0].trim(),"short":r[1],"author":r[2],"timestamp":r[3].parse::<i64>().unwrap_or(0),"subject":r[4],"parents":r[5].split_whitespace().collect::<Vec<_>>(),"refs":r[6]})).collect();
                Ok(json!(rows))
            }
            Request::CommitFiles { oid } => {
                validate_oid(&oid)?;
                let raw = run(
                    &root,
                    &[
                        "diff-tree",
                        "--root",
                        "--first-parent",
                        "-m",
                        "--no-commit-id",
                        "--name-only",
                        "-r",
                        "-z",
                        "--no-renames",
                        &oid,
                    ],
                    None,
                )?;
                let paths: Result<Vec<_>, _> = raw
                    .split(|b| *b == 0)
                    .filter(|p| !p.is_empty())
                    .map(|p| text(p.to_vec()))
                    .collect();
                Ok(json!(paths?))
            }
            Request::CommitDiff { oid, path } => {
                validate_oid(&oid)?;
                let mut args = vec![
                    "show",
                    "--format=",
                    "--first-parent",
                    "--no-ext-diff",
                    "--no-textconv",
                    "--no-color",
                    "--no-renames",
                ];
                if !media::raster(&path) {
                    args.push("--binary");
                }
                args.extend([oid.as_str(), "--", path.as_str()]);
                let patch = text(run(&root, &args, None)?)?;
                Ok(json!({"patch":patch}))
            }
        }
    }
    fn status(&self) -> Result<Value, String> {
        self.status_details(true)
    }
    fn status_details(&self, complete: bool) -> Result<Value, String> {
        let root = self.root.as_ref().ok_or("Open a repository first")?;
        let revision = self.watch.revision();
        let branch = git(root, &["symbolic-ref", "--quiet", "--short", "HEAD"], None)?;
        let branch = if branch.0 == 0 {
            text(branch.1)?.trim().to_string()
        } else {
            text(run(root, &["rev-parse", "--short", "HEAD"], None)?)?
                .trim()
                .to_string()
        };
        let changed_files = files(root)?;
        let line_stats = if complete {
            Some(line_stats::count(root, &changed_files)?)
        } else {
            None
        };
        let staged = if complete {
            staged_token(root)?
        } else {
            String::new()
        };
        Ok(
            json!({"root":root,"name":root.file_name().unwrap_or_default().to_string_lossy(),"branch":branch,"files":changed_files,"lineStats":line_stats,"stagedToken":staged,"detailsPending":!complete,"revision":revision,"watching":self.watch.healthy(),"head":operations::head(root),"stateToken":operations::snapshot(root),"branchRef":operations::branch_ref(root),"remotes":operations::remotes(root)?,"tracking":operations::tracking(root),"operation":operations::operation(root)}),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    fn fixture() -> (tempfile::TempDir, Service) {
        let dir = tempfile::tempdir().unwrap();
        run(dir.path(), &["init", "-b", "main"], None).unwrap();
        run(
            dir.path(),
            &["config", "user.name", "Branchline Test"],
            None,
        )
        .unwrap();
        run(
            dir.path(),
            &["config", "user.email", "test@example.invalid"],
            None,
        )
        .unwrap();
        run(dir.path(), &["config", "commit.gpgsign", "false"], None).unwrap();
        let mut service = Service::default();
        service
            .handle(Request::Open {
                path: dir.path().to_string_lossy().into_owned(),
            })
            .unwrap();
        (dir, service)
    }
    #[test]
    fn status_counts_staged_unstaged_untracked_and_binary_changes() {
        let (dir, mut service) = fixture();
        baseline(dir.path());
        modify(dir.path());
        run(dir.path(), &["add", "sample.txt"], None).unwrap();
        let content = fs::read_to_string(dir.path().join("sample.txt")).unwrap();
        fs::write(dir.path().join("sample.txt"), format!("{content}extra\n")).unwrap();
        fs::write(dir.path().join("new.txt"), "alpha\nbeta").unwrap();
        fs::write(dir.path().join("image.bin"), b"\0binary").unwrap();
        let status = service.handle(Request::Status).unwrap();
        assert_eq!(status["files"].as_array().unwrap().len(), 3);
        assert_eq!(status["lineStats"]["additions"], 5);
        assert_eq!(status["lineStats"]["deletions"], 2);
        assert_eq!(status["lineStats"]["binaryFiles"], 1);
        let (dir, mut service) = fixture();
        fs::write(dir.path().join("first.txt"), "first\nsecond").unwrap();
        let status = service.handle(Request::Status).unwrap();
        assert_eq!(status["lineStats"]["additions"], 2);
        assert_eq!(status["lineStats"]["deletions"], 0);
    }
    #[test]
    fn quick_open_defers_expensive_details_without_granting_commit_authority() {
        let (dir, mut service) = fixture();
        baseline(dir.path());
        modify(dir.path());
        run(dir.path(), &["add", "sample.txt"], None).unwrap();
        let quick = service
            .handle(Request::OpenQuick {
                path: dir.path().to_string_lossy().into(),
            })
            .unwrap();
        assert_eq!(quick["detailsPending"], true);
        assert_eq!(quick["stagedToken"], "");
        assert!(quick["lineStats"].is_null());
        assert_eq!(quick["files"].as_array().unwrap().len(), 1);
        let full = service
            .handle(Request::StatusDetails {
                expected_root: quick["root"].as_str().unwrap().into(),
            })
            .unwrap();
        assert_eq!(full["detailsPending"], false);
        assert!(!full["stagedToken"].as_str().unwrap().is_empty());
        assert!(service
            .handle(Request::StatusDetails {
                expected_root: "/another/repo".into()
            })
            .is_err());
    }
    #[test]
    fn image_and_source_previews_use_the_correct_git_versions_and_reject_external_paths() {
        let (dir, mut service) = fixture();
        let root = dir.path();
        let before = "<svg xmlns=\"http://www.w3.org/2000/svg\"><rect width=\"1\"/></svg>";
        fs::write(root.join("icon.svg"), before).unwrap();
        run(root, &["add", "icon.svg"], None).unwrap();
        run(root, &["commit", "-m", "Initial image"], None).unwrap();
        let after = before.replace("width=\"1\"", "width=\"2\"");
        fs::write(root.join("icon.svg"), &after).unwrap();
        run(root, &["add", "icon.svg"], None).unwrap();
        let expected_root = service.root.as_ref().unwrap().to_string_lossy().to_string();
        let preview = service
            .handle(Request::ImagePreview {
                path: "icon.svg".into(),
                staged: true,
                oid: None,
                expected_root: expected_root.clone(),
            })
            .unwrap();
        assert!(preview["before"]["dataUrl"]
            .as_str()
            .unwrap()
            .starts_with("data:image/svg+xml;base64,"));
        assert_ne!(preview["before"], preview["after"]);
        let source = service
            .handle(Request::SourcePreview {
                path: "icon.svg".into(),
                staged: true,
                oid: None,
                expected_root: expected_root.clone(),
            })
            .unwrap();
        assert_eq!(source["before"], before);
        assert_eq!(source["after"], after);
        run(root, &["commit", "-m", "Change image"], None).unwrap();
        let oid = operations::head(root);
        let committed = service
            .handle(Request::SourcePreview {
                path: "icon.svg".into(),
                staged: false,
                oid: Some(oid),
                expected_root: expected_root.clone(),
            })
            .unwrap();
        assert_eq!(committed, source);
        fs::write(root.join("pixel.png"), b"\x89PNG\r\n\x1a\n\0test").unwrap();
        run(root, &["add", "pixel.png"], None).unwrap();
        let raster = service
            .handle(Request::Diff {
                path: "pixel.png".into(),
                staged: true,
            })
            .unwrap();
        assert!(raster["patch"].as_str().unwrap().contains("Binary files"));
        assert!(!raster["patch"]
            .as_str()
            .unwrap()
            .contains("GIT binary patch"));
        run(root, &["commit", "-m", "Add pixel"], None).unwrap();
        let raster = service
            .handle(Request::CommitDiff {
                oid: operations::head(root),
                path: "pixel.png".into(),
            })
            .unwrap();
        assert!(raster["patch"].as_str().unwrap().contains("Binary files"));

        let outside = tempfile::NamedTempFile::new().unwrap();
        std::os::unix::fs::symlink(outside.path(), root.join("outside.svg")).unwrap();
        let blocked = service
            .handle(Request::ImagePreview {
                path: "outside.svg".into(),
                staged: false,
                oid: None,
                expected_root: expected_root.clone(),
            })
            .unwrap();
        assert!(blocked["after"].is_null());
        assert!(service
            .handle(Request::SourcePreview {
                path: "../outside".into(),
                staged: false,
                oid: None,
                expected_root
            })
            .is_err());
    }
    fn baseline(dir: &Path) {
        fs::write(
            dir.join("sample.txt"),
            (1..=30).map(|i| format!("line {i}\n")).collect::<String>(),
        )
        .unwrap();
        run(dir, &["add", "--", "sample.txt"], None).unwrap();
        run(dir, &["commit", "-m", "Baseline"], None).unwrap();
    }
    fn modify(dir: &Path) {
        let original = fs::read_to_string(dir.join("sample.txt")).unwrap();
        fs::write(
            dir.join("sample.txt"),
            original
                .replace("line 2\n", "changed 2\n")
                .replace("line 25\n", "changed 25\n"),
        )
        .unwrap();
    }
    #[test]
    fn stage_one_hunk_commit_and_read_history() {
        let (dir, mut service) = fixture();
        baseline(dir.path());
        modify(dir.path());
        let patch = diff(dir.path(), "sample.txt", false).unwrap();
        assert_eq!(patch.matches("\n@@ ").count(), 2);
        service
            .handle(Request::StageHunk {
                path: "sample.txt".into(),
                staged: false,
                patch,
                hunk: 0,
            })
            .unwrap();
        let staged = staged_patch(dir.path()).unwrap();
        assert!(staged.contains("+changed 2"));
        assert!(!staged.contains("+changed 25"));
        let working = diff(dir.path(), "sample.txt", false).unwrap();
        assert!(working.contains("+changed 25"));
        service
            .handle(Request::Commit {
                message: "Partial change".into(),
                expected: staged_token(dir.path()).unwrap(),
            })
            .unwrap();
        let history = service
            .handle(Request::History {
                skip: 0,
                all: false,
            })
            .unwrap();
        assert_eq!(history.as_array().unwrap().len(), 2);
        assert_eq!(history[0]["subject"], "Partial change");
        let oid = history[0]["oid"].as_str().unwrap().to_string();
        let paths = service
            .handle(Request::CommitFiles { oid: oid.clone() })
            .unwrap();
        assert_eq!(paths[0], "sample.txt");
        let result = service
            .handle(Request::CommitDiff {
                oid,
                path: "sample.txt".into(),
            })
            .unwrap();
        assert!(result["patch"].as_str().unwrap().contains("+changed 2"));
        assert!(diff(dir.path(), "sample.txt", false)
            .unwrap()
            .contains("+changed 25"));
    }
    #[test]
    fn stale_hunks_and_stale_commits_are_rejected() {
        let (dir, mut service) = fixture();
        baseline(dir.path());
        modify(dir.path());
        let patch = diff(dir.path(), "sample.txt", false).unwrap();
        fs::write(dir.path().join("sample.txt"), "External edit\n").unwrap();
        assert!(service
            .handle(Request::StageHunk {
                path: "sample.txt".into(),
                staged: false,
                patch,
                hunk: 0
            })
            .unwrap_err()
            .contains("changed since"));
        service
            .handle(Request::StageFile {
                path: "sample.txt".into(),
                staged: false,
            })
            .unwrap();
        assert!(service
            .handle(Request::Commit {
                message: "Nope".into(),
                expected: "stale".into()
            })
            .unwrap_err()
            .contains("last review"));
        assert_eq!(
            text(run(dir.path(), &["log", "-1", "--format=%s"], None).unwrap())
                .unwrap()
                .trim(),
            "Baseline"
        );
    }
    #[test]
    fn unstage_hunk_leaves_worktree_intact() {
        let (dir, mut service) = fixture();
        baseline(dir.path());
        modify(dir.path());
        service
            .handle(Request::StageFile {
                path: "sample.txt".into(),
                staged: false,
            })
            .unwrap();
        let original = fs::read(dir.path().join("sample.txt")).unwrap();
        let patch = diff(dir.path(), "sample.txt", true).unwrap();
        service
            .handle(Request::StageHunk {
                path: "sample.txt".into(),
                staged: true,
                patch,
                hunk: 1,
            })
            .unwrap();
        assert_eq!(fs::read(dir.path().join("sample.txt")).unwrap(), original);
        assert!(!staged_patch(dir.path()).unwrap().contains("+changed 25"));
        assert!(staged_patch(dir.path()).unwrap().contains("+changed 2"));
    }
    #[test]
    fn unborn_repository_unicode_names_and_literal_pathspecs() {
        let (dir, mut service) = fixture();
        let name = "雪 [a] space.txt";
        fs::write(dir.path().join(name), "hello\n").unwrap();
        assert_eq!(files(dir.path()).unwrap()[0].path, name);
        assert_eq!(
            service
                .handle(Request::History {
                    skip: 0,
                    all: false
                })
                .unwrap(),
            json!([])
        );
        service
            .handle(Request::StageFile {
                path: name.into(),
                staged: false,
            })
            .unwrap();
        service
            .handle(Request::StageFile {
                path: name.into(),
                staged: true,
            })
            .unwrap();
        assert!(staged_patch(dir.path()).unwrap().is_empty());
        service
            .handle(Request::StageFile {
                path: name.into(),
                staged: false,
            })
            .unwrap();
        service
            .handle(Request::Commit {
                message: "Initial".into(),
                expected: staged_token(dir.path()).unwrap(),
            })
            .unwrap();
        let history = service
            .handle(Request::History {
                skip: 0,
                all: false,
            })
            .unwrap();
        let oid = history[0]["oid"].as_str().unwrap().to_string();
        assert_eq!(
            service
                .handle(Request::CommitFiles { oid: oid.clone() })
                .unwrap()[0],
            name
        );
        assert!(service
            .handle(Request::CommitDiff {
                oid,
                path: name.into()
            })
            .unwrap()["patch"]
            .as_str()
            .unwrap()
            .contains("+hello"));
    }
    #[test]
    fn failing_hook_preserves_index_and_message_can_be_retried() {
        use std::os::unix::fs::PermissionsExt;
        let (dir, mut service) = fixture();
        baseline(dir.path());
        modify(dir.path());
        service
            .handle(Request::StageFile {
                path: "sample.txt".into(),
                staged: false,
            })
            .unwrap();
        let hook = dir.path().join(".git/hooks/pre-commit");
        fs::write(
            &hook,
            "#!/bin/sh\necho 'Hook rejected commit' >&2\nexit 1\n",
        )
        .unwrap();
        fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
        let before = staged_patch(dir.path()).unwrap();
        assert!(service
            .handle(Request::Commit {
                message: "Rejected".into(),
                expected: staged_token(dir.path()).unwrap()
            })
            .unwrap_err()
            .contains("Hook rejected"));
        assert_eq!(before, staged_patch(dir.path()).unwrap());
    }
    #[test]
    fn rename_and_binary_use_file_level_staging() {
        let (dir, mut service) = fixture();
        baseline(dir.path());
        run(dir.path(), &["mv", "sample.txt", "renamed.txt"], None).unwrap();
        let f = files(dir.path()).unwrap();
        assert_eq!(f[0].old_path.as_deref(), Some("sample.txt"));
        service
            .handle(Request::StageFile {
                path: "renamed.txt".into(),
                staged: true,
            })
            .unwrap();
        assert!(staged_patch(dir.path()).unwrap().is_empty());
        fs::write(dir.path().join("binary.bin"), [0, 1, 255, 0]).unwrap();
        service
            .handle(Request::StageFile {
                path: "binary.bin".into(),
                staged: false,
            })
            .unwrap();
        assert!(staged_patch(dir.path())
            .unwrap()
            .contains("GIT binary patch"));
    }
    #[test]
    fn no_newline_hunk_roundtrips() {
        let (dir, mut service) = fixture();
        baseline(dir.path());
        fs::write(dir.path().join("sample.txt"), "no newline").unwrap();
        let patch = diff(dir.path(), "sample.txt", false).unwrap();
        service
            .handle(Request::StageHunk {
                path: "sample.txt".into(),
                staged: false,
                patch,
                hunk: 0,
            })
            .unwrap();
        assert!(diff(dir.path(), "sample.txt", false).unwrap().is_empty());
        let patch = diff(dir.path(), "sample.txt", true).unwrap();
        service
            .handle(Request::StageHunk {
                path: "sample.txt".into(),
                staged: true,
                patch,
                hunk: 0,
            })
            .unwrap();
        assert!(staged_patch(dir.path()).unwrap().is_empty());
    }
}
