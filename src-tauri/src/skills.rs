use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};
const CONTENT: &str = include_str!("../../skills/branchline-git-approval/SKILL.md");
mod updates;
mod instructions;
mod instruction_actions;
mod instruction_files;
#[cfg(test)]
mod instruction_tests;
const REVISION: u64 = 6;
const NAME: &str = "branchline-git-approval";
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Target {
    id: String,
    name: String,
    path: String,
    status: String,
    detail: String,
}
pub struct Installer {
    home: PathBuf,
    data: PathBuf,
    gate: Mutex<()>,
    last_auto_update: Mutex<Option<Value>>,
}
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    Status,
    Finish { targets: Vec<String>, #[serde(default)] instructions: Vec<String> },
    RemoveInstructions { target: String, confirmed: bool },
    Dismiss,
}
fn hash(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}
fn safe_path(home: &Path, path: &Path) -> Result<(), String> {
    let relative = path
        .strip_prefix(home)
        .map_err(|_| "Skill location is outside your home folder")?;
    let mut current = home.to_path_buf();
    for part in relative.components() {
        if !matches!(part, std::path::Component::Normal(_)) {
            return Err("Invalid skill location".into());
        }
        current.push(part);
        if let Ok(meta) = fs::symlink_metadata(&current) {
            if meta.file_type().is_symlink() {
                return Err("This skill location uses a symbolic link; install it manually".into());
            }
        }
    }
    Ok(())
}
impl Installer {
    pub fn new(home: PathBuf, data: PathBuf) -> Self {
        Self {
            home,
            data,
            gate: Mutex::new(()),
            last_auto_update: Mutex::new(None),
        }
    }
    fn locations(&self) -> Vec<(String, String, PathBuf)> {
        let has = |name: &str| self.home.join(name).is_dir();
        let codex = has(".codex") || Path::new("/Applications/Codex.app").is_dir();
        let chatgpt = Path::new("/Applications/ChatGPT.app").is_dir()
            || self.home.join("Applications/ChatGPT.app").is_dir();
        let cursor = has(".cursor") || Path::new("/Applications/Cursor.app").is_dir();
        let mut result = Vec::new();
        if codex || cursor || chatgpt || self.home.join(".agents/skills").join(NAME).exists() {
            let name = [
                (chatgpt, "ChatGPT Desktop"),
                (codex, "Codex"),
                (cursor, "Cursor"),
            ]
            .into_iter()
            .filter_map(|(detected, name)| detected.then_some(name))
            .collect::<Vec<_>>()
            .join(" · ");
            result.push((
                "agents".into(),
                if name.is_empty() {
                    "Shared AI clients".into()
                } else {
                    name
                },
                self.home.join(".agents/skills").join(NAME),
            ));
        }
        if has(".claude") {
            result.push((
                "claude".into(),
                "Claude Code".into(),
                self.home.join(".claude/skills").join(NAME),
            ));
        }
        result
    }
    fn target(&self, id: String, name: String, folder: PathBuf) -> Target {
        let path = folder.join("SKILL.md");
        let (status, detail) = if let Err(error) = safe_path(&self.home, &path) {
            ("conflict", error)
        } else if !folder.exists() {
            ("available", "Available in all local projects".into())
        } else {
            match fs::read_to_string(&path) {
                Ok(text) if text == CONTENT => ("installed", "Up to date".into()),
                Ok(text) => match self.receipt(&folder, &text) {
                    Ok(Some(receipt)) if receipt.revision > REVISION => {
                        ("installed", "Newer guide installed; kept as-is".into())
                    }
                    Ok(Some(receipt)) if receipt.revision < REVISION => {
                        ("update", "Update available".into())
                    }
                    _ => (
                        "conflict",
                        "Existing skill kept. It was edited locally or needs manual review.".into(),
                    ),
                },
                Err(_) => (
                    "conflict",
                    "Existing folder kept. Choose a different name manually.".into(),
                ),
            }
        };
        Target {
            id,
            name,
            path: path.to_string_lossy().into(),
            status: status.into(),
            detail,
        }
    }
    pub fn handle(&self, request: Request) -> Result<Value, String> {
        let _guard = self
            .gate
            .lock()
            .map_err(|_| "Skill installer unavailable")?;
        match request {
            Request::Status => {
                let targets = self
                    .locations()
                    .into_iter()
                    .map(|(id, name, path)| self.target(id, name, path))
                    .collect::<Vec<_>>();
                Ok(
                    json!({"shouldPrompt":!self.data.join("skills-onboarding.json").exists()&&!targets.is_empty(),"targets":targets,"content":CONTENT,"autoUpdate":self.last_report(),"instructions":self.instruction_targets()}),
                )
            }
            Request::Dismiss => {
                self.remember()?;
                Ok(json!({"dismissed":true}))
            }
            Request::RemoveInstructions { target, confirmed } => {
                if !confirmed { return Err("Confirm removal of the Branchline reminder first".into()); }
                self.remove_instructions(&target)?;
                Ok(json!({"removed":true}))
            }
            Request::Finish { targets, instructions } => {
                if targets.is_empty() && instructions.is_empty() {
                    return Err("Select an AI client first".into());
                }
                self.validate_instruction_targets(&instructions)?;
                let locations = self.locations();
                let mut results = Vec::new();
                for id in &targets {
                    if !locations.iter().any(|(known, _, _)| known == id) {
                        return Err("Unknown or unavailable skill location".into());
                    }
                }
                for (id, name, folder) in locations
                    .into_iter()
                    .filter(|(id, _, _)| targets.contains(id))
                {
                    let target = self.target(id.clone(), name, folder.clone());
                    let result = if target.status == "conflict" {
                        Err(target.detail)
                    } else if target.status == "installed" {
                        Ok(())
                    } else {
                        self.install(&folder)
                    };
                    results.push(json!({"id":id,"ok":result.is_ok(),"error":result.err()}));
                }
                for id in instructions {
                    let result = self.install_instructions(&id);
                    results.push(json!({"id":format!("instructions:{id}"),"ok":result.is_ok(),"error":result.err()}));
                }
                if results.iter().all(|r| r["ok"] == true) {
                    self.remember()?;
                }
                Ok(json!({"results":results}))
            }
        }
    }
    fn remember(&self) -> Result<(), String> {
        fs::create_dir_all(&self.data).map_err(|e| e.to_string())?;
        fs::write(
            self.data.join("skills-onboarding.json"),
            "{\"version\":1,\"seen\":true}",
        )
        .map_err(|e| e.to_string())
    }
    fn install(&self, folder: &Path) -> Result<(), String> {
        let destination = folder.join("SKILL.md");
        safe_path(&self.home, &destination)?;
        safe_path(&self.home, &folder.join(".branchline-skill.json"))?;
        let previous = if destination.exists() {
            let text = fs::read_to_string(&destination).map_err(|e| e.to_string())?;
            let receipt = self
                .receipt(folder, &text)?
                .ok_or("This guide is no longer an unmodified managed installation")?;
            if receipt.revision > REVISION || receipt.revision == REVISION && text != CONTENT {
                return Err("A newer or different guide is installed; kept as-is".into());
            }
            if text != CONTENT {
                self.backup(folder, &text)?;
            }
            Some(text)
        } else {
            None
        };
        fs::create_dir_all(folder).map_err(|e| e.to_string())?;
        let mut temp = tempfile::NamedTempFile::new_in(folder).map_err(|e| e.to_string())?;
        use std::io::Write;
        temp.write_all(CONTENT.as_bytes())
            .map_err(|e| e.to_string())?;
        safe_path(&self.home, &destination)?;
        if let Some(previous) = previous {
            if fs::read_to_string(&destination).map_err(|e| e.to_string())? != previous {
                return Err("The guide changed during the update; kept as-is".into());
            }
            temp.persist(&destination).map_err(|e| e.to_string())?;
        } else {
            temp.persist_noclobber(&destination)
                .map_err(|e| e.to_string())?;
        }
        self.write_receipt(folder)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn setup() -> (tempfile::TempDir, Installer) {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("home");
        fs::create_dir_all(home.join(".codex")).unwrap();
        fs::create_dir_all(home.join(".claude")).unwrap();
        let data = home.join("app-data");
        let installer = Installer::new(home, data);
        (dir, installer)
    }
    #[test]
    fn detection_does_not_install_and_dismissal_persists() {
        let (_d, i) = setup();
        assert_eq!(i.handle(Request::Status).unwrap()["shouldPrompt"], true);
        assert!(!i.home.join(".agents").exists());
        i.handle(Request::Dismiss).unwrap();
        assert_eq!(i.handle(Request::Status).unwrap()["shouldPrompt"], false);
    }
    #[test]
    fn installs_only_selected_target_and_keeps_custom_files() {
        let (_d, i) = setup();
        i.handle(Request::Finish { instructions: Vec::new(),
            targets: vec!["agents".into()],
        })
        .unwrap();
        assert_eq!(
            fs::read_to_string(i.home.join(".agents/skills").join(NAME).join("SKILL.md")).unwrap(),
            CONTENT
        );
        assert!(!i.home.join(".claude/skills").exists());
        let file = i.home.join(".agents/skills").join(NAME).join("SKILL.md");
        fs::write(&file, "custom").unwrap();
        let response = i
            .handle(Request::Finish { instructions: Vec::new(),
                targets: vec!["agents".into()],
            })
            .unwrap();
        assert_eq!(response["results"][0]["ok"], false);
        assert_eq!(fs::read_to_string(file).unwrap(), "custom");
        assert!(i
            .handle(Request::Finish { instructions: Vec::new(),
                targets: vec!["arbitrary/path".into()]
            })
            .is_err());
    }
    #[cfg(unix)]
    #[test]
    fn refuses_symlinked_destinations() {
        let (_d, i) = setup();
        std::os::unix::fs::symlink(i.home.join(".claude"), i.home.join(".agents")).unwrap();
        let r = i
            .handle(Request::Finish { instructions: Vec::new(),
                targets: vec!["agents".into()],
            })
            .unwrap();
        assert_eq!(r["results"][0]["ok"], false);
        assert!(!i.home.join(".claude/skills").exists());
    }
}

#[cfg(test)]
mod update_tests {
    use super::*;
    fn fixture() -> (tempfile::TempDir, Installer, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("home");
        fs::create_dir_all(home.join(".codex")).unwrap();
        fs::create_dir_all(home.join(".claude")).unwrap();
        let folder = home.join(".agents/skills").join(NAME);
        let data = home.join("app-data");
        (dir, Installer::new(home, data), folder)
    }
    fn old_guide(folder: &Path, revision: Option<u64>) {
        fs::create_dir_all(folder).unwrap();
        fs::write(folder.join("SKILL.md"), "Previous bundled guide").unwrap();
        let mut receipt = json!({"version":1,"sha256":hash("Previous bundled guide")});
        if let Some(revision) = revision {
            receipt["revision"] = json!(revision);
        }
        fs::write(folder.join(".branchline-skill.json"), receipt.to_string()).unwrap();
    }
    #[test]
    fn startup_updates_owned_legacy_guides_once_with_backup_and_report() {
        let (_dir, installer, folder) = fixture();
        old_guide(&folder, None);
        let report = installer.auto_update().unwrap();
        assert_eq!(report["updatedCount"], 1);
        assert_eq!(
            fs::read_to_string(folder.join("SKILL.md")).unwrap(),
            CONTENT
        );
        let backup = fs::read_dir(installer.data.join("skill-backups"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        assert_eq!(
            fs::read_to_string(backup).unwrap(),
            "Previous bundled guide"
        );
        assert_eq!(installer.auto_update().unwrap()["updatedCount"], 0);
        assert_eq!(
            fs::read_dir(installer.data.join("skill-backups"))
                .unwrap()
                .count(),
            1
        );
        let status = installer.handle(Request::Status).unwrap();
        assert_eq!(status["autoUpdate"]["updatedCount"], 0);
        assert_eq!(status["targets"][0]["status"], "installed");
        assert!(!installer.home.join(".claude/skills").exists());
    }
    #[test]
    fn previous_owned_revision_updates_without_changing_installation_scope() {
        let (_dir, installer, folder) = fixture();
        old_guide(&folder, Some(REVISION - 1));
        let report = installer.auto_update().unwrap();
        assert_eq!(report["updatedCount"], 1);
        let installed = fs::read_to_string(folder.join("SKILL.md")).unwrap();
        assert_eq!(installed, CONTENT);
        assert_eq!(installer.receipt(&folder, &installed).unwrap().unwrap().revision, REVISION);
        assert!(!installer.home.join(".codex/AGENTS.md").exists());
        assert!(!installer.home.join(".claude/skills").join(NAME).exists());
        assert_eq!(installer.auto_update().unwrap()["updatedCount"], 0);
    }

    #[test]
    fn startup_never_installs_unmanaged_customized_or_deleted_guides() {
        let (_dir, installer, folder) = fixture();
        installer.auto_update().unwrap();
        assert!(!folder.exists());
        assert_eq!(
            installer.handle(Request::Status).unwrap()["shouldPrompt"],
            true
        );
        old_guide(&folder, None);
        fs::write(folder.join("SKILL.md"), "My customization").unwrap();
        assert_eq!(installer.auto_update().unwrap()["updatedCount"], 0);
        assert_eq!(
            fs::read_to_string(folder.join("SKILL.md")).unwrap(),
            "My customization"
        );
        fs::remove_file(folder.join(".branchline-skill.json")).unwrap();
        fs::write(folder.join("SKILL.md"), "Unmanaged guide").unwrap();
        assert_eq!(installer.auto_update().unwrap()["updatedCount"], 0);
        assert_eq!(
            fs::read_to_string(folder.join("SKILL.md")).unwrap(),
            "Unmanaged guide"
        );
        fs::remove_file(folder.join("SKILL.md")).unwrap();
        installer.auto_update().unwrap();
        assert!(!folder.join("SKILL.md").exists());
    }
    #[test]
    fn newer_or_same_revision_different_guides_are_not_downgraded() {
        let (_dir, installer, folder) = fixture();
        old_guide(&folder, Some(REVISION + 1));
        assert_eq!(installer.auto_update().unwrap()["updatedCount"], 0);
        installer
            .handle(Request::Finish { instructions: Vec::new(),
                targets: vec!["agents".into()],
            })
            .unwrap();
        assert_eq!(
            fs::read_to_string(folder.join("SKILL.md")).unwrap(),
            "Previous bundled guide"
        );
        old_guide(&folder, Some(REVISION));
        assert_eq!(installer.auto_update().unwrap()["attentionCount"], 1);
        assert_eq!(
            fs::read_to_string(folder.join("SKILL.md")).unwrap(),
            "Previous bundled guide"
        );
    }
    #[cfg(unix)]
    #[test]
    fn startup_preserves_symlinks_and_retries_failed_updates() {
        let (_dir, installer, folder) = fixture();
        old_guide(&folder, None);
        std::os::unix::fs::symlink(installer.home.join(".claude"), installer.data.clone()).unwrap();
        let report = installer.auto_update().unwrap();
        assert_eq!(report["updatedCount"], 0);
        assert!(report["attentionCount"].as_u64().unwrap() > 0);
        assert_eq!(
            fs::read_to_string(folder.join("SKILL.md")).unwrap(),
            "Previous bundled guide"
        );
        fs::remove_file(&installer.data).unwrap();
        assert_eq!(installer.auto_update().unwrap()["updatedCount"], 1);
        let external = installer.home.join("external-guide");
        fs::write(&external, "Leave me alone").unwrap();
        fs::remove_file(folder.join("SKILL.md")).unwrap();
        std::os::unix::fs::symlink(&external, folder.join("SKILL.md")).unwrap();
        assert_eq!(installer.auto_update().unwrap()["updatedCount"], 0);
        assert_eq!(fs::read_to_string(external).unwrap(), "Leave me alone");
    }
}
