use super::{read_git, Detail, FileSelection, FileSource, State};
use std::{
    collections::BTreeSet,
    path::{Component, Path},
};

fn names(root: &Path, prefix: &[&str], paths: &[String]) -> Option<Vec<String>> {
    let mut args = prefix.to_vec();
    args.push("--");
    args.extend(paths.iter().map(String::as_str));
    read_git(root, &args).map(|text| {
        text.split('\0')
            .filter(|name| !name.is_empty())
            .map(str::to_string)
            .collect()
    })
}

pub(super) fn stage(
    args: &[String],
    command_index: usize,
    state: &mut State,
    root: &Path,
) -> Detail {
    let mut detail = Detail::new(command_index, "index.stage", "Stage changes");
    detail.evidence_kind = "projected".into();
    let (mut paths, mut tracked_only, mut all, mut literal) = (Vec::new(), false, false, false);
    for arg in args {
        if !literal && arg == "--" {
            literal = true;
            continue;
        }
        if !literal && arg.starts_with('-') {
            match arg.as_str() {
                "-u" | "--update" => tracked_only = true,
                "-A" | "--all" => all = true,
                "-v" | "--verbose" => {}
                _ => {
                    detail.warnings.push(format!(
                        "Staging option {arg:?} is not represented in the file projection."
                    ));
                    state.index_uncertain = true;
                    return detail;
                }
            }
        } else {
            paths.push(arg.clone());
        }
    }
    if paths.is_empty() {
        if all || tracked_only {
            paths.push(".".into());
        } else {
            detail
                .warnings
                .push("No paths were selected; this command may stage nothing.".into());
            return detail;
        }
    }
    if paths.iter().any(|path| {
        path.starts_with(':')
            || path.contains(['*', '?', '['])
            || Path::new(path).is_absolute()
            || Path::new(path)
                .components()
                .any(|part| matches!(part, Component::ParentDir))
    }) {
        detail.warnings.push("Path patterns or paths outside the repository need runtime interpretation; exact selected files are unavailable.".into());
        state.index_uncertain = true;
        return detail;
    }
    let Some(tracked) = names(
        root,
        &[
            "diff",
            "--name-only",
            "--no-ext-diff",
            "--no-textconv",
            "-z",
        ],
        &paths,
    ) else {
        detail
            .warnings
            .push("Current tracked changes could not be read for this staging selection.".into());
        state.index_uncertain = true;
        return detail;
    };
    let mut files = tracked.into_iter().collect::<BTreeSet<_>>();
    if !tracked_only {
        let Some(untracked) = names(
            root,
            &["ls-files", "--others", "--exclude-standard", "-z"],
            &paths,
        ) else {
            detail
                .warnings
                .push("Current new files could not be read for this staging selection.".into());
            state.index_uncertain = true;
            return detail;
        };
        files.extend(untracked);
    }
    let files = files.into_iter().collect::<Vec<_>>();
    detail.files = Some(files.clone());
    detail.file_previews = files
        .iter()
        .map(|path| FileSelection {
            path: path.clone(),
            source: FileSource::Worktree,
        })
        .collect();
    detail.evidence = if tracked_only {
        "Current tracked changes matching this selection, including deletions; prospective staging only if the command succeeds. New untracked files are excluded."
    } else {
        "Current working changes and nonignored new files matching this selection, including deletions; prospective staging only if the command succeeds."
    }.into();
    if state.index_uncertain || state.previous {
        detail.warnings.push("Earlier commands may change these files or the index. This projection describes the current repository, not the exact later result.".into());
    }
    detail.warnings.push("File contents, filters, and changes made before execution are not verified by this projection.".into());
    if files.iter().any(|path| state.staged.contains(path)) {
        state.projected_scope_uncertain = true;
        detail.warnings.push("This selection restages a file already in the index; it may replace or remove that file's pending commit change.".into());
    }
    state.projected.extend(files);
    detail
}
