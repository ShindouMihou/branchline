use super::*;

pub(super) fn name(value: &str) -> bool {
    !value.is_empty() && !value.starts_with('-') && value.bytes().all(|c| c.is_ascii_alphanumeric() || b"_./-".contains(&c))
}
pub(super) fn branch(value: &str) -> bool {
    name(value) && value != "HEAD" && !value.starts_with("refs/") && !value.contains("..")
        && value.split('/').all(|part| !part.is_empty() && !part.starts_with('.') && !part.ends_with('.') && !part.ends_with(".lock"))
}
pub(super) fn github_repository(value: &str) -> bool {
    let parts = value.split('/').collect::<Vec<_>>();
    parts.len() == 2 && parts.iter().all(|v| !v.is_empty() && *v != "." && *v != ".." && v.bytes().all(|c| c.is_ascii_alphanumeric() || b"_.-".contains(&c)))
}
fn read(root: &str, args: &[&str]) -> Option<String> {
    git::ai::command_details::read_git(Path::new(root), args).map(|s| s.strip_suffix('\n').unwrap_or(&s).to_string())
}
pub(super) fn push_url(root: &str, remote: &str) -> Option<String> {
    let value = read(root, &["remote", "get-url", "--push", "--all", remote])?;
    (!value.is_empty() && !value.contains(['\n', '\r'])).then_some(value)
}

pub(super) fn matches(rule: &GrantRecord, plan: &Plan) -> bool {
    if plan.preview.as_ref().is_none_or(|p| p.risks.iter().any(|r| r.code != "unverified_commands" && r.code != "primary_branch_argument")) { return false; }
    !plan.steps.is_empty() && plan.steps.iter().all(|step| {
        let Step::Command {program, argv, stdin, ..} = step else { return false; };
        let commands = if program == "shell" {
            if argv.len() != 2 || argv[0] != "-c" { return false; }
            let Some(commands) = git::ai::command_details::policy_commands(&argv[1]) else { return false; };
            commands
        } else { vec![std::iter::once(program.clone()).chain(argv.iter().cloned()).collect()] };
        if stdin.is_some() && commands.len() != 1 { return false; }
        commands.iter().all(|words| command(rule, words, stdin.is_some(), &plan.repository))
    })
}

fn command(rule: &GrantRecord, words: &[String], input: bool, root: &str) -> bool {
    let args = words.iter().map(String::as_str).collect::<Vec<_>>();
    match args.as_slice() {
        ["git", "status", flags @ ..] => !input && flags.iter().all(|f| matches!(*f, "--short" | "-s" | "--branch" | "-b" | "--porcelain" | "--porcelain=v1" | "--porcelain=v2")),
        ["git", "diff", flags @ ..] => !input && !flags.is_empty() && flags.iter().all(|f| matches!(*f, "--cached" | "--staged" | "--check" | "--quiet" | "--exit-code" | "--stat")),
        ["git", "branch", "--show-current"] | ["git", "rev-parse", "HEAD"] => !input,
        ["git", "add", "--", paths @ ..] => !input && rule.operations.contains(&Operation::Stage) && !paths.is_empty(),
        ["git", "commit", options @ ..] => !input && rule.operations.contains(&Operation::Commit) && !options.is_empty()
            && options.len() % 2 == 0 && options.chunks(2).all(|p| matches!(p[0], "-m" | "--message") && !p[1].is_empty()),
        ["git", "push", options @ ..] => !input && rule.operations.contains(&Operation::Push) && push(rule, options, root),
        ["gh", "pr", action @ ("create" | "edit"), options @ ..] => pull_request(rule, action, options, input),
        _ => false,
    }
}

fn push(rule: &GrantRecord, args: &[&str], root: &str) -> bool {
    let args = if args.first().is_some_and(|a| matches!(*a, "-u" | "--set-upstream")) { &args[1..] } else { args };
    let [remote, target] = args else { return false; };
    let Some(branch) = rule.branch.as_deref() else { return false; };
    if rule.remote.as_deref() != Some(*remote) { return false; }
    let Some(url) = push_url(root, remote) else { return false; };
    if rule.resolved_push_url.as_ref().is_some_and(|expected| expected != &url)
        || !rule.all_repositories && rule.remote_url.as_ref() != Some(&url) { return false; }
    if *target == branch { return read(root, &["show-ref", "--verify", "--quiet", &format!("refs/heads/{branch}")]).is_some(); }
    if *target == format!("HEAD:refs/heads/{branch}") || *target == format!("HEAD:{branch}") {
        return read(root, &["symbolic-ref", "--quiet", "--short", "HEAD"]).as_deref() == Some(branch);
    }
    false
}

fn pull_request(rule: &GrantRecord, action: &str, args: &[&str], input: bool) -> bool {
    let create = action == "create";
    if !rule.operations.contains(&if create { Operation::PrCreate } else { Operation::PrEdit }) { return false; }
    let (mut repository, mut base, mut body_input) = (None, None, false);
    let (mut title, mut body, mut head, mut target, mut draft) = (false, false, false, false, false);
    let mut i = 0;
    while i < args.len() {
        let arg = args[i];
        if !create && !target && arg.bytes().all(|c| c.is_ascii_digit()) && !arg.is_empty() { target = true; i += 1; continue; }
        if create && arg == "--draft" && !draft { draft = true; i += 1; continue; }
        let Some(value) = args.get(i + 1).copied() else { return false; };
        match arg {
            "--repo" | "-R" if repository.is_none() => repository = Some(value),
            "--title" | "-t" if !title && !value.is_empty() => title = true,
            "--body" | "-b" if !body => body = true,
            "--body-file" | "-F" if !body && value == "-" && input => { body = true; body_input = true; },
            "--base" | "-B" if create && base.is_none() && branch(value) => base = Some(value),
            "--head" | "-H" if create && !head && branch(value) => head = true,
            _ => return false,
        }
        i += 2;
    }
    repository == rule.github_repository.as_ref().map(|r| format!("github.com/{r}")).as_deref() && input == body_input
        && if create { title && body && head && base == rule.base.as_deref() } else { target && (title || body) }
}
