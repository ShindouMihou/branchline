//! GitHub provider: fixed CLI operations, explicit repository and head; never a shell.
use super::*;
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub repository: String,
    pub head: String,
    pub base: String,
    pub title: String,
    pub body: String,
    pub draft: bool,
    pub cli: String,
    pub cli_identity: String,
}
fn repository(url: &str) -> Result<String, String> {
    let path = url.strip_prefix("https://github.com/")
        .or_else(|| url.strip_prefix("git@github.com:"))
        .ok_or("Pull requests currently support github.com HTTPS or SSH remotes. Choose a GitHub remote; local remotes cannot host pull requests.")?;
    let path = path.strip_suffix(".git").unwrap_or(path);
    let parts: Vec<_> = path.split('/').collect();
    if parts.len() != 2
        || parts.iter().any(|p| {
            p.is_empty()
                || *p == "."
                || *p == ".."
                || !p
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        })
    {
        return Err("Use a GitHub remote with one owner and repository name.".into());
    }
    Ok(path.to_string())
}
fn cli() -> Result<(String, String), String> {
    let mut candidates = Vec::new();
    if let Some(paths) = std::env::var_os("PATH") {
        for path in std::env::split_paths(&paths).filter(|p| p.is_absolute()) {
            candidates.push(path.join("gh"));
        }
    }
    for path in ["/opt/homebrew/bin/gh", "/usr/local/bin/gh"] {
        candidates.push(path.into());
    }
    if let Some(home) = std::env::var_os("HOME") {
        candidates.push(PathBuf::from(home).join(".local/bin/gh"));
    }
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    for path in candidates {
        if let Ok(path) = std::fs::canonicalize(path) {
            if let Ok(m) = std::fs::metadata(&path) {
                if m.is_file() && m.permissions().mode() & 0o111 != 0 {
                    return Ok((
                        path.to_string_lossy().into(),
                        format!(
                            "{}:{}:{}:{}:{}",
                            m.dev(),
                            m.ino(),
                            m.len(),
                            m.mtime(),
                            m.mtime_nsec()
                        ),
                    ));
                }
            }
        }
    }
    Err("Install GitHub CLI (gh), then sign in with gh auth login before requesting a pull request.".into())
}
pub fn preview(root: &Path, step: &Step, push: &PushPreview) -> Result<Preview, String> {
    let Step::PullRequest {
        head,
        base,
        title,
        body,
        draft,
        ..
    } = step
    else {
        return Err("Missing pull request step".into());
    };
    let repository = repository(&push.destination)?;
    // Base must be locally known; the source may be a new branch created by this plan.
    let base_ref = format!("refs/remotes/{}/{}", push.remote, base);
    if git(root, &["rev-parse", "--verify", &base_ref], None)?.0 != 0 {
        return Err("Fetch the pull request target branch in Branchline first so it can be shown accurately.".into());
    }
    let (cli, cli_identity) = cli()?;
    Ok(Preview {
        repository,
        head: head.clone(),
        base: base.clone(),
        title: title.clone(),
        body: body.clone(),
        draft: *draft,
        cli,
        cli_identity,
    })
}
fn run_cli(
    root: &Path,
    p: &Preview,
    args: &[String],
    input: Option<&[u8]>,
) -> Result<Vec<u8>, String> {
    use std::os::unix::process::CommandExt;
    let mut cmd = Command::new(&p.cli);
    cmd.process_group(0);
    cmd.current_dir(root)
        .args(args)
        .env("GH_PROMPT_DISABLED", "1")
        .env("GH_PAGER", "cat")
        .env("GH_HOST", "github.com")
        .env_remove("GH_REPO")
        .env_remove("GH_DEBUG");
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| {
            "Cannot start GitHub CLI. Install gh and sign in with gh auth login.".to_string()
        })?;
    let mut stdout = child.stdout.take().unwrap();
    let reader = std::thread::spawn(move || {
        let mut result = Vec::new();
        let mut chunk = [0u8; 8192];
        loop {
            let n = stdout.read(&mut chunk).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            if result.len() < 2_000_000 {
                result.extend_from_slice(&chunk[..n]);
            }
        }
        Ok::<_, String>(result)
    });
    let mut stdin = child.stdin.take().unwrap();
    let bytes = input.unwrap_or_default().to_vec();
    let writer = std::thread::spawn(move || stdin.write_all(&bytes));
    let status = child
        .wait_timeout(Duration::from_secs(60))
        .map_err(|_| "GitHub CLI status unavailable")?;
    let status = match status {
        Some(s) => s,
        None => {
            unsafe {
                libc::kill(-(child.id() as i32), libc::SIGKILL);
            }
            let _ = child.wait();
            return Err("GitHub CLI timed out. Inspect GitHub before retrying; the pull request may already exist.".into());
        }
    };
    let result = reader.join().map_err(|_| "GitHub output unavailable")??;
    let _ = writer.join();
    if !status.success() {
        return Err("GitHub CLI did not confirm completion. Check authentication with gh auth status and inspect GitHub before retrying. Raw output is omitted to protect credentials.".into());
    }
    Ok(result)
}
fn checked(
    root: &Path,
    p: &Preview,
    args: &[String],
    input: Option<&[u8]>,
    log: &mut CommandLog<'_>,
) -> Result<Vec<u8>, String> {
    log(&p.cli, args, input, false, None)?;
    let result = run_cli(root, p, args, input);
    log(
        &p.cli,
        args,
        input,
        true,
        Some(if result.is_ok() {
            "succeeded"
        } else {
            "failed_or_uncertain"
        }),
    )?;
    result
}
fn api(
    root: &Path,
    p: &Preview,
    path: &str,
    fields: &[(&str, &str)],
    log: &mut CommandLog<'_>,
) -> Result<Value, String> {
    let mut args = vec![
        "api".into(),
        "--hostname".into(),
        "github.com".into(),
        "--method".into(),
        "GET".into(),
        path.into(),
    ];
    for (key, value) in fields {
        args.push("-f".into());
        args.push(format!("{key}={value}"));
    }
    serde_json::from_slice(&checked(root, p, &args, None, log)?).map_err(|_| {
        "GitHub returned an unreadable response. Inspect GitHub before retrying.".into()
    })
}
fn encoded(value: &str) -> String {
    value
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}
pub fn execute(
    root: &Path,
    step: &Step,
    p: &Preview,
    source: &str,
    log: &mut CommandLog<'_>,
) -> Result<Value, String> {
    let Step::PullRequest {
        remote,
        head,
        base,
        title,
        body,
        draft,
    } = step
    else {
        return Err("Missing pull request step".into());
    };
    let urls = txt(root, &["remote", "get-url", "--push", "--all", remote])?;
    let (binary, identity) = cli()?;
    if repository(&safe_url(&urls)?)? != p.repository
        || head != &p.head
        || base != &p.base
        || title != &p.title
        || body != &p.body
        || draft != &p.draft
        || binary != p.cli
        || identity != p.cli_identity
    {
        return Err(
            "The GitHub destination, request details or CLI changed. Request a new review.".into(),
        );
    }
    create_and_verify(root, p, source, log)
}
fn create_and_verify(
    root: &Path,
    p: &Preview,
    source: &str,
    log: &mut CommandLog<'_>,
) -> Result<Value, String> {
    let (head, base, title, body, draft) = (&p.head, &p.base, &p.title, &p.body, &p.draft);
    let ref_path = format!("repos/{}/git/ref/heads/{}", p.repository, encoded(head));
    let remote_head = api(root, p, &ref_path, &[], log)?;
    if remote_head["object"]["sha"].as_str() != Some(source) {
        return Err(
            "The published branch changed. No pull request was created; request a new review."
                .into(),
        );
    }
    let _ = api(
        root,
        p,
        &format!("repos/{}/git/ref/heads/{}", p.repository, encoded(base)),
        &[],
        log,
    )?;
    let owner = p.repository.split('/').next().unwrap();
    let head_filter = format!("{owner}:{head}");
    let existing = api(
        root,
        p,
        &format!("repos/{}/pulls", p.repository),
        &[("state", "open"), ("head", &head_filter), ("base", base)],
        log,
    )?;
    if !existing.as_array().is_some_and(|a| a.is_empty()) {
        return Err("A pull request may already exist for these branches. Inspect GitHub; Branchline will not create a duplicate.".into());
    }
    let mut args = vec![
        "pr".into(),
        "create".into(),
        "--repo".into(),
        format!("github.com/{}", p.repository),
        "--head".into(),
        head.clone(),
        "--base".into(),
        base.clone(),
        "--title".into(),
        title.clone(),
        "--body-file".into(),
        "-".into(),
        "--no-maintainer-edit".into(),
    ];
    if *draft {
        args.push("--draft".into());
    }
    let output = checked(root, p, &args, Some(body.as_bytes()), log)?;
    let url = String::from_utf8(output)
        .map_err(|_| "Pull request creation result unreadable; inspect GitHub before retrying.")?;
    let url = url.trim();
    let prefix = format!("https://github.com/{}/pull/", p.repository);
    let number=url.strip_prefix(&prefix).filter(|n|!n.is_empty()&&n.bytes().all(|c|c.is_ascii_digit())).ok_or("Pull request may have been created, but its URL could not be verified. Inspect GitHub before retrying.")?;
    let actual = api(
        root,
        p,
        &format!("repos/{}/pulls/{number}", p.repository),
        &[],
        log,
    )?;
    if actual["head"]["sha"].as_str() != Some(source)
        || actual["head"]["ref"] != *head
        || actual["base"]["ref"] != *base
        || actual["head"]["repo"]["full_name"] != p.repository
        || actual["base"]["repo"]["full_name"] != p.repository
        || actual["title"] != *title
        || actual["body"].as_str().unwrap_or("") != body
        || actual["draft"] != *draft
        || actual["state"] != "open"
        || actual["html_url"] != url
    {
        return Err("A pull request was created, but its details differ from the approved plan. Inspect GitHub; do not retry automatically.".into());
    }
    Ok(
        json!({"pullRequestUrl":url,"number":actual["number"],"repository":p.repository,"head":head,"base":base,"draft":draft}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_repository_destination_and_encodes_branch_paths() {
        assert_eq!(
            repository("git@github.com:owner/project.git").unwrap(),
            "owner/project"
        );
        assert_eq!(
            repository("https://github.com/owner/project.git").unwrap(),
            "owner/project"
        );
        for url in [
            "https://github.com.evil/owner/project",
            "https://user@github.com/owner/project",
            "/tmp/remote.git",
            "https://github.com/owner/../other",
            "https://github.com/owner/repo?token=x",
        ] {
            assert!(repository(url).is_err());
        }
        assert_eq!(encoded("feat/test#1"), "feat%2Ftest%231");
    }
}
#[cfg(test)]
mod integration_tests {
    use super::*;
    fn fixture(scenario: &str) -> (tempfile::TempDir, Preview) {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("gh");
        std::fs::write(&script,r#"#!/usr/bin/python3
import json,sys,pathlib
args=sys.argv[1:]; root=pathlib.Path.cwd(); scenario=(root/'scenario').read_text(); body=sys.stdin.read()
with (root/'calls.jsonl').open('a') as f: f.write(json.dumps({'args':args,'body':body})+'\n')
if args[0]=='pr':
    if scenario=='uncertain': sys.exit(1)
    print('https://github.com/owner/project/pull/42'); sys.exit()
path=args[5]
if '/git/ref/' in path: print(json.dumps({'object':{'sha':'b'*40 if scenario=='head-moved' else 'a'*40}}))
elif path.endswith('/pulls'): print('[{}]' if scenario=='duplicate' else '[]')
else: print(json.dumps({'number':42,'html_url':'https://github.com/owner/project/pull/42','state':'open','draft':True,'title':'Feature','body':'wrong' if scenario=='bad-body' else 'Line 1\nLine 2','head':{'sha':'a'*40,'ref':'feat/test','repo':{'full_name':'owner/project'}},'base':{'ref':'main','repo':{'full_name':'owner/project'}}}))
"#).unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::write(dir.path().join("scenario"), scenario).unwrap();
        let p = Preview {
            repository: "owner/project".into(),
            head: "feat/test".into(),
            base: "main".into(),
            title: "Feature".into(),
            body: "Line 1\nLine 2".into(),
            draft: true,
            cli: script.to_string_lossy().into(),
            cli_identity: "fixture".into(),
        };
        (dir, p)
    }
    #[test]
    fn creates_and_verifies_exact_pr_with_noninteractive_arguments_and_program_audit() {
        let (dir, p) = fixture("success");
        let mut commands = Vec::new();
        let result = create_and_verify(
            dir.path(),
            &p,
            &"a".repeat(40),
            &mut |program, argv, input, finished, _| {
                if !finished {
                    commands.push((
                        program.to_string(),
                        argv.to_vec(),
                        input.map(|x| x.to_vec()),
                    ))
                }
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(
            result["pullRequestUrl"],
            "https://github.com/owner/project/pull/42"
        );
        let (_, args, body) = commands.iter().find(|(_, a, _)| a[0] == "pr").unwrap();
        assert!(args.windows(2).any(|a| a == ["--head", "feat/test"]));
        assert!(args
            .windows(2)
            .any(|a| a == ["--repo", "github.com/owner/project"]));
        assert!(args.contains(&"--draft".into()));
        assert_eq!(body.as_deref(), Some(p.body.as_bytes()));
        assert!(commands.iter().all(|c| c.0 == p.cli));
    }
    #[test]
    fn changed_heads_duplicates_and_uncertain_creations_stop_without_retry() {
        for scenario in ["head-moved", "duplicate", "uncertain", "bad-body"] {
            let (dir, p) = fixture(scenario);
            let mut attempts = 0;
            let result = create_and_verify(
                dir.path(),
                &p,
                &"a".repeat(40),
                &mut |_, argv, _, finished, _| {
                    if !finished && argv[0] == "pr" {
                        attempts += 1
                    }
                    Ok(())
                },
            );
            assert!(result.is_err(), "{scenario}");
            assert_eq!(
                attempts,
                if ["head-moved", "duplicate"].contains(&scenario) {
                    0
                } else {
                    1
                }
            );
        }
    }
}
