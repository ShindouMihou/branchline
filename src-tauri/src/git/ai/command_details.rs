mod push;
mod literal;
mod shell;
mod staging;
mod types;
use super::commands::CommandStep;
use staging::stage;
use std::{
    fs,
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
};
pub use types::{Detail, FileSelection, FileSource};

fn read_git(root: &Path, args: &[&str]) -> Option<String> {
    let (code, out, _) = super::super::git(root, args, None).ok()?;
    (code == 0).then(|| String::from_utf8(out).ok()).flatten()
}

fn read_text(root: Option<&Path>, value: &str) -> Result<String, String> {
    let root = root.ok_or("File contents are not read after a directory or configuration change; review the file at execution time.")?;
    if value == "-" {
        return Err("The message is read from stdin when this command runs.".into());
    }
    let root = fs::canonicalize(root)
        .map_err(|_| "Cannot identify the repository for the message preview.")?;
    let requested = root.join(value);
    let metadata = fs::symlink_metadata(&requested)
        .map_err(|_| "The message file is not currently readable.")?;
    let path =
        fs::canonicalize(&requested).map_err(|_| "The message file is not currently readable.")?;
    if !path.starts_with(&root)
        || !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() > 64_000
    {
        return Err(
            "File contents are previewed only for repository-local regular files up to 64 KB."
                .into(),
        );
    }
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(|_| "The message file changed or cannot be opened safely.")?;
    let opened = file
        .metadata()
        .map_err(|_| "Cannot identify the opened message file.")?;
    if metadata.dev() != opened.dev() || metadata.ino() != opened.ino() || !opened.is_file() {
        return Err("The message file changed during preview.".into());
    }
    let mut bytes = Vec::new();
    file.take(64_001)
        .read_to_end(&mut bytes)
        .map_err(|_| "Cannot read the message file.")?;
    if bytes.len() > 64_000 {
        return Err("The message file exceeds the preview limit.".into());
    }
    String::from_utf8(bytes).map_err(|_| "The message file is not UTF-8 text.".into())
}

fn flag(
    args: &[String],
    index: &mut usize,
    short: &str,
    long: &str,
) -> Option<Result<String, String>> {
    let arg = &args[*index];
    if (!short.is_empty() && arg == short) || arg == long {
        *index += 1;
        return Some(
            args.get(*index)
                .cloned()
                .ok_or_else(|| format!("Missing value for {arg}.")),
        );
    }
    if let Some(value) = arg.strip_prefix(&format!("{long}=")) {
        return Some(Ok(value.into()));
    }
    if !short.is_empty() && arg.starts_with(short) && arg.len() > short.len() {
        return Some(Ok(arg[short.len()..].into()));
    }
    None
}

#[derive(Clone)]
struct State {
    branch: Option<String>,
    staged: Vec<String>,
    projected: Vec<String>,
    index_uncertain: bool,
    projected_scope_uncertain: bool,
    previous: bool,
}

fn commit(root: Option<&Path>, args: &[String], command_index: usize, state: &State) -> Detail {
    let mut detail = Detail::new(command_index, "commit.create", "Commit changes");
    detail.warnings.push("The message shown is command input; Git cleanup, hooks, or message options may change the final commit message.".into());
    detail.source_branch = state.branch.clone();
    detail.files = Some(state.staged.clone());
    detail.file_previews = state
        .staged
        .iter()
        .map(|path| FileSelection {
            path: path.clone(),
            source: if state.projected.contains(path) {
                FileSource::Worktree
            } else {
                FileSource::Index
            },
        })
        .collect();
    detail.evidence_kind = "current_snapshot".into();
    detail.evidence = "Files shown are in the current index at preview time, not a guarantee of the later commit.".into();
    if !state.projected.is_empty() {
        detail
            .files
            .as_mut()
            .unwrap()
            .extend(state.projected.clone());
        detail.files.as_mut().unwrap().sort();
        detail.files.as_mut().unwrap().dedup();
        detail.file_previews = detail
            .files
            .as_ref()
            .unwrap()
            .iter()
            .map(|path| FileSelection {
                path: path.clone(),
                source: if state.projected.contains(path) {
                    FileSource::Worktree
                } else {
                    FileSource::Index
                },
            })
            .collect();
        detail.evidence = "Current index files plus literal paths selected by earlier staging commands; prospective contents and command success are not verified.".into();
    }
    if state.projected_scope_uncertain {
        detail.files = None;
        detail.file_previews.clear();
        detail.evidence_kind = "unknown".into();
        detail.evidence = "Restaging an already staged file may remove or replace its pending change. The later commit file selection cannot be determined from the current index alone.".into();
    }
    let (mut messages, mut files) = (Vec::new(), Vec::new());
    let mut index = 0;
    while index < args.len() {
        if args[index] == "--" {
            if index + 1 < args.len() {
                detail.files = None;
                detail.file_previews.clear();
                detail.warnings.push("Explicit paths select the commit content; the current-index file list is not a complete preview of that selection.".into());
            }
            break;
        }
        if args[index].starts_with("-am") {
            let value = args[index].strip_prefix("-am").unwrap();
            if value.is_empty() {
                index += 1;
                match args.get(index) {
                    Some(value) => messages.push(value.clone()),
                    None => detail.warnings.push("Missing message after -am.".into()),
                }
            } else {
                messages.push(value.into());
            }
            detail.warnings.push("Tracked working-file changes are also eligible; the current-index file list is incomplete.".into());
        } else if let Some(value) = flag(args, &mut index, "-m", "--message") {
            match value {
                Ok(value) => messages.push(value),
                Err(error) => detail.warnings.push(error),
            }
        } else if let Some(value) = flag(args, &mut index, "-F", "--file") {
            match value.and_then(|path| read_text(root, &path)) {
                Ok(value) => files.push(value),
                Err(error) => detail.warnings.push(error),
            }
        } else {
            match args[index].as_str() {
                "--amend" => detail.warnings.push("This amends the current commit and changes its identity; the existing message/content may also apply.".into()),
                "-a" | "--all" => detail.warnings.push("Tracked working-file changes are also eligible; the current-index file list is incomplete.".into()),
                "--no-edit" => detail.warnings.push("Git selects an existing message at execution time.".into()),
                "--allow-empty" | "--allow-empty-message" | "--signoff" | "-s" => {},
                _ => detail.warnings.push(format!("Additional commit argument {:?} may change the message or included content.", args[index])),
            }
        }
        index += 1;
    }
    if !messages.is_empty() && files.is_empty() {
        detail.message = Some(messages.join("\n\n"));
    } else if messages.is_empty() && files.len() == 1 {
        detail.message = files.pop();
        detail.warnings.push("Message file contents may change before execution; Git cleanup and hooks can alter the final message.".into());
    } else {
        detail.warnings.push(
            "No single literal commit message can be determined from these arguments.".into(),
        );
    }
    if state.index_uncertain {
        detail.warnings.push("Earlier commands may change the index; the displayed file list is not complete or verified.".into());
    }
    detail
}

fn branch_detail(
    command: &str,
    args: &[String],
    command_index: usize,
    state: &mut State,
) -> Detail {
    let creation = args
        .first()
        .is_some_and(|arg| matches!(arg.as_str(), "-c" | "-C" | "-b" | "-B"));
    let offset = usize::from(creation);
    let Some(target) = args.get(offset).filter(|value| !value.starts_with('-')) else {
        return Detail::unavailable(
            command_index,
            "Branch options require Git to determine the resulting branch.",
        );
    };
    if creation && args.get(offset + 1).is_some_and(|value| value.starts_with('-')) {
        return Detail::unavailable(
            command_index,
            "Additional branch options require Git to determine the branch start point.",
        );
    }
    if args.len() > offset + 1 + usize::from(creation) {
        return Detail::unavailable(
            command_index,
            "Additional checkout arguments may select paths rather than switch branches.",
        );
    }
    let mut detail = Detail::new(
        command_index,
        if creation {
            "branch.create"
        } else {
            "branch.switch"
        },
        if creation {
            "Create and switch branch"
        } else {
            "Switch branch"
        },
    );
    detail.source_branch = if creation {
        args.get(offset + 1)
            .cloned()
            .or_else(|| state.branch.clone())
    } else {
        state.branch.clone()
    };
    detail.target_branch = Some(target.clone());
    if !creation && command == "checkout" {
        detail.warnings.push("Checkout can also select commits or paths; branch existence and the resulting files are not verified here.".into());
    }
    if args
        .first()
        .is_some_and(|arg| matches!(arg.as_str(), "-C" | "-B"))
    {
        detail
            .warnings
            .push("This may reset an existing branch to a different commit.".into());
    }
    state.branch = Some(target.clone());
    detail
}

fn create_branch(args: &[String], command_index: usize, state: &State) -> Detail {
    if !(1..=2).contains(&args.len()) || args.iter().any(|arg| arg.starts_with('-')) {
        return Detail::unavailable(
            command_index,
            "Branch flags are not represented in this static preview.",
        );
    }
    let mut detail = Detail::new(command_index, "branch.create", "Create branch");
    detail.source_branch = args.get(1).cloned().or_else(|| state.branch.clone());
    detail.target_branch = Some(args[0].clone());
    detail
        .warnings
        .push("Creating this branch does not switch to it.".into());
    detail
}

fn pull_request(root: Option<&Path>, args: &[String], command_index: usize, state: &State) -> Detail {
    let mut detail = Detail::new(command_index, "pr.create", "Open pull request");
    detail.head_branch = state.branch.clone();
    detail.evidence = "Literal PR options; an omitted head is inferred from the current or prospectively selected branch, not verified with GitHub.".into();
    let mut index = 0;
    while index < args.len() {
        if args[index] == "--" {
            detail.warnings.push("Arguments after the option terminator are not PR options; their interpretation is left to GitHub CLI.".into());
            break;
        }
        let mut recognized = false;
        for (short, long, field) in [
            ("-t", "--title", 0),
            ("-b", "--body", 1),
            ("-B", "--base", 2),
            ("-H", "--head", 3),
            ("-F", "--body-file", 4),
        ] {
            if let Some(value) = flag(args, &mut index, short, long) {
                recognized = true;
                match value {
                    Ok(value) => match field {
                        0 => detail.pull_request_title = Some(value),
                        1 => detail.body = Some(value),
                        2 => detail.base_branch = Some(value),
                        3 => detail.head_branch = Some(value),
                        _ => match read_text(root, &value) {
                            Ok(text) => detail.body = Some(text),
                            Err(error) => detail.warnings.push(error),
                        },
                    },
                    Err(error) => detail.warnings.push(error),
                }
                break;
            }
        }
        if !recognized {
            detail
                .warnings
                .push(if args[index] == "--draft" || args[index] == "-d" {
                    "The pull request will be a draft.".into()
                } else {
                    format!(
                        "Additional PR argument {:?} is not represented in these details.",
                        args[index]
                    )
                });
        }
        index += 1;
    }
    if detail.base_branch.is_none() {
        detail
            .warnings
            .push("The base branch is chosen by GitHub CLI at execution time.".into());
    }
    if detail.pull_request_title.is_none() || detail.body.is_none() {
        detail.warnings.push("Title or description may be generated or requested by GitHub CLI; missing text is not previewed.".into());
    }
    detail.warnings.push("Account, repository, server defaults, publication, and resulting PR URL are not verified by this preview.".into());
    detail
}

pub fn inspect(root: &Path, steps: &[CommandStep]) -> Result<Vec<Detail>, String> {
    let branch = read_git(root, &["symbolic-ref", "--quiet", "--short", "HEAD"])
        .map(|s| s.trim().to_string());
    let staged = read_git(
        root,
        &[
            "diff",
            "--cached",
            "--name-only",
            "--no-ext-diff",
            "--no-textconv",
            "-z",
        ],
    );
    let mut state = State {
        branch,
        staged: staged
            .as_deref()
            .unwrap_or_default()
            .split('\0')
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect(),
        projected: Vec::new(),
        index_uncertain: staged.is_none(),
        projected_scope_uncertain: false,
        previous: false,
    };
    let mut details = Vec::new();
    for (command_index, step) in steps.iter().enumerate() {
        let mut context_uncertain = false;
        let mut requested_directory = None;
        let commands = if step.program == "shell" {
            match step
                .argv
                .get(1)
                .ok_or_else(|| "Shell text is missing.".to_string())
                .and_then(|script| shell::split(script))
            {
                Ok(commands) => commands,
                Err(error) => {
                    details.push(Detail::unavailable(command_index, &error));
                    state.branch = None;
                    state.index_uncertain = true;
                    continue;
                }
            }
        } else {
            vec![std::iter::once(step.program.clone())
                .chain(step.argv.clone())
                .collect()]
        };
        for words in commands {
            let words = literal::identity_command(&words).unwrap_or(words);
            if let [program, path] = words.as_slice() {
                if program == "cd" && Path::new(path).is_absolute() {
                    context_uncertain = true;
                    requested_directory = Some(path.clone());
                    state.branch = None;
                    state.staged.clear();
                    state.projected.clear();
                    state.index_uncertain = true;
                }
            }
            let literal_context = context_uncertain && requested_directory.is_some();
            let git_override = matches!(words.as_slice(),[program,option,..] if program=="git"&&option.starts_with('-'));
            let mut detail = if literal_context || literal::special(&words) {
                literal::inspect(&words, command_index, &mut state).unwrap_or_else(|| {
                    requested_directory = None;
                    Detail::unavailable(
                        command_index,
                        "This command needs runtime interpretation. Review its exact text.",
                    )
                })
            } else if context_uncertain {
                Detail::unavailable(command_index, "An earlier shell command may change the working directory or environment. Structured details cannot be resolved in that context.")
            } else {
                match words.as_slice() {
                    [program, command, args @ ..] if program == "git" && command == "commit" => {
                        commit(Some(root), args, command_index, &state)
                    }
                    [program, command, args @ ..] if program == "git" && command == "add" => {
                        stage(args, command_index, &mut state, root)
                    }
                    [program, command, args @ ..] if program == "git" && command == "push" => {
                        push::inspect(Some(root), args, command_index, &state)
                    }
                    [program, command, args @ ..]
                        if program == "git"
                            && matches!(command.as_str(), "switch" | "checkout") =>
                    {
                        branch_detail(command, args, command_index, &mut state)
                    }
                    [program, command, args @ ..] if program == "git" && command == "branch" => {
                        create_branch(args, command_index, &state)
                    }
                    [program, family, command, args @ ..]
                        if program == "gh" && family == "pr" && command == "create" =>
                    {
                        pull_request(Some(root), args, command_index, &state)
                    }
                    _ => {
                        state.branch = None;
                        state.index_uncertain = true;
                        context_uncertain = !words
                            .first()
                            .is_some_and(|program| matches!(program.as_str(), "git" | "gh"));
                        Detail::unavailable(command_index, "This command has no structured preview yet. Review its exact text; execution remains available.")
                    }
                }
            };
            if detail.working_directory.is_none() {
                detail.working_directory = requested_directory.clone();
            }
            if literal_context {
                detail.warnings.push("Directory and command arguments are requested values. Directory changes, command success, file selection and repository state are not verified here.".into());
            }
            if detail.kind == "details_unavailable" {
                state.branch = None;
                state.index_uncertain = true;
            }
            if state.previous {
                if detail.evidence_kind != "unknown" {
                    detail.evidence_kind = "projected".into();
                }
                detail.warnings.push("This is a prospective later action; whether it runs depends on earlier commands and shell control flow.".into());
            }
            if detail.kind == "commit.create" && !git_override {
                state.staged.clear();
                state.projected.clear();
                state.projected_scope_uncertain = false;
                state.index_uncertain = true;
            }
            if git_override {
                state.projected_scope_uncertain = true;
                state.index_uncertain = true;
            }
            state.previous = true;
            details.push(detail);
        }
    }
    Ok(details)
}

#[cfg(test)]
mod tests;

pub(super) fn literal_commands(step: &CommandStep) -> Option<Vec<Vec<String>>> {
    if step.program == "shell" {
        shell::split(step.argv.get(1)?).ok()
    } else {
        Some(vec![std::iter::once(step.program.clone())
            .chain(step.argv.clone())
            .collect()])
    }
}

#[cfg(test)]
mod blind_tests;

#[cfg(test)]
mod blind_cycles;

#[cfg(test)]
mod context_tests;
