use super::{flag, Detail, State};

pub(super) fn inspect(
    action: &str,
    args: &[String],
    index: usize,
    state: &State,
) -> Option<Detail> {
    match action {
        "stash" => stash(args, index),
        "rebase" => Some(rebase(args, index, state)),
        "reset" => Some(reset(args, index)),
        _ => None,
    }
}

fn stash(args: &[String], index: usize) -> Option<Detail> {
    let explicit = args.first().is_some_and(|word| !word.starts_with('-'));
    let operation = if explicit { args[0].as_str() } else { "push" };
    let args = if explicit { &args[1..] } else { args };
    let mut detail = match operation {
        "push" | "save" => Detail::new(index, "stash.save", "Save work in a stash"),
        "apply" | "pop" => Detail::new(index, "stash.apply", "Restore stashed work"),
        _ => return None,
    };
    detail.operation = Some(operation.into());
    let mut cursor = 0;
    let mut refs = Vec::new();
    while cursor < args.len() {
        if args[cursor] == "--" {
            break;
        }
        if let Some(value) = flag(args, &mut cursor, "-m", "--message") {
            match value {
                Ok(message) => detail.message = Some(message),
                Err(error) => detail.warnings.push(error),
            }
        } else if matches!(
            args[cursor].as_str(),
            "-u" | "--include-untracked" | "-a" | "--all"
        ) {
            detail.include_untracked = Some(true);
            if matches!(args[cursor].as_str(), "-a" | "--all") {
                detail
                    .warnings
                    .push("Ignored files are also included in the requested stash.".into());
            }
        } else if args[cursor].starts_with('-') {
            detail.warnings.push(format!(
                "Additional stash option {:?} is not represented in this summary.",
                args[cursor]
            ));
        } else {
            refs.push(args[cursor].clone());
        }
        cursor += 1;
    }
    if detail.kind == "stash.save" {
        detail.include_untracked.get_or_insert(false);
        detail.warnings.push("Saving a stash may remove saved changes from the working tree. The exact files and contents are not verified by this card.".into());
    } else {
        detail.stash_ref = refs.first().cloned().or_else(|| Some("stash@{0}".into()));
        detail.warnings.push("Restoring a stash may conflict with current files. A successful earlier rebase does not guarantee the restore succeeds.".into());
        if operation == "pop" {
            detail
                .warnings
                .push("The stash is removed if it is applied successfully.".into());
        }
    }
    Some(detail)
}

fn rebase(args: &[String], index: usize, state: &State) -> Detail {
    let mut detail = Detail::new(index, "history.rebase", "Rebase commits");
    detail.source_branch = state.branch.clone();
    let mut cursor = 0;
    let mut positional = Vec::new();
    let mut uncertain = false;
    while cursor < args.len() {
        if let Some(value) = flag(args, &mut cursor, "", "--onto") {
            match value {
                Ok(value) => detail.base_ref = Some(value),
                Err(error) => detail.warnings.push(error),
            }
        } else if matches!(
            args[cursor].as_str(),
            "--continue" | "--abort" | "--skip" | "--quit"
        ) {
            detail.operation = Some(args[cursor].trim_start_matches('-').into());
        } else if matches!(
            args[cursor].as_str(),
            "-i" | "--interactive" | "--autostash" | "--autosquash" | "--rebase-merges" | "--root"
        ) {
            detail
                .warnings
                .push(format!("Requested rebase option: {}.", args[cursor]));
        } else if args[cursor].starts_with('-') {
            uncertain = true;
            detail.warnings.push(format!(
                "Rebase option {:?} may affect positional references; review the exact command.",
                args[cursor]
            ));
        } else {
            positional.push(args[cursor].clone());
        }
        cursor += 1;
    }
    if !uncertain {
        detail.upstream_ref = positional.first().cloned();
        detail.target_branch = positional.get(1).cloned();
    }
    detail.warnings.push("A rebase can rewrite commit identities and stop on conflicts. Later commands run only according to the original shell control flow.".into());
    detail
}

fn reset(args: &[String], index: usize) -> Detail {
    let mut detail = Detail::new(index, "history.reset", "Reset Git state");
    detail.operation = Some("mixed".into());
    let mut refs = Vec::new();
    let mut uncertain = false;
    for arg in args {
        if arg == "--" {
            break;
        }
        if matches!(
            arg.as_str(),
            "--mixed" | "--soft" | "--hard" | "--merge" | "--keep"
        ) {
            detail.operation = Some(arg.trim_start_matches('-').into());
        } else if arg.starts_with('-') {
            uncertain = true;
        } else {
            refs.push(arg.clone());
        }
    }
    if !uncertain && refs.len() <= 1 {
        detail.target_branch = Some(refs.first().cloned().unwrap_or_else(|| "HEAD".into()));
    } else {
        detail.warnings.push(
            "Path selection or additional reset options are not represented in this summary."
                .into(),
        );
    }
    detail.warnings.push(match detail.operation.as_deref() {
        Some("hard") => "Hard reset can discard tracked working-file changes.",
        Some("soft") => "Soft reset moves the current branch while retaining index and working-file changes.",
        Some("mixed") => "Mixed reset resets the index (unstages changes) and retains working files. A target other than HEAD may also move the current branch.",
        _ => "This reset mode can affect the index and working files; review the exact command.",
    }.into());
    detail
}
