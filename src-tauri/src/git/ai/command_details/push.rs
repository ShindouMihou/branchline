use super::{
    read_git,
    types::{PushTarget, PushTargetKind},
    Detail, State,
};
use std::path::Path;

pub(super) fn inspect(
    root: Option<&Path>,
    args: &[String],
    command_index: usize,
    state: &State,
) -> Detail {
    let mut detail = Detail::new(command_index, "remote.push", "Push commits");
    detail.evidence = "Literal push arguments and configured remote names only. Destination state and the publication result have not been checked with the remote.".into();
    let mut positional = Vec::new();
    let mut literal = false;
    let mut unsupported = false;
    for arg in args {
        if !literal && arg == "--" {
            literal = true;
            continue;
        }
        if !literal && arg.starts_with('-') {
            match arg.as_str() {
                "--atomic" => detail.atomic = Some(true),
                "--no-atomic" => detail.atomic = Some(false),
                "-u" | "--set-upstream" => detail
                    .warnings
                    .push("This also sets upstream tracking if the push succeeds.".into()),
                _ => {
                    detail.warnings.push(format!("Push option {arg:?} is not represented in this target preview. Later arguments require exact-command review."));
                    unsupported = true;
                    detail.push_targets_incomplete = true;
                    break;
                }
            }
        } else {
            positional.push(arg.as_str());
        }
    }
    let Some(remote) = positional.first() else {
        detail.warnings.push("The remote and destination are not determined by this preview; no target is assumed here.".into());
        return detail;
    };
    let configured = root.is_none_or(|root| {
        read_git(root, &["remote"]).is_some_and(|names| names.lines().any(|name| name == *remote))
    });
    if !configured {
        detail.warnings.push("The supplied remote is not a currently configured remote name. Its target cannot be represented here.".into());
        return detail;
    }
    detail.remote = Some((*remote).into());
    if root.is_none() {
        detail.evidence = "Literal remote and refspec arguments only; remote configuration and destination state are not verified.".into();
    }
    if unsupported {
        return detail;
    }
    if positional.len() == 1 {
        detail.warnings.push("The destination ref is selected by Git configuration at execution time; no branch is assumed here.".into());
        return detail;
    }
    for refspec in &positional[1..] {
        if refspec.starts_with('+') || refspec.contains('*') {
            detail.push_targets_incomplete = true;
            detail.warnings.push(format!("Refspec {refspec:?} may force updates or select multiple refs; its effects are not represented here. Review the exact command."));
            continue;
        }
        let (source, target) = refspec.split_once(':').unwrap_or((refspec, refspec));
        if source.is_empty() || target.is_empty() || target == "HEAD" || target.contains(':') {
            detail.push_targets_incomplete = true;
            detail.warnings.push(format!("Refspec {refspec:?} has a deletion, implicit destination, or unsupported syntax. Review the exact command."));
            continue;
        }
        let kind = if target.starts_with("refs/tags/")
            || (!target.starts_with("refs/") && source.starts_with("refs/tags/"))
        {
            PushTargetKind::Tag
        } else if target.starts_with("refs/heads/")
            || (!target.starts_with("refs/")
                && (source.starts_with("refs/heads/")
                    || source == "HEAD" && state.branch.is_some()))
        {
            PushTargetKind::Branch
        } else {
            PushTargetKind::Ref
        };
        detail.push_targets.push(PushTarget {
            source_ref: source.into(),
            target_ref: target.into(),
            source_branch: if source == "HEAD" {
                state.branch.clone()
            } else {
                None
            },
            kind,
        });
    }
    if detail.push_targets.len() == 1
        && positional.len() == 2
        && detail.push_targets[0].kind != PushTargetKind::Tag
    {
        let target = &detail.push_targets[0];
        detail.source_branch = if target.source_ref == "HEAD" {
            state.branch.clone()
        } else {
            Some(target.source_ref.trim_start_matches("refs/heads/").into())
        };
        detail.target_branch = Some(target.target_ref.trim_start_matches("refs/heads/").into());
    }
    detail.warnings.push("Source ref resolution, fast-forward eligibility, server policies, and additional push configuration are not verified by this preview.".into());
    detail
}
