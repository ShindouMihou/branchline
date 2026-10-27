mod release;
use super::{branch_detail, commit, create_branch, flag, pull_request, push, Detail, State};
use std::path::Path;

pub(super) fn identity_command(words: &[String]) -> Option<Vec<String>> {
    if words.first()?.as_str() != "git" {
        return None;
    }
    let mut index = 1;
    while let Some(option) = words.get(index) {
        let value = if option == "-c" {
            index += 1;
            words.get(index)?.as_str()
        } else if option.starts_with("-c") && option.len() > 2 {
            &option[2..]
        } else {
            break;
        };
        let (key, _) = value.split_once('=')?;
        if !matches!(key, "user.name" | "user.email") {
            return None;
        }
        index += 1;
    }
    if index == 1 || words.get(index)?.starts_with('-') {
        return None;
    }
    Some(
        std::iter::once("git".to_string())
            .chain(words[index..].iter().cloned())
            .collect(),
    )
}

pub(super) fn special(words: &[String]) -> bool {
    matches!(words, [program, command, ..] if program=="git" && (command=="init" || command.starts_with('-')))
        || matches!(words, [program, family, command, ..] if program=="gh" && (family=="repo" && matches!(command.as_str(),"create"|"edit") || family=="release"&&command=="create"))
}

fn git_args(words: &[String]) -> Option<(&str, &[String], Option<String>)> {
    let mut index = 1;
    let mut directory = None;
    while words.get(index).is_some_and(|arg| arg.starts_with('-')) {
        let option = words.get(index)?;
        match option.as_str() {
            "-c" | "--config-env" => {
                words.get(index + 1)?;
                index += 2;
            }
            "-C" => {
                directory = Some(words.get(index + 1)?.clone());
                index += 2;
            }
            value
                if value.starts_with("--config-env=")
                    || value.starts_with("-c") && value.len() > 2 =>
            {
                index += 1
            }
            _ => return None,
        }
    }
    Some((words.get(index)?.as_str(), &words[index + 1..], directory))
}

fn initialize(args: &[String], index: usize) -> Detail {
    let mut detail = Detail::new(index, "repository.init", "Initialize Git repository");
    let mut cursor = 0;
    while cursor < args.len() {
        if args[cursor] == "--" {
            break;
        }
        if let Some(value) = flag(args, &mut cursor, "-b", "--initial-branch") {
            match value {
                Ok(value) => detail.target_branch = Some(value),
                Err(error) => detail.warnings.push(error),
            }
        } else if !args[cursor].starts_with('-') {
            detail.source_directory = Some(args[cursor].clone());
        } else {
            detail.warnings.push(format!(
                "Additional initialization option {:?} is not represented.",
                args[cursor]
            ));
        }
        cursor += 1;
    }
    detail.warnings.push("Initialization has not run. The initial branch setting applies to a new repository; an existing repository may retain its current branch.".into());
    detail
}

fn github_repository(command: &str, args: &[String], index: usize) -> Detail {
    let mut detail = Detail::new(
        index,
        if command == "create" {
            "repository.create"
        } else {
            "repository.edit"
        },
        if command == "create" {
            "Create GitHub repository"
        } else {
            "Update GitHub repository"
        },
    );
    if args.first().is_some_and(|arg| !arg.starts_with('-')) {
        detail.repository = args.first().cloned();
    }
    let mut cursor = usize::from(detail.repository.is_some());
    let mut topics = Vec::new();
    let mut visibilities = Vec::new();
    while cursor < args.len() {
        if args[cursor] == "--" {
            break;
        }
        if matches!(
            args[cursor].as_str(),
            "--public" | "--private" | "--internal"
        ) {
            visibilities.push(args[cursor].trim_start_matches('-').to_string());
        } else if let Some(value) = flag(args, &mut cursor, "", "--source") {
            match value {
                Ok(value) => detail.source_directory = Some(value),
                Err(error) => detail.warnings.push(error),
            }
        } else if let Some(value) = flag(args, &mut cursor, "", "--add-topic") {
            match value {
                Ok(value) => topics.push(value),
                Err(error) => detail.warnings.push(error),
            }
        } else if let Some(value) = flag(args, &mut cursor, "", "--description") {
            match value {
                Ok(value) => detail.body = Some(value),
                Err(error) => detail.warnings.push(error),
            }
        } else if let Some(value) = flag(args, &mut cursor, "", "--remote") {
            match value {
                Ok(value) => detail.remote = Some(value),
                Err(error) => detail.warnings.push(error),
            }
        } else {
            detail.warnings.push(format!("Additional repository argument {:?} is not represented; inspect the exact command.",args[cursor]));
        }
        cursor += 1;
    }
    if visibilities.len() == 1 {
        detail.visibility = visibilities.pop();
    }
    if !topics.is_empty() {
        detail.topics = Some(topics);
    }
    detail.warnings.push("Repository ownership, authentication, server permissions and resulting GitHub settings have not been verified.".into());
    detail
}

pub(super) fn inspect(words: &[String], index: usize, state: &mut State) -> Option<Detail> {
    if let [program, path] = words {
        if program == "cd" && Path::new(path).is_absolute() {
            let mut detail = Detail::new(index, "context.directory", "Change working directory");
            detail.working_directory = Some(path.clone());
            detail.warnings.push("This is the literal requested directory. It has not been opened, inspected or verified by this preview.".into());
            return Some(detail);
        }
    }
    if words.first().is_some_and(|program| program == "git") {
        let (command, args, directory) = git_args(words)?;
        let mut scoped = state.clone();
        let state = if words.get(1).is_some_and(|arg| arg.starts_with('-')) {
            scoped.branch = None;
            &mut scoped
        } else {
            state
        };
        let mut detail = match command {
            "init" => initialize(args, index),
            "commit" => {
                let mut detail = commit(None, args, index, state);
                detail.files = None;
                detail.file_previews.clear();
                detail.evidence_kind = "unknown".into();
                detail.evidence="Literal commit arguments only. File membership is not read from a different or overridden repository context.".into();
                detail
            }
            "add" => {
                let mut detail = Detail::new(index, "index.stage", "Stage changes");
                detail.evidence_kind = "unknown".into();
                detail.evidence="The requested staging command is recognized. Its file selection is not read from a different or overridden repository context.".into();
                detail
            }
            "stash" | "rebase" | "reset" => super::history::inspect(command, args, index, state)?,
            "push" => push::inspect(None, args, index, state),
            "switch" | "checkout" => branch_detail(command, args, index, state),
            "branch" => create_branch(args, index, state),
            _ => return None,
        };
        detail.working_directory = directory;
        return Some(detail);
    }
    match words {
        [program, family, command, args @ ..]
            if program == "gh" && family == "release" && command == "create" =>
        {
            Some(release::inspect(args, index))
        }
        [program, family, command, args @ ..]
            if program == "gh" && family == "pr" && command == "create" =>
        {
            Some(pull_request(None, args, index, state))
        }
        [program, family, command, args @ ..]
            if program == "gh"
                && family == "repo"
                && matches!(command.as_str(), "create" | "edit") =>
        {
            Some(github_repository(command, args, index))
        }
        _ => None,
    }
}

pub(super) fn normalize_arguments(words: Vec<String>) -> Vec<String> {
    if words.first().is_some_and(|word| word == "git") {
        if let Some((action, args, _)) = git_args(&words) {
            return std::iter::once("git".to_string()).chain(std::iter::once(action.to_string())).chain(args.iter().cloned()).collect();
        }
    }
    words
}
