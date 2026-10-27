use super::*;
use types::BodySource;

fn details(script: &str, input: Option<&str>) -> Vec<Detail> {
    let root = tempfile::tempdir().unwrap();
    inspect(root.path(), &[CommandStep { program: "shell".into(), argv: vec!["-c".into(),script.into()], stdin: input.map(str::to_string), timeout_seconds: None }]).unwrap()
}

#[test]
fn receipt_command_shows_the_pr_target_repository_title_and_supplied_description() {
    let script = "gh pr edit 1 --repo Pedi-Solutions-Inc/kusina --title \"Rewrite stage prompts, cut agent token use, and add a persistent dev stack\" --body-file -";
    let body = format!("## Summary\n\n{}\n\n- Keep **Markdown** and 日本語 exactly.\n", "x".repeat(5400));
    let preview = details(script, Some(&body));
    assert_eq!(preview.len(),1);
    let pr = &preview[0];
    assert_eq!(pr.kind,"pr.edit");assert_eq!(pr.title,"Update pull request");
    assert_eq!(pr.pull_request_target.as_deref(),Some("1"));
    assert_eq!(pr.repository.as_deref(),Some("Pedi-Solutions-Inc/kusina"));
    assert_eq!(pr.pull_request_title.as_deref(),Some("Rewrite stage prompts, cut agent token use, and add a persistent dev stack"));
    assert_eq!(pr.body.as_deref(),Some(body.as_str()));assert_eq!(pr.body_source,Some(BodySource::Stdin));
    assert!(pr.head_branch.is_none());assert!(pr.base_branch.is_none());
}

#[test]
fn create_and_edit_support_stdin_options_direct_and_through_a_single_literal_shell_command() {
    for action in ["create","edit 1"] {
        for flag in ["--body-file -","--body-file=-","-F -","-F-"] {
            let script = format!("gh pr {action} -R owner/repo --title Title {flag}");
            assert_eq!(details(&script,Some(""))[0].body.as_deref(),Some(""));
            let root=tempfile::tempdir().unwrap();
            let preview=inspect(root.path(),&[CommandStep{program:"gh".into(),argv:script.split_whitespace().skip(1).map(str::to_string).collect(),stdin:Some("# Direct body".into()),timeout_seconds:None}]).unwrap();
            assert_eq!(preview[0].body.as_deref(),Some("# Direct body"));
        }
    }
}

#[test]
fn shell_consumers_redirects_and_pipes_never_claim_the_supplied_input_is_the_pr_body() {
    for script in ["cat && gh pr edit 1 --body-file -","printf generated | gh pr edit 1 --body-file -","gh pr edit 1 --body-file - < alternate.txt","gh pr edit 1 --body-file - && gh pr edit 2 --body-file -","test ready = ready && gh pr edit 1 --body-file -"] {
        let preview=details(script,Some("Supplied input"));
        let pr=preview.iter().find(|detail|detail.kind=="pr.edit").unwrap();
        assert!(pr.body.is_none(),"{script}");
        assert!(pr.warnings.iter().any(|warning|warning.contains("cannot be attributed")));
    }
}

#[test]
fn missing_input_is_distinguished_from_empty_and_literal_body_text_is_not_replaced() {
    let preview=details("gh pr edit 1 --body-file -",None);
    assert!(preview[0].body.is_none());assert!(preview[0].warnings.iter().any(|warning|warning.contains("no standard input")));
    let preview=details("gh pr edit 1 --body 'literal description'",Some("unrelated input"));
    assert_eq!(preview[0].body.as_deref(),Some("literal description"));
    assert_eq!(preview[0].body_source,Some(BodySource::Argument));
}

#[test]
fn unknown_option_values_are_not_mistaken_for_a_pr_target_and_edit_does_not_invent_a_branch_change() {
    let preview=details("gh pr edit --add-label bug --title Title",None);
    assert!(preview[0].pull_request_target.is_none());assert!(preview[0].head_branch.is_none());
    let preview=details("gh pr edit https://github.com/owner/repo/pull/23 --base next --body changed",None);
    assert_eq!(preview[0].pull_request_target.as_deref(),Some("https://github.com/owner/repo/pull/23"));
    assert_eq!(preview[0].base_branch.as_deref(),Some("next"));
}

#[test]
fn oversized_stdin_keeps_the_exact_input_available_without_overloading_the_markdown_preview() {
    let preview=details("gh pr edit 1 --body-file -",Some(&"x".repeat(64_001)));
    assert!(preview[0].body.is_none());
    assert!(preview[0].warnings.iter().any(|warning|warning.contains("64 KB")));
}
