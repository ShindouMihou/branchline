use super::*;

pub(super) fn command_preview(root: &Path, steps: &[Step]) -> Result<Preview, String> {
    let commands = steps.iter().map(|step| match step {
        Step::Command { program, argv, stdin, timeout_seconds } => Ok(commands::CommandStep {
            program: program.clone(), argv: argv.clone(), stdin: stdin.clone(), timeout_seconds: *timeout_seconds,
        }),
        _ => Err("Submit the whole unchanged command using branchline_propose_command.".to_string()),
    }).collect::<Result<Vec<_>, _>>()?;
    let manifest = commands::preview(root, &commands)?;
    let mut risks = vec![RiskFlag {
        code: "unverified_commands".into(), title: "Approve exact commands".into(),
        detail: "These commands run with your account's permissions. They may change files, history, GitHub settings, or other repositories and may invoke configured hooks, aliases, extensions, and helpers. Branchline checks process outcomes, not their complete effects. Review every argument and input.".into(),
    }];
    let groups = commands.iter().filter_map(command_details::literal_commands).flatten().collect::<Vec<_>>();
    let pushes = groups.iter().filter(|words| words.first().is_some_and(|p| p == "git") && words.get(1).is_some_and(|p| p == "push")).collect::<Vec<_>>();
    if pushes.iter().any(|words| words.iter().skip(2).any(|arg| {
        let destination = arg.rsplit(':').next().unwrap_or(arg).trim_start_matches('+').trim_start_matches("refs/heads/");
        matches!(destination, "main" | "master")
    })) {
        risks.push(RiskFlag { code: "primary_branch_argument".into(), title: "Pushing to a primary branch".into(), detail: "A push names main or master as its destination. Review the whole command carefully; this may update a shared primary branch.".into() });
    }
    if groups.iter().any(|words| words.iter().skip(1).any(|arg| matches!(arg.as_str(), "--force" | "-f" | "--hard" | "--mirror" | "--delete" | "delete") || arg.starts_with("--force-with-lease"))) {
        risks.push(RiskFlag { code: "destructive_command_argument".into(), title: "Potentially destructive command".into(), detail: "The request includes force, deletion, mirror, or hard-reset arguments. Completed effects may be irreversible.".into() });
    }
    let mut source = txt(root, &["symbolic-ref", "--quiet", "--short", "HEAD"]).ok();
    let initial_branch = source.clone().unwrap_or_default();
    let mut mismatch = None;
    for words in &groups {
        match words.as_slice() {
            [program, action, args @ ..] if program == "git" && matches!(action.as_str(), "switch" | "checkout") => {
                let offset = usize::from(args.first().is_some_and(|arg| matches!(arg.as_str(), "-c" | "-C" | "-b" | "-B")));
                source = args.get(offset).filter(|arg| !arg.starts_with('-')).cloned();
            }
            [program, action, args @ ..] if program == "git" && action == "push" => {
                for argument in args {
                    if let Some((from, to)) = argument.split_once(':') {
                        let from = from.trim_start_matches('+').trim_start_matches("refs/heads/");
                        let from = if from == "HEAD" { source.as_deref().unwrap_or("") } else { from };
                        let to = to.trim_start_matches("refs/heads/");
                        if !from.is_empty() && !to.is_empty() && from != to && !from.starts_with('-') {
                            mismatch = Some(format!("The push maps {from} to {to}. These names differ; this does not rename or switch the source branch."));
                        }
                    }
                }
            }
            [program, action, ..] if program == "git" && action == "commit" => {},
            [program, action, ..] if program == "git" && action == "add" => {},
            _ => source = None,
        }
    }
    if let Some(detail) = mismatch {
        risks.push(RiskFlag { code: "branch_mismatch".into(), title: "Source and destination branches differ".into(), detail });
    }
    let mut preview = Preview {
        root: manifest.root.clone(), git_dir: String::new(), common_dir: String::new(), head: String::new(), branch: initial_branch,
        staged_token: String::new(), patch: String::new(), files: Vec::new(),
        warnings: vec!["The whole command runs unchanged after approval. Shell operators retain their normal behavior. Branchline records the process outcome without independently verifying every effect. Commands may access resources outside this repository. Arguments and input are stored in the private local activity record; output is withheld because it may contain secrets. Interactive commands need noninteractive arguments or standard input.".into()],
        risks, push: None, pull_request: None, fingerprint: String::new(), command_workflow: Some(manifest),
        command_details: command_details::inspect(root, &commands).unwrap_or_default(),
    };
    preview.fingerprint = format!("{:x}", Sha256::digest(serde_json::to_vec(&preview).map_err(|e| e.to_string())?));
    Ok(preview)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn primary_branch_warning_distinguishes_pr_base_from_push_destination() {
        let dir = tempfile::tempdir().unwrap();
        assert!(std::process::Command::new("git").args(["init", "-q", "-b", "main"]).current_dir(dir.path()).status().unwrap().success());
        let inspect = |script: &str| command_preview(dir.path(), &[Step::Command { program: "shell".into(), argv: vec!["-c".into(), script.into()], stdin: None, timeout_seconds: None }]).unwrap();
        assert!(!inspect("git push origin HEAD:feat/test && gh pr create --base main --head feat/test --title Title --body Description").risks.iter().any(|risk| risk.code == "primary_branch_argument"));
        assert!(inspect("git push origin HEAD:refs/heads/main").risks.iter().any(|risk| risk.code == "primary_branch_argument"));
    }
}
