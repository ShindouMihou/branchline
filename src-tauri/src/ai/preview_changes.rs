use crate::git::ai::Preview;
use serde_json::Value;

fn text(value: &Value) -> String {
    let value = value.as_str().unwrap_or_default();
    let display = value.chars().take(120).collect::<String>();
    format!("{display}{}", if value.chars().count() > 120 { "…" } else { "" })
}

fn names(value: &Value) -> Vec<String> {
    value.as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect()
}

fn files(before: &Value, after: &Value, label: &str, changes: &mut Vec<String>) {
    let before = names(before);
    let after = names(after);
    for (verb, values, other) in [("added", &after, &before), ("removed", &before, &after)] {
        let existing = other.iter().collect::<std::collections::HashSet<_>>();
        let changed = values.iter().filter(|value| !existing.contains(value)).collect::<Vec<_>>();
        if !changed.is_empty() {
            let examples = changed.iter().take(4).map(|value| value.as_str()).collect::<Vec<_>>().join(", ");
            changes.push(format!("{label}: {} {verb} ({examples}{}).", changed.len(), if changed.len() > 4 { ", …" } else { "" }));
        }
    }
}

fn field(key: &str, before: &Value, after: &Value, context: &str, changes: &mut Vec<String>) {
    if before == after { return; }
    let label = match key {
        "branch" | "sourceBranch" => "Source branch",
        "targetBranch" => "Destination branch",
        "baseBranch" | "base" => "Base branch",
        "headBranch" => "Pull request branch",
        "head" => "HEAD / source revision",
        "message" => "Commit message",
        "pullRequestTitle" => "Pull request title",
        "body" => "Pull request description",
        "files" => "File selection",
        "assets" => "Release assets",
        "root" | "workingDirectory" => "Working directory",
        "remote" => "Remote",
        "destination" => "Publishing destination",
        "expectedOid" => "Remote branch revision",
        "stagedToken" | "patch" => "Staged contents",
        "filePreviews" => "File preview sources",
        "repositoryIdentity" | "gitDir" | "commonDir" | "rootDevice" | "rootInode" => "Repository identity",
        "executables" => "Command executable identity",
        "warnings" => "Preview notes",
        "evidence" | "evidenceKind" => "Preview confidence",
        "title" => "Action title",
        _ => key,
    };
    let label = format!("{context}{label}");
    if matches!(key, "files" | "assets") {
        let count = changes.len();
        files(before, after, &label, changes);
        if count != changes.len() { return; }
    }
    let detail = if key == "body" {
        format!("changed ({} → {} characters)", before.as_str().map_or(0, |s| s.chars().count()), after.as_str().map_or(0, |s| s.chars().count()))
    } else if matches!(key, "branch" | "sourceBranch" | "targetBranch" | "baseBranch" | "base" | "headBranch" | "head" | "message" | "pullRequestTitle" | "root" | "workingDirectory" | "remote" | "destination" | "expectedOid") {
        format!("{} → {}", if before.is_null() { "Not determined".into() } else { text(before) }, if after.is_null() { "Not determined".into() } else { text(after) })
    } else {
        "changed".into()
    };
    changes.push(format!("{label}: {detail}."));
}

fn object(before: &Value, after: &Value, context: &str, changes: &mut Vec<String>) {
    let mut keys = std::collections::BTreeSet::new();
    for value in [before, after] {
        if let Some(value) = value.as_object() { keys.extend(value.keys()); }
    }
    for key in keys { field(key, &before[key], &after[key], context, changes); }
}

pub(super) fn explain(before: &Preview, after: &Preview) -> String {
    let before = serde_json::to_value(before).expect("Preview is serializable");
    let after = serde_json::to_value(after).expect("Preview is serializable");
    let mut changes = Vec::new();
    for (verb, current, other) in [("New", &after["risks"], &before["risks"]), ("Removed", &before["risks"], &after["risks"])] {
        for risk in current.as_array().into_iter().flatten() {
            let previous = other.as_array().into_iter().flatten().find(|item| item["code"] == risk["code"]);
            match previous {
                None => changes.push(format!("{verb} warning: {}.", text(&risk["title"]))),
                Some(previous) if verb == "New" && previous != risk => changes.push(format!("Warning updated: {}.", text(&risk["title"]))),
                _ => {},
            }
        }
    }
    if before["commandWorkflow"] != after["commandWorkflow"] {
        object(&before["commandWorkflow"], &after["commandWorkflow"], "Execution context · ", &mut changes);
    }
    let old_details = before["commandDetails"].as_array().cloned().unwrap_or_default();
    let new_details = after["commandDetails"].as_array().cloned().unwrap_or_default();
    for index in 0..old_details.len().max(new_details.len()) {
        let old = old_details.get(index).unwrap_or(&Value::Null);
        let new = new_details.get(index).unwrap_or(&Value::Null);
        let title = if new.is_null() { &old["title"] } else { &new["title"] };
        let context = format!("Action {} · {} · ", index + 1, text(title));
        if old.is_null() || new.is_null() {
            changes.push(format!("Action {}: {} {}.", index + 1, text(title), if old.is_null() { "became available" } else { "is no longer available" }));
        } else {
            object(old, new, &context, &mut changes);
        }
    }
    for (key, _) in before.as_object().into_iter().flatten() {
        match key.as_str() {
            "fingerprint" | "risks" | "commandWorkflow" | "commandDetails" => {},
            "push" | "pullRequest" if before[key] != after[key] => object(&before[key], &after[key], if key == "push" { "Push · " } else { "Pull request · " }, &mut changes),
            _ => field(key, &before[key], &after[key], "", &mut changes),
        }
    }
    changes.dedup();
    if changes.is_empty() {
        changes.push("The preview fingerprint changed, but its saved fields match. This may be a preview consistency issue; no repository change was confirmed.".into());
    }
    let remaining = changes.len().saturating_sub(12);
    changes.truncate(12);
    if remaining > 0 { changes.push(format!("{remaining} additional preview fields changed.")); }
    format!("Approval stopped because the preview changed:\n{}\nNo command ran. Ask the AI for a fresh request to review these changes.", changes.iter().map(|change| format!("• {change}")).collect::<Vec<_>>().join("\n"))
}
