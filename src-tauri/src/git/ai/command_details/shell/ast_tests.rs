use super::preview;

fn words(script: &str) -> Vec<Vec<String>> {
    preview(script)
        .unwrap()
        .into_iter()
        .filter_map(Result::ok)
        .collect()
}

#[test]
fn preparation_commands_and_wrappers_preserve_git_arguments() {
    for prefix in [
        "python3 -c 'import hashlib; assert True' && ",
        "NODE_OPTIONS=--no-experimental-webstorage node check > /tmp/check.log 2>&1 && ",
        "export PATH=/usr/bin; ",
        "test \"$(git rev-parse HEAD)\" = expected && ",
        "if test -f ready; then printf ready; fi; ",
        "cat <<'END'\ngit push origin fake\nEND\n",
        "printf ready | cat; ",
        "printf ready | cat\n",
    ] {
        let script =
            format!("{prefix}git commit -m 'Reviewed message' && git push origin HEAD:feat/test");
        let commands = words(&script);
        assert!(
            commands
                .iter()
                .any(|words| words == &["git", "commit", "-m", "Reviewed message"]),
            "{prefix}"
        );
        assert!(
            commands
                .iter()
                .any(|words| words == &["git", "push", "origin", "HEAD:feat/test"]),
            "{prefix}"
        );
        assert!(!commands
            .iter()
            .any(|words| words == &["git", "push", "origin", "fake"]));
    }
    for wrapper in [
        "env TOKEN=example ",
        "env TOKEN=\"$TOKEN\" ",
        "env -i -- ",
        "command -- ",
        "command -p ",
        "/usr/bin/",
    ] {
        assert!(
            words(&format!("{wrapper}git commit -m Actual"))
                .iter()
                .any(|words| words == &["git", "commit", "-m", "Actual"]),
            "{wrapper}"
        );
    }
    assert!(!words("command -v git commit -m Fake")
        .iter()
        .any(|words| words.first().is_some_and(|word| word == "git")));
}

#[test]
fn shell_bodies_are_possible_actions_but_quoted_payloads_and_definitions_are_not() {
    for script in [
        "if ready; then git push origin feature; fi",
        "for i in a b; do git push origin feature; done",
        "(git push origin feature)",
        "sh -c 'git push origin feature'",
        "git push origin feature | cat",
    ] {
        let fragments = preview(script).unwrap();
        assert!(fragments.iter().any(Result::is_err), "{script}");
        assert!(
            fragments
                .iter()
                .filter_map(|entry| entry.as_ref().ok())
                .any(|words| words == &["git", "push", "origin", "feature"]),
            "{script}"
        );
    }
    for script in [
        "printf 'git push origin fake'",
        "python3 -c 'git push origin fake'",
        "f() { git push origin fake; }; f",
        "git commit -m \"$(printf 'git push origin fake')\"",
        "cat <<'EOF'\ngit push origin fake\nEOF",
    ] {
        assert!(
            !words(script)
                .iter()
                .any(|words| words == &["git", "push", "origin", "fake"]),
            "{script}"
        );
    }
}

#[test]
fn generated_quoted_messages_roundtrip_without_creating_actions() {
    let alphabet = [
        'a', '\'', '"', '$', '`', '&', '|', ';', '\n', '\\', 'é', '界', '>', '<', '(', ')', '#',
        ' ',
    ];
    let mut seed = 73_u64;
    for count in 0..512 {
        let mut message = String::from("git push origin fake ");
        for _ in 0..count % 80 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            message.push(alphabet[(seed >> 32) as usize % alphabet.len()]);
        }
        let quoted = format!("'{}'", message.replace('\'', "'\"'\"'"));
        let commands = words(&format!(
            "python3 -c 'pass' && git commit -m {quoted} && git push origin feature"
        ));
        assert_eq!(commands.len(), 3);
        assert_eq!(commands[1], ["git", "commit", "-m", message.as_str()]);
        assert_eq!(commands[2], ["git", "push", "origin", "feature"]);
    }
}
