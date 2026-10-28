use tree_sitter::{Node, Parser, ParseOptions, ParseState};

pub(super) fn commands(script: &str) -> Option<Vec<Vec<String>>> {
    if script.len() > 32_768 { return None; }
    let mut parser = Parser::new();
    parser.set_language(&tree_sitter_bash::LANGUAGE.into()).ok()?;
    let mut checks = 0usize;
    let mut progress = |_: &ParseState| { checks += 1; checks > 4096 };
    let tree = parser.parse_with_options(&mut |offset, _| &script.as_bytes()[offset..], None, Some(ParseOptions::new().progress_callback(&mut progress)))?;
    if tree.root_node().has_error() { return None; }
    let mut result = Vec::new();
    visit(tree.root_node(), script, &mut result, 0)?;
    (!result.is_empty()).then_some(result)
}

fn visit(node: Node<'_>, script: &str, result: &mut Vec<Vec<String>>, depth: usize) -> Option<()> {
    if depth > 32 || result.len() >= 64 { return None; }
    match node.kind() {
        "program" | "list" => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.is_named() { visit(child, script, result, depth + 1)?; }
                else if !matches!(&script[child.byte_range()], ";" | "&&") { return None; }
            }
        }
        "command" => {
            let mut cursor = node.walk();
            for (index, child) in node.children(&mut cursor).enumerate() {
                let field = node.field_name_for_child(index as u32);
                if !child.is_named() || !matches!(field, Some("name" | "argument")) || !literal(child, script) { return None; }
            }
            let mut words = super::shell::split(&script[node.byte_range()]).ok()?;
            if words.len() != 1 { return None; }
            result.push(words.remove(0));
        }
        _ => return None,
    }
    Some(())
}

fn literal(node: Node<'_>, script: &str) -> bool {
    match node.kind() {
        "word" => !script[node.byte_range()].contains(['{', '}']),
        "raw_string" | "string" => true,
        "number" => script[node.byte_range()].bytes().all(|c| c.is_ascii_digit()),
        "command_name" | "concatenation" => {
            let mut cursor = node.walk();
            let valid = node.named_children(&mut cursor).all(|child| literal(child, script));
            valid
        }
        _ => false,
    }
}
