use super::wrappers;
use tree_sitter::{Node, ParseOptions, ParseState, Parser};

type Fragment = Result<Vec<String>, String>;
#[derive(Default)]
struct Budget {
    parse_checks: usize,
    nodes: usize,
    arguments: usize,
}

const UNCERTAIN: &str = "This expression changes command context or control flow. Later literal arguments are shown as possible actions, not verified execution.";

fn literal(node: Node<'_>, script: &str) -> Result<String, String> {
    let text = &script[node.byte_range()];
    let words = super::split(&format!("literal {text}"))?;
    match words.as_slice() {
        [words] if words.len() == 2 => Ok(words[1].clone()),
        _ => Err("This argument is evaluated by the shell at runtime.".into()),
    }
}

fn command(
    node: Node<'_>,
    script: &str,
    fragments: &mut Vec<Fragment>,
    depth: usize,
    budget: &mut Budget,
) -> Result<(), String> {
    let name = node
        .child_by_field_name("name")
        .ok_or("The command name is unavailable.")?;
    let program = literal(name, script)?;
    let mut words = vec![program.clone()];
    let mut scoped = false;
    let mut dynamic = false;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "variable_assignment"
            || matches!(child.kind(), "file_redirect" | "herestring_redirect")
        {
            scoped = true;
        }
    }
    let mut cursor = node.walk();
    for child in node.children_by_field_name("argument", &mut cursor) {
        if matches!(program.as_str(), "env" | "/usr/bin/env")
            && words.len() == 1
            && wrappers::assignment(&script[child.byte_range()])
        {
            scoped = true;
            continue;
        }
        budget.arguments += 1;
        if budget.arguments > 16_384 {
            return Err("Shell arguments exceed the static preview limit.".into());
        }
        words.push(match literal(child, script) {
            Ok(value) => value,
            Err(_) => {
                dynamic = true;
                super::RUNTIME_ARGUMENT.into()
            }
        });
    }
    let (words, wrapped) = wrappers::unwrap(words);
    if scoped || wrapped || dynamic {
        fragments.push(Err(UNCERTAIN.into()));
    }
    if words.len() == 3
        && matches!(words[0].as_str(), "sh" | "bash" | "/bin/sh" | "/bin/bash")
        && words[1] == "-c"
    {
        fragments.push(Err(UNCERTAIN.into()));
        fragments.extend(parse(&words[2], depth + 1, budget)?);
        fragments.push(Err(UNCERTAIN.into()));
    } else {
        fragments.push(Ok(words));
    }
    Ok(())
}

fn visit(
    node: Node<'_>,
    script: &str,
    fragments: &mut Vec<Fragment>,
    depth: usize,
    budget: &mut Budget,
) -> Result<(), String> {
    budget.nodes += 1;
    if depth > 48 || budget.nodes > 8192 || fragments.len() > 2048 {
        return Err(
            "The shell structure exceeds the static preview limit. Review the exact command."
                .into(),
        );
    }
    match node.kind() {
        "comment" => {}
        "command" => {
            if let Err(error) = command(node, script, fragments, depth, budget) {
                fragments.push(Err(error));
            }
        }
        "program" | "list" => {
            let mut cursor = node.walk();
            let uncertain = node
                .children(&mut cursor)
                .any(|child| matches!(child.kind(), "||" | "&"));
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                if uncertain {
                    fragments.push(Err(UNCERTAIN.into()));
                }
                visit(child, script, fragments, depth + 1, budget)?;
            }
            if uncertain {
                fragments.push(Err(UNCERTAIN.into()));
            }
        }
        "redirected_statement" => {
            fragments.push(Err(UNCERTAIN.into()));
            if let Some(body) = node.child_by_field_name("body") {
                visit(body, script, fragments, depth + 1, budget)?;
            }
        }
        "pipeline"
        | "subshell"
        | "do_group"
        | "compound_statement"
        | "if_statement"
        | "elif_clause"
        | "else_clause"
        | "while_statement"
        | "for_statement"
        | "c_style_for_statement"
        | "case_statement"
        | "case_item"
        | "negated_command" => {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                fragments.push(Err(UNCERTAIN.into()));
                visit(child, script, fragments, depth + 1, budget)?;
            }
            fragments.push(Err(UNCERTAIN.into()));
        }
        _ => fragments.push(Err(format!(
            "{} requires runtime interpretation. Review the exact command.",
            node.kind().replace('_', " ")
        ))),
    }
    Ok(())
}

fn parse(script: &str, depth: usize, budget: &mut Budget) -> Result<Vec<Fragment>, String> {
    if script.len() > 131_072 || depth > 48 {
        return Err("The shell text exceeds the static preview limit.".into());
    }
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_bash::LANGUAGE.into())
        .map_err(|_| "The shell parser is unavailable.")?;
    let mut progress = |_: &ParseState| {
        budget.parse_checks += 1;
        budget.parse_checks > 4096
    };
    let tree = parser
        .parse_with_options(
            &mut |offset, _| &script.as_bytes()[offset..],
            None,
            Some(ParseOptions::new().progress_callback(&mut progress)),
        )
        .ok_or("Shell parsing exceeded the static work limit.")?;
    drop(progress);
    if tree.root_node().has_error() {
        return Err("The shell syntax could not be parsed completely. Review the exact command; no partial syntax is treated as reliable.".into());
    }
    let mut fragments = Vec::new();
    visit(
        tree.root_node(),
        script,
        &mut fragments,
        depth,
        budget,
    )?;
    Ok(fragments)
}

pub(super) fn preview(script: &str) -> Result<Vec<Fragment>, String> {
    parse(script, 0, &mut Budget::default())
}

#[cfg(test)]
mod scheduling_tests {
    use super::*;

    #[test]
    fn scheduling_delay_does_not_change_literal_push_detection() {
        let script = "test \"$(git rev-parse HEAD)\" = expected && git push --atomic origin main";
        let mut budget = Budget::default();
        std::thread::sleep(std::time::Duration::from_millis(110));
        let delayed = parse(script, 0, &mut budget);
        assert_eq!(delayed, preview(script));
    }
}
