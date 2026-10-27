pub(super) fn assignment(word: &str) -> bool {
    word.split_once('=').is_some_and(|(name, _)| {
        !name.is_empty()
            && name.bytes().enumerate().all(|(index, ch)| {
                ch == b'_' || ch.is_ascii_alphabetic() || index > 0 && ch.is_ascii_digit()
            })
    })
}

pub(super) fn unwrap(mut words: Vec<String>) -> (Vec<String>, bool) {
    let mut scoped = false;
    for _ in 0..8 {
        let Some(program) = words.first().map(String::as_str) else {
            break;
        };
        let mut index = 1;
        match program {
            "env" | "/usr/bin/env" => {
                while let Some(word) = words.get(index) {
                    if word == "--" {
                        index += 1;
                        break;
                    }
                    if assignment(word)
                        || matches!(word.as_str(), "-i" | "--ignore-environment")
                        || word.starts_with("--unset=")
                    {
                        index += 1;
                    } else if matches!(word.as_str(), "-u" | "--unset")
                        && words.get(index + 1).is_some()
                    {
                        index += 2;
                    } else if word.starts_with('-') {
                        return (words, scoped);
                    } else {
                        break;
                    }
                }
            }
            "command" => {
                if words.get(index).is_some_and(|word| word == "-p") {
                    index += 1;
                }
                if words.get(index).is_some_and(|word| word == "--") {
                    index += 1;
                }
                if words.get(index).is_some_and(|word| word.starts_with('-')) {
                    return (words, scoped);
                }
            }
            _ => break,
        }
        if index >= words.len() {
            break;
        }
        words = words[index..].to_vec();
        scoped = true;
    }
    if let Some(program) = words.first_mut() {
        if program.starts_with('/') && matches!(program.rsplit('/').next(), Some("git" | "gh")) {
            *program = program.rsplit('/').next().unwrap().into();
            scoped = true;
        }
    }
    (words, scoped)
}
