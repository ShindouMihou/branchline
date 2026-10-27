use super::{flag, message_file, types::BodySource, Detail, State};
use std::path::Path;

pub(super) fn inspect(root: Option<&Path>, args: &[String], index: usize, state: &State, edit: bool) -> Detail {
    let mut detail = Detail::new(index, if edit { "pr.edit" } else { "pr.create" }, if edit { "Update pull request" } else { "Open pull request" });
    if !edit { detail.head_branch = state.branch.clone(); }
    detail.evidence = if edit {
        "Requested PR changes only. The current pull request contents and resulting update are not fetched or verified."
    } else {
        "Literal PR options; an omitted head is inferred from the current or prospectively selected branch, not verified with GitHub."
    }.into();
    let mut cursor = 0;
    let mut ambiguous_target = false;
    while cursor < args.len() {
        if args[cursor] == "--" {
            detail.warnings.push("Arguments after the option terminator are not interpreted as PR options. Review the exact command.".into());
            break;
        }
        let mut recognized = false;
        for (short, long, field) in [("-t", "--title", 0), ("-b", "--body", 1), ("-B", "--base", 2), ("-H", "--head", 3), ("-F", "--body-file", 4), ("-R", "--repo", 5)] {
            if edit && field == 3 { continue; }
            if let Some(value) = flag(args, &mut cursor, short, long) {
                recognized = true;
                match value {
                    Ok(value) => match field {
                        0 => detail.pull_request_title = Some(value),
                        1 => { detail.body = Some(value); detail.body_source = Some(BodySource::Argument); },
                        2 => detail.base_branch = Some(value),
                        3 => detail.head_branch = Some(value),
                        5 => detail.repository = Some(value),
                        _ => {
                            detail.body = None;
                            detail.body_source = Some(if value == "-" { BodySource::Stdin } else { BodySource::File });
                            if value != "-" {
                                match message_file::read_body(root, &value) {
                                    Ok(text) => {
                                        detail.body = Some(text);
                                        detail.warnings.push("The description is a current file snapshot, not a frozen execution input. Earlier commands or later edits may change what GitHub CLI reads.".into());
                                    },
                                    Err(error) => detail.warnings.push(error),
                                }
                            }
                        },
                    },
                    Err(error) => detail.warnings.push(error),
                }
                break;
            }
        }
        if !recognized {
            let arg = &args[cursor];
            if edit && !arg.starts_with('-') {
                if detail.pull_request_target.is_some() { ambiguous_target = true; }
                else { detail.pull_request_target = Some(arg.clone()); }
            } else if !edit && matches!(arg.as_str(), "--draft" | "-d") {
                detail.warnings.push("The pull request will be a draft.".into());
            } else {
                detail.warnings.push(format!("Additional PR option {arg:?} is not represented in these details. Review the exact command."));
                if !arg.contains('=') && arg.starts_with('-') && arg != "--remove-milestone" && args.get(cursor + 1).is_some_and(|next| !next.starts_with('-')) { cursor += 1; }
            }
        }
        cursor += 1;
    }
    if ambiguous_target {
        detail.pull_request_target = None;
        detail.warnings.push("Multiple positional arguments prevent identifying one pull request target.".into());
    } else if edit && detail.pull_request_target.is_none() {
        detail.warnings.push("GitHub CLI selects the pull request for the current branch at execution time.".into());
    }
    if !edit && detail.base_branch.is_none() { detail.warnings.push("The base branch is chosen by GitHub CLI at execution time.".into()); }
    if !edit && (detail.pull_request_title.is_none() || detail.body_source.is_none()) {
        detail.warnings.push("Title or description may be generated or requested by GitHub CLI; missing text is not previewed.".into());
    }
    detail.warnings.push("Account, repository, server defaults, and the GitHub result are not verified by this preview.".into());
    detail
}

pub(super) fn bind_input(detail: &mut Detail, input: Option<&str>, direct: bool) {
    if detail.body_source != Some(BodySource::Stdin) { return; }
    if !direct {
        detail.warnings.push("The description reads from stdin, but shell commands, pipes, or redirections may consume or replace it. The supplied command input cannot be attributed to this PR safely.".into());
    } else if let Some(input) = input {
        if input.len() > 64_000 {
            detail.warnings.push("The description exceeds the 64 KB structured preview limit. Inspect the full supplied command input instead.".into());
            return;
        }
        detail.body = Some(input.into());
        detail.warnings.push("The description is the exact supplied standard input for this single GitHub command. GitHub CLI and server behavior are not independently verified.".into());
    } else {
        detail.warnings.push("This command reads its description from stdin, but no standard input was supplied.".into());
    }
}
