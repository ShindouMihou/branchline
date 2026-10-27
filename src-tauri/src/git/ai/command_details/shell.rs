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
