use super::CommandLog;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    os::{
        fd::AsRawFd,
        unix::{
            fs::{MetadataExt, PermissionsExt},
            process::CommandExt,
        },
    },
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

const OUTPUT_LIMIT: usize = 131_072;
const INPUT_LIMIT: usize = 1_048_576;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CommandStep {
    pub program: String,
    pub argv: Vec<String>,
    #[serde(default)]
    pub stdin: Option<String>,
    #[serde(default)]
    pub timeout_seconds: Option<u64>,
}

impl CommandStep {
    pub fn validate(&self) -> Result<(), String> {
        if !matches!(self.program.as_str(), "git" | "gh" | "shell") {
            return Err("Choose git, gh, or shell as the command program.".into());
        }
        if self.program == "shell"
            && (self.argv.len() != 2 || self.argv[0] != "-c" || self.argv[1].trim().is_empty())
        {
            return Err("Shell commands require exactly -c and the unchanged command text.".into());
        }
        if self.argv.is_empty()
            || self.argv.len() > 256
            || self.argv.iter().any(|arg| arg.contains('\0'))
            || self.argv.iter().map(String::len).sum::<usize>() > 131_072
            || self.stdin.as_ref().is_some_and(|s| s.len() > INPUT_LIMIT)
            || self
                .timeout_seconds
                .is_some_and(|s| !(1..=600).contains(&s))
        {
            return Err("Command arguments, input, or timeout exceed the supported limits.".into());
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ExecutableBinding {
    pub program: String,
    pub path: String,
    pub sha256: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub root: String,
    pub repository_identity: String,
    pub root_device: u64,
    pub root_inode: u64,
    pub executables: Vec<ExecutableBinding>,
}

fn executable_hash(path: &Path) -> Result<String, String> {
    let metadata =
        fs::metadata(path).map_err(|_| "The reviewed command executable is unavailable.")?;
    if !metadata.is_file()
        || metadata.permissions().mode() & 0o111 == 0
        || metadata.len() > 268_435_456
    {
        return Err("The command program is not an executable file.".into());
    }
    let mut file = fs::File::open(path)
        .map_err(|_| "Cannot read the command executable.")?
        .take(268_435_457);
    let mut digest = Sha256::new();
    let mut buffer = [0; 65_536];
    let mut read = 0;
    loop {
        let n = file
            .read(&mut buffer)
            .map_err(|_| "Cannot fingerprint the command executable.")?;
        if n == 0 {
            break;
        }
        read += n;
        if read > 268_435_456 {
            return Err("Command executable changed while fingerprinting.".into());
        }
        digest.update(&buffer[..n]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn resolve(program: &str) -> Result<ExecutableBinding, String> {
    if program == "shell" {
        let path = fs::canonicalize("/bin/sh").map_err(|_| "The system shell is unavailable.")?;
        let sha256 = executable_hash(&path)?;
        return Ok(ExecutableBinding {
            program: program.into(),
            path: path.to_string_lossy().into_owned(),
            sha256,
        });
    }
    let mut paths =
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()).collect::<Vec<_>>();
    paths.extend(["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"].map(PathBuf::from));
    for directory in paths.into_iter().filter(|p| p.is_absolute()) {
        let Ok(path) = fs::canonicalize(directory.join(program)) else {
            continue;
        };
        let Ok(sha256) = executable_hash(&path) else {
            continue;
        };
        return Ok(ExecutableBinding {
            program: program.into(),
            path: path.to_string_lossy().into_owned(),
            sha256,
        });
    }
    Err(format!("Install {program} before requesting this command."))
}

pub fn preview(root: &Path, steps: &[CommandStep]) -> Result<Manifest, String> {
    if steps.is_empty() || steps.len() > 64 {
        return Err("Use between 1 and 64 command steps.".into());
    }
    let root = fs::canonicalize(root).map_err(|_| "The repository directory is unavailable.")?;
    let metadata = fs::metadata(&root).map_err(|_| "Cannot identify the repository directory.")?;
    let mut executables = Vec::new();
    for step in steps {
        step.validate()?;
        if !executables
            .iter()
            .any(|binding: &ExecutableBinding| binding.program == step.program)
        {
            executables.push(resolve(&step.program)?);
        }
    }
    Ok(Manifest {
        repository_identity: super::repository_identity(&root)?,
        root: root.to_string_lossy().into_owned(),
        root_device: metadata.dev(),
        root_inode: metadata.ino(),
        executables,
    })
}

fn nonblocking(pipe: &impl AsRawFd) -> Result<(), String> {
    let fd = pipe.as_raw_fd();
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err("Cannot configure bounded command input/output.".into());
    }
    Ok(())
}

fn drain(pipe: &mut impl Read, output: &mut Vec<u8>, truncated: &mut bool) -> Result<(), String> {
    let mut chunk = [0; 8192];
    for _ in 0..32 {
        match pipe.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                let kept = n.min(OUTPUT_LIMIT.saturating_sub(output.len()));
                output.extend_from_slice(&chunk[..kept]);
                *truncated |= kept < n;
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => {
                return Err("Cannot read command output; effects may already have occurred.".into())
            }
        }
    }
    Ok(())
}

fn stop(child: &mut std::process::Child) {
    unsafe {
        libc::kill(-(child.id() as i32), libc::SIGKILL);
    }
    let _ = child.kill();
    let _ = child.wait();
}

fn run(root: &Path, executable: &Path, step: &CommandStep) -> Result<Value, String> {
    let mut command = Command::new(executable);
    command
        .current_dir(root)
        .args(&step.argv)
        .process_group(0)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GH_PROMPT_DISABLED", "1")
        .env("GIT_PAGER", "cat")
        .env("GH_PAGER", "cat")
        .env("PAGER", "cat")
        .env("GIT_EDITOR", "false")
        .env("GIT_SEQUENCE_EDITOR", "false");
    for key in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GH_REPO",
        "GH_DEBUG",
    ] {
        command.env_remove(key);
    }
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| "Cannot launch the reviewed command program.")?;
    let outcome = (|| {
        let mut stdout = child
            .stdout
            .take()
            .ok_or("Command output is unavailable.")?;
        let mut stderr = child
            .stderr
            .take()
            .ok_or("Command error output is unavailable.")?;
        let mut stdin = child.stdin.take();
        nonblocking(&stdout)?;
        nonblocking(&stderr)?;
        nonblocking(stdin.as_ref().ok_or("Command input is unavailable.")?)?;
        let input = step.stdin.as_deref().unwrap_or_default().as_bytes();
        let mut written = 0;
        let mut out = Vec::new();
        let mut err = Vec::new();
        let (mut out_truncated, mut err_truncated) = (false, false);
        let deadline = Instant::now() + Duration::from_secs(step.timeout_seconds.unwrap_or(120));
        let mut timed_out = false;
        let status = loop {
            drain(&mut stdout, &mut out, &mut out_truncated)?;
            drain(&mut stderr, &mut err, &mut err_truncated)?;
            if let Some(pipe) = stdin.as_mut() {
                if written < input.len() {
                    match pipe.write(&input[written..input.len().min(written + 8192)]) {
                        Ok(n) => written += n,
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                        Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => {
                            stdin.take();
                        }
                        Err(_) => {
                            return Err(
                                "Cannot send command input; effects may already have occurred."
                                    .into(),
                            )
                        }
                    }
                }
                if written == input.len() {
                    stdin.take();
                }
            }
            if let Some(status) = child
                .try_wait()
                .map_err(|_| "Cannot confirm command completion.")?
            {
                break status;
            }
            if Instant::now() >= deadline {
                timed_out = true;
                stop(&mut child);
                break child
                    .wait()
                    .map_err(|_| "Cannot confirm the timed-out command stopped.")?;
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        drain(&mut stdout, &mut out, &mut out_truncated)?;
        drain(&mut stderr, &mut err, &mut err_truncated)?;
        Ok(
            json!({"exitCode":status.code(),"processSucceeded":status.success()&&!timed_out,"timedOut":timed_out,"stdout":String::from_utf8_lossy(&out),"stderr":String::from_utf8_lossy(&err),"stdoutCapturedBytes":out.len(),"stderrCapturedBytes":err.len(),"stdoutCapturedDigest":format!("{:x}",Sha256::digest(&out)),"stderrCapturedDigest":format!("{:x}",Sha256::digest(&err)),"stdoutTruncated":out_truncated,"stderrTruncated":err_truncated,"effectsVerified":false,"verification":"unverified"}),
        )
    })();
    if outcome.is_err() {
        stop(&mut child);
    }
    outcome
}

pub fn execute(
    root: &Path,
    step: &CommandStep,
    manifest: &Manifest,
    log: &mut CommandLog<'_>,
) -> Result<Value, String> {
    step.validate()?;
    let root =
        fs::canonicalize(root).map_err(|_| "The reviewed working directory is unavailable.")?;
    let metadata =
        fs::metadata(&root).map_err(|_| "Cannot identify the reviewed working directory.")?;
    if root.to_string_lossy() != manifest.root
        || metadata.dev() != manifest.root_device
        || metadata.ino() != manifest.root_inode
    {
        return Err("The reviewed working directory was replaced. Request a new review.".into());
    }
    let binding = manifest
        .executables
        .iter()
        .find(|binding| binding.program == step.program)
        .ok_or("Missing reviewed executable binding.")?;
    if executable_hash(Path::new(&binding.path))? != binding.sha256 {
        return Err("The command executable changed after review. Request a new review.".into());
    }
    log(
        &binding.path,
        &step.argv,
        step.stdin.as_deref().map(str::as_bytes),
        false,
        None,
    )?;
    let result = run(&root, Path::new(&binding.path), step);
    let outcome = match &result {
        Ok(value) if value["processSucceeded"] == true => "process_succeeded_unverified",
        _ => "failed_or_uncertain",
    };
    log(
        &binding.path,
        &step.argv,
        step.stdin.as_deref().map(str::as_bytes),
        true,
        Some(outcome),
    )?;
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    fn step(program: &str, args: &[&str]) -> CommandStep {
        CommandStep {
            program: program.into(),
            argv: args.iter().map(|s| s.to_string()).collect(),
            stdin: None,
            timeout_seconds: None,
        }
    }
    #[test]
    fn accepts_broad_commands_but_rejects_invalid_envelopes() {
        for args in [
            vec!["add", "--", "file.txt"],
            vec!["-c", "alias.test=!echo hello", "test"],
            vec!["api", "graphql", "-f", "query=mutation {}"],
        ] {
            assert!(step("git", &args).validate().is_ok());
        }
        assert!(step("sh", &["-c", "true"]).validate().is_err());
        assert!(step("shell", &["-c", "git status && git diff"])
            .validate()
            .is_ok());
        assert!(step("shell", &["-c", ""]).validate().is_err());
        assert!(step("shell", &["-c", "git status", "extra"])
            .validate()
            .is_err());
        assert!(step("git", &["status\0"]).validate().is_err());
        let mut command = step("git", &["status"]);
        command.stdin = Some("file\0other-file\0".into());
        assert!(command.validate().is_ok());
        command.timeout_seconds = Some(0);
        assert!(command.validate().is_err());
    }
    #[test]
    fn command_output_is_bounded_and_never_claims_verified_effects() {
        let root = tempfile::tempdir().unwrap();
        let result = run(
            root.path(),
            Path::new("/usr/bin/printf"),
            &step("git", &["%s", "hello"]),
        )
        .unwrap();
        assert_eq!(result["stdout"], "hello");
        assert_eq!(result["processSucceeded"], true);
        assert_eq!(result["effectsVerified"], false);
        let mut bytes = Vec::new();
        let mut truncated = false;
        drain(
            &mut std::io::Cursor::new(vec![b'x'; OUTPUT_LIMIT + 10]),
            &mut bytes,
            &mut truncated,
        )
        .unwrap();
        assert_eq!(bytes.len(), OUTPUT_LIMIT);
        assert!(truncated);
    }
    #[test]
    fn timeout_does_not_wait_for_input() {
        let root = tempfile::tempdir().unwrap();
        let mut command = step("git", &["5"]);
        command.stdin = Some("x".repeat(INPUT_LIMIT));
        command.timeout_seconds = Some(1);
        let started = Instant::now();
        let result = run(root.path(), Path::new("/bin/sleep"), &command).unwrap();
        assert_eq!(result["timedOut"], true);
        assert_eq!(result["processSucceeded"], false);
        assert!(started.elapsed() < Duration::from_secs(3));
    }
    #[test]
    fn completed_process_does_not_wait_for_descendant_output_handles() {
        let root = tempfile::tempdir().unwrap();
        let command = step("git", &["-c", "sleep 2 & printf done"]);
        let started = Instant::now();
        let result = run(root.path(), Path::new("/bin/sh"), &command).unwrap();
        assert_eq!(result["stdout"], "done");
        assert_eq!(result["processSucceeded"], true);
        assert!(started.elapsed() < Duration::from_secs(1));
    }
    #[test]
    fn stages_literal_files_with_a_reviewed_executable() {
        let root = tempfile::tempdir().unwrap();
        assert!(Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(root.path())
            .status()
            .unwrap()
            .success());
        fs::write(root.path().join("hello.txt"), "hello").unwrap();
        let command = step("git", &["add", "--", "hello.txt"]);
        let manifest = preview(root.path(), &[command.clone()]).unwrap();
        let mut commands = Vec::new();
        let result = execute(
            root.path(),
            &command,
            &manifest,
            &mut |program, argv, _, finished, _| {
                commands.push((program.to_string(), argv.to_vec(), finished));
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(result["processSucceeded"], true);
        assert_eq!(commands.len(), 2);
        let output = Command::new("git")
            .args(["diff", "--cached", "--name-only"])
            .current_dir(root.path())
            .output()
            .unwrap();
        assert_eq!(
            String::from_utf8(output.stdout).unwrap().trim(),
            "hello.txt"
        );
    }
    #[test]
    fn exact_shell_chain_preserves_connectors_and_stops_on_nonzero() {
        let root = tempfile::tempdir().unwrap();
        assert!(Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(root.path())
            .status()
            .unwrap()
            .success());
        fs::write(root.path().join("x"), "hello").unwrap();
        let command = step("shell", &["-c", "git add -- x && git status --short"]);
        let manifest = preview(root.path(), &[command.clone()]).unwrap();
        let mut logged = Vec::new();
        let result = execute(
            root.path(),
            &command,
            &manifest,
            &mut |_, argv, _, finished, _| {
                if !finished {
                    logged.push(argv.to_vec());
                }
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(result["processSucceeded"], true);
        assert!(result["stdout"].as_str().unwrap().contains("A  x"));
        assert_eq!(logged, vec![command.argv.clone()]);
        let command = step("shell", &["-c", "false && printf should-not-run"]);
        let result = execute(
            root.path(),
            &command,
            &manifest,
            &mut |_, _, _, _, _| Ok(()),
        )
        .unwrap();
        assert_eq!(result["processSucceeded"], false);
        assert_eq!(result["exitCode"], 1);
        assert_eq!(result["stdout"], "");
    }
    #[test]
    fn replaced_executable_and_working_directory_stop_before_command_intent() {
        let root = tempfile::tempdir().unwrap();
        let repository = root.path().join("repo");
        fs::create_dir(&repository).unwrap();
        assert!(Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(&repository)
            .status()
            .unwrap()
            .success());
        let command = step("git", &["status", "--short"]);
        let mut manifest = preview(&repository, &[command.clone()]).unwrap();
        let executable = root.path().join("reviewed-git");
        fs::write(&executable, "original reviewed program").unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        manifest.executables[0].path = executable.to_string_lossy().into_owned();
        manifest.executables[0].sha256 = executable_hash(&executable).unwrap();
        fs::write(&executable, "replacement program").unwrap();
        let mut logged = false;
        let error = execute(&repository, &command, &manifest, &mut |_, _, _, _, _| {
            logged = true;
            Ok(())
        })
        .unwrap_err();
        assert!(error.contains("executable changed"));
        assert!(!logged);
        fs::rename(&repository, root.path().join("previous-repo")).unwrap();
        fs::create_dir(&repository).unwrap();
        let error = execute(&repository, &command, &manifest, &mut |_, _, _, _, _| {
            logged = true;
            Ok(())
        })
        .unwrap_err();
        assert!(error.contains("working directory was replaced"));
        assert!(!logged);
    }
}

#[cfg(test)]
#[path = "blind_execution_tests.rs"]
mod blind_execution_tests;
