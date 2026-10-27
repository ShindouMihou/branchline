use super::{read_git, Detail, State};
use std::path::Path;

pub(super) fn inspect(root: Option<&Path>, args: &[String], command_index: usize, state: &State) -> Detail {
    let mut detail = Detail::new(command_index, "remote.push", "Push commits");
    detail.source_branch = state.branch.clone();
    detail.evidence = "Literal push arguments and configured remote names only. Destination state and the publication result have not been checked with the remote.".into();
    let mut positional = Vec::new();
    let mut literal = false;
    for arg in args {
        if !literal && arg == "--" {
            literal = true;
            continue;
        }
        if !literal && arg.starts_with('-') {
            match arg.as_str() {
                "-u" | "--set-upstream" => detail
                    .warnings
                    .push("This also sets upstream tracking if the push succeeds.".into()),
                _ => {
                    detail.warnings.push(format!("Push option {arg:?} is not represented in this target preview. Review the exact command."));
                    return detail;
                }
            }
        } else {
            positional.push(arg.as_str());
        }
    }
    let Some(remote) = positional.first() else {
        detail.warnings.push("The remote and destination are selected by Git configuration when this command runs; no target is assumed here.".into());
        return detail;
    };
    let configured = root.is_none_or(|root|
        read_git(root, &["remote"]).is_some_and(|names| names.lines().any(|name| name == *remote)));
    if !configured {
        detail.warnings.push("The supplied remote is not a currently configured remote name. Its target cannot be represented here.".into());
        return detail;
    }
    detail.remote = Some((*remote).into());
    if root.is_none() {
        detail.evidence = "Literal remote and refspec arguments only; remote configuration and destination state are not verified.".into();
    }
    match positional.as_slice() {
        [_] => detail.warnings.push("The destination ref is selected by Git configuration at execution time; no branch is assumed here.".into()),
        [_, refspec] => {
            if refspec.starts_with('+') || refspec.contains('*') {
                detail.warnings.push("This refspec may force updates or select multiple refs; its effects are not represented here.".into());
                return detail;
            }
            let (source, target) = refspec.split_once(':').unwrap_or((refspec, refspec));
            if source.is_empty() || target.is_empty() || target == "HEAD" {
                detail.warnings.push("This refspec has a deletion or implicit destination; no exact branch target is assumed here.".into());
                return detail;
            }
            detail.source_branch = if source == "HEAD" { state.branch.clone() } else { Some(source.trim_start_matches("refs/heads/").into()) };
            detail.target_branch = Some(target.trim_start_matches("refs/heads/").into());
            detail.warnings.push("Source ref resolution, fast-forward eligibility, server policies, and additional push configuration are not verified by this preview.".into());
        }
        _ => detail.warnings.push("Multiple push refspecs are not represented in this summary. Review every destination in the exact command.".into()),
    }
    detail
}
