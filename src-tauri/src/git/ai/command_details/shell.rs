pub(super) fn split(script: &str) -> Result<Vec<Vec<String>>, String> {
    let mut chars = script.chars().peekable();
    let mut commands = Vec::new();
    let mut words = Vec::new();
    let mut word = String::new();
    let mut started = false;
    let mut quote = None;
    while let Some(ch) = chars.next() {
        if quote == Some('\'') {
            if ch == '\'' {
                quote = None;
            } else {
                word.push(ch);
            }
            continue;
        }
        if ch == '\\' {
            let next = chars
                .next()
                .ok_or("The shell text ends with an incomplete escape.")?;
            if next != '\n' {
                if quote == Some('"') && !matches!(next, '$' | '`' | '"' | '\\') {
                    word.push('\\');
                }
                word.push(next);
                started = true;
            }
            continue;
        }
        if matches!(ch, '$' | '`') {
            return Err("Shell expansion cannot be resolved before this command runs.".into());
        }
        if quote == Some('"') {
            if ch == '"' {
                quote = None;
            } else {
                word.push(ch);
            }
            continue;
        }
        if matches!(ch, '\'' | '"') {
            quote = Some(ch);
            started = true;
            continue;
        }
        if ch == '#' && !started {
            while chars.peek().is_some_and(|ch| *ch != '\n') {
                chars.next();
            }
            continue;
        }
        if matches!(ch, '|' | '<' | '>' | '(' | ')' | '*' | '?' | '[') || ch == '~' && !started {
            return Err("This shell expression needs runtime interpretation; its structured details are unavailable.".into());
        }
        let separator = ch == ';' || ch == '\n' || ch == '&';
        if ch == '&' && chars.next() != Some('&') {
            return Err(
                "Background shell commands have no reliable static sequence preview.".into(),
            );
        }
        if ch.is_whitespace() || separator {
            if started {
                words.push(std::mem::take(&mut word));
                started = false;
            }
            if separator && !words.is_empty() {
                commands.push(std::mem::take(&mut words));
            }
        } else {
            word.push(ch);
            started = true;
        }
    }
    if quote.is_some() {
        return Err("The shell text has an unfinished quote.".into());
    }
    if started {
        words.push(word);
    }
    if !words.is_empty() {
        commands.push(words);
    }
    Ok(commands)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_literal_quotes_and_separators_without_expansion() {
        assert_eq!(split("git commit -m 'Hello && world' && git switch \"new branch\"; gh pr create --title='A title'").unwrap(), vec![vec!["git","commit","-m","Hello && world"],vec!["git","switch","new branch"],vec!["gh","pr","create","--title=A title"]]);
        assert_eq!(
            split("git commit -m 'literal $variable' # ignored\ngit status")
                .unwrap()
                .len(),
            2
        );
        for input in [
            "git commit -m \"$MESSAGE\"",
            "git add *.rs",
            "git status | cat",
            "git status &",
            "git show $(whoami)",
            "git status > output",
        ] {
            assert!(split(input).is_err());
        }
    }
}

pub(super) const RUNTIME_ARGUMENT: &str = "\0branchline_runtime\0";
mod ast;
mod wrappers;
#[cfg(test)]
mod ast_tests;
pub(super) fn preview(script: &str) -> Result<Vec<Result<Vec<String>, String>>, String> {
    ast::preview(script)
}

#[cfg(test)]
mod recovery_tests {
    use super::*;
    #[test]
    fn command_substitutions_and_nested_quotes_do_not_leak_inner_commands() {
        for expression in [
            "\"$(printf '%s' 'fake && git push origin hidden')\"",
            "\"$(printf '%s' \"$(printf ') && git push origin hidden')\")\"",
            "\"${VALUE:-'fake && git push origin hidden'}\"",
            "\"$((1 + (2 * 3)))\"",
        ] {
            let commands = preview(&format!("test {expression} = ready && git commit -m Actual")).unwrap();
            assert!(commands[0].is_err());
            assert_eq!(commands.last().unwrap().as_ref().unwrap(), &["git", "commit", "-m", "Actual"]);
        }
    }
    #[test]
    fn malformed_shell_syntax_remains_wholly_unavailable() {
        for script in [
            "test \"$(echo unfinished\" && git push origin fake",
            "if true; then git push origin fake",
            "git push origin fake &&",
        ] { assert!(preview(script).is_err(), "{script}"); }
    }
    #[test]
    fn comments_and_continuations_do_not_create_extra_actions() {
        let commands = preview("test \"$(printf ready # && git push origin hidden\n)\" = ready &&\\\n git commit -m 'Actual' # && git push origin hidden\n").unwrap();
        assert!(commands.iter().any(Result::is_err));
        assert_eq!(commands.last().unwrap().as_ref().unwrap(), &["git", "commit", "-m", "Actual"]);
        let deeply_nested = format!("test \"{}echo ready{}\" && git push origin hidden", "$(".repeat(40), ")".repeat(40));
        let commands = preview(&deeply_nested).unwrap();
        assert!(commands.iter().any(Result::is_err));
        assert!(commands.iter().filter_map(|entry| entry.as_ref().ok()).any(|words| words == &["git", "push", "origin", "hidden"]));
    }
}

#[cfg(test)]
mod redirected_tests {
    use super::*;
    #[test]
    fn redirection_targets_never_become_preview_commands() {
        for script in [
            "node check > '/tmp/log && git push origin hidden' 2>&1 && git commit -m Actual",
            "node check >>\"$(printf '%s' 'fake && git push origin hidden')\" 2>&1 && git commit -m Actual",
        ] {
            let commands = preview(script).unwrap();
            assert!(commands.iter().any(Result::is_err));
            let commands = commands.into_iter().filter_map(Result::ok).collect::<Vec<_>>();
            assert_eq!(commands.last().unwrap(), &["git", "commit", "-m", "Actual"]);
            assert!(!commands.iter().any(|words| words == &["git", "push", "origin", "hidden"]));
        }
        let commands = preview("cat <<EOF\ngit push origin hidden\nEOF").unwrap();
        assert!(commands.iter().filter_map(|entry| entry.as_ref().ok()).all(|words| words != &["git", "push", "origin", "hidden"]));
    }

    #[test]
    fn redirected_checks_keep_later_commit_and_push_visible() {
        let commands = preview("test \"$(git rev-parse HEAD)\" = expected && node -e 'run()' > /tmp/check.log 2>&1 && git commit -m 'Actual commit' && git push origin HEAD:feature").unwrap();
        assert!(commands.iter().any(|entry| entry.as_ref().is_ok_and(|words| words == &["git", "commit", "-m", "Actual commit"])));
        assert!(commands.iter().any(|entry| entry.as_ref().is_ok_and(|words| words == &["git", "push", "origin", "HEAD:feature"])));
    }
}
