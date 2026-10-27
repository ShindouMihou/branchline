use super::instruction_files::{span, START};
use super::*;
fn setup() -> (tempfile::TempDir, Installer) {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    fs::create_dir_all(home.join(".codex")).unwrap();
    fs::create_dir_all(home.join(".claude")).unwrap();
    let installer = Installer::new(home.clone(), home.join("app-data"));
    (dir, installer)
}
fn install(installer: &Installer, reminders: Vec<String>) -> Value {
    installer
        .handle(Request::Finish {
            targets: vec!["agents".into(), "claude".into()],
            instructions: reminders,
        })
        .unwrap()
}
#[test]
fn status_and_skill_only_install_never_inject_instructions() {
    let (_dir, installer) = setup();
    let path = installer.home.join(".claude/CLAUDE.md");
    fs::write(&path, "Existing private instructions").unwrap();
    assert_eq!(
        installer.handle(Request::Status).unwrap()["instructions"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    install(&installer, Vec::new());
    installer.auto_update().unwrap();
    assert_eq!(
        fs::read_to_string(path).unwrap(),
        "Existing private instructions"
    );
    assert!(!installer.home.join(".codex/AGENTS.md").exists());
}
#[test]
fn opt_in_is_idempotent_and_preserves_unrelated_text() {
    let (_dir, installer) = setup();
    let path = installer.home.join(".claude/CLAUDE.md");
    let original = "# My rules\nKeep existing preferences.\n";
    fs::write(&path, original).unwrap();
    assert_eq!(
        install(&installer, vec!["claude".into()])["results"][2]["ok"],
        true
    );
    let first = fs::read_to_string(&path).unwrap();
    assert!(first.starts_with(original));
    assert!(first.contains("~/.claude/skills/branchline-git-approval/SKILL.md"));
    assert_eq!(first.matches(START).count(), 1);
    install(&installer, vec!["claude".into()]);
    assert_eq!(fs::read_to_string(&path).unwrap(), first);
    assert!(!installer.home.join(".codex/AGENTS.md").exists());
    assert!(installer.data.join("instruction-backups").is_dir());
}
#[test]
fn removal_requires_confirmation_and_restores_the_original_file() {
    let (_dir, installer) = setup();
    let path = installer.home.join(".codex/AGENTS.md");
    let original = "User rules without a trailing newline";
    fs::write(&path, original).unwrap();
    install(&installer, vec!["codex".into()]);
    assert!(installer
        .handle(Request::RemoveInstructions {
            target: "codex".into(),
            confirmed: false
        })
        .is_err());
    installer
        .handle(Request::RemoveInstructions {
            target: "codex".into(),
            confirmed: true,
        })
        .unwrap();
    assert_eq!(fs::read_to_string(path).unwrap(), original);
}
#[test]
fn removal_keeps_later_user_edits_outside_the_block() {
    let (_dir, installer) = setup();
    let path = installer.home.join(".claude/CLAUDE.md");
    install(&installer, vec!["claude".into()]);
    let text = fs::read_to_string(&path).unwrap();
    fs::write(&path, format!("New prefix\n{text}\nNew suffix")).unwrap();
    installer.remove_instructions("claude").unwrap();
    assert_eq!(
        fs::read_to_string(path).unwrap(),
        "New prefix\n\nNew suffix"
    );
}
#[test]
fn customized_duplicate_malformed_and_unowned_blocks_are_kept() {
    let (_dir, installer) = setup();
    let path = installer.home.join(".claude/CLAUDE.md");
    install(&installer, vec!["claude".into()]);
    let installed = fs::read_to_string(&path).unwrap();
    for text in [
        installed.replace("Git and GitHub approvals", "My edits"),
        format!("{installed}{installed}"),
        "<!-- branchline:integration --!>\nMy instructions".to_string(),
    ] {
        fs::write(&path, &text).unwrap();
        assert!(installer.install_instructions("claude").is_err());
        assert!(installer.remove_instructions("claude").is_err());
        installer.auto_update().unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), text);
    }
    fs::write(&path, &installed).unwrap();
    fs::remove_file(installer.instruction_receipt_path("claude")).unwrap();
    assert!(installer.install_instructions("claude").is_err());
    assert_eq!(fs::read_to_string(path).unwrap(), installed);
}
#[test]
fn only_owned_older_blocks_update_and_outside_bytes_are_unchanged() {
    let (_dir, installer) = setup();
    let path = installer.home.join(".claude/CLAUDE.md");
    fs::write(&path, "Before\r\n").unwrap();
    install(&installer, vec!["claude".into()]);
    let text = fs::read_to_string(&path).unwrap();
    let range = span(&text).unwrap().unwrap();
    let old = format!(
        "{}\r\nOld reminder\r\n{}\r\n",
        START,
        instruction_files::END
    );
    let updated = format!(
        "{}{}{}After\r\n",
        &text[..range.start],
        old,
        &text[range.end..]
    );
    fs::write(&path, &updated).unwrap();
    let receipt_path = installer.instruction_receipt_path("claude");
    let mut receipt: Value = serde_json::from_slice(&fs::read(&receipt_path).unwrap()).unwrap();
    receipt["revision"] = json!(instructions::REVISION - 1);
    receipt["block_hash"] = json!(hash(&old));
    fs::write(&receipt_path, serde_json::to_vec(&receipt).unwrap()).unwrap();
    installer.auto_update().unwrap();
    let result = fs::read_to_string(&path).unwrap();
    assert!(result.starts_with(&text[..range.start]));
    assert!(result.ends_with("After\r\n"));
    assert!(!result.contains("Old reminder"));
    assert!(!result.replace("\r\n", "").contains('\n'));
}
#[test]
fn removed_sections_do_not_reappear_on_startup_and_missing_skills_are_not_linked() {
    let (_dir, installer) = setup();
    assert!(
        installer
            .handle(Request::Finish {
                targets: Vec::new(),
                instructions: vec!["claude".into()]
            })
            .unwrap()["results"][0]["ok"]
            == false
    );
    install(&installer, vec!["claude".into()]);
    let path = installer.home.join(".claude/CLAUDE.md");
    fs::write(&path, "User removed the reminder").unwrap();
    installer.auto_update().unwrap();
    assert_eq!(
        fs::read_to_string(path).unwrap(),
        "User removed the reminder"
    );
    assert!(installer
        .handle(Request::Finish {
            targets: Vec::new(),
            instructions: vec!["../../outside".into()]
        })
        .is_err());
}
#[test]
fn linked_instruction_files_and_concurrent_edits_are_refused() {
    let (_dir, installer) = setup();
    install(&installer, Vec::new());
    let path = installer.home.join(".claude/CLAUDE.md");
    let outside = installer.home.join("outside.md");
    fs::write(&outside, "Keep").unwrap();
    std::os::unix::fs::symlink(&outside, &path).unwrap();
    assert!(installer.install_instructions("claude").is_err());
    fs::remove_file(&path).unwrap();
    fs::hard_link(&outside, &path).unwrap();
    assert!(installer.install_instructions("claude").is_err());
    fs::remove_file(&path).unwrap();
    fs::write(&path, "Initial").unwrap();
    let before = instruction_files::read(&installer.home, &path).unwrap();
    fs::write(&path, "Changed by editor").unwrap();
    assert!(instruction_files::write(&installer.home, &path, &before, "Replacement").is_err());
    assert_eq!(fs::read_to_string(path).unwrap(), "Changed by editor");
    assert_eq!(fs::read_to_string(outside).unwrap(), "Keep");
}

#[test]
fn codex_override_and_receipt_failures_preserve_user_instructions() {
    let (_dir, installer) = setup();
    install(&installer, Vec::new());
    let path = installer.home.join(".codex/AGENTS.md");
    fs::write(&path, "Existing rules").unwrap();
    fs::write(
        installer.home.join(".codex/AGENTS.override.md"),
        "Active rules",
    )
    .unwrap();
    assert!(installer.install_instructions("codex").is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), "Existing rules");
    fs::remove_file(installer.home.join(".codex/AGENTS.override.md")).unwrap();
    fs::create_dir_all(installer.instruction_receipt_path("codex")).unwrap();
    assert!(installer.install_instructions("codex").is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), "Existing rules");
}
