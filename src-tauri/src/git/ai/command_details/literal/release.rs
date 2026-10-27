use super::super::{flag, Detail};

pub(super) fn inspect(args: &[String], index: usize) -> Detail {
    let mut detail = Detail::new(index, "release.create", "Create GitHub release");
    if args.first().is_some_and(|arg| !arg.starts_with('-')) {
        detail.tag = args.first().cloned();
    }
    let mut cursor = usize::from(detail.tag.is_some());
    let mut assets = Vec::new();
    let mut literal = false;
    let mut complete = true;
    while cursor < args.len() {
        let arg = &args[cursor];
        if !literal && arg == "--" {
            literal = true;
            cursor += 1;
            continue;
        }
        if literal || !arg.starts_with('-') {
            assets.push(arg.clone());
            cursor += 1;
            continue;
        }
        let mut recognized = false;
        for (short, long, field) in [
            ("-R", "--repo", 0),
            ("", "--target", 1),
            ("-t", "--title", 2),
            ("-n", "--notes", 3),
            ("-F", "--notes-file", 4),
        ] {
            if let Some(value) = flag(args, &mut cursor, short, long) {
                recognized = true;
                match value {
                    Ok(value) => match field {
                        0 => detail.repository = Some(value),
                        1 => detail.target_branch = Some(value),
                        2 => detail.release_title = Some(value),
                        3 => detail.body = Some(value),
                        _ => detail.notes_file = Some(value),
                    },
                    Err(error) => detail.warnings.push(error),
                }
                break;
            }
        }
        if !recognized {
            if matches!(
                arg.as_str(),
                "--generate-notes" | "--latest" | "--draft" | "--prerelease" | "--verify-tag"
            ) {
                detail.warnings.push(format!(
                    "Release option {arg} will be applied by GitHub CLI."
                ));
            } else {
                complete = false;
                detail.warnings.push(format!("Release option {arg:?} is not represented; asset membership may be incomplete."));
            }
        }
        cursor += 1;
    }
    if complete {
        detail.assets = Some(assets);
    }
    detail.warnings.push("This uploads a release and its requested assets. The repository, tag, server state and resulting release URL are not independently verified. Notes-file contents are not loaded into this preview.".into());
    detail
}
