use super::{hash, instruction_files as files, safe_path, Installer};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{fs, path::PathBuf};
pub(super) const REVISION: u64 = 3;
const TEMPLATE: &str = include_str!("../../../resources/branchline-integration.md");
pub(super) struct Location {
    pub(super) id: &'static str,
    pub(super) name: &'static str,
    pub(super) skill_id: &'static str,
    pub(super) path: PathBuf,
    pub(super) skill_path: &'static str,
}
#[derive(Serialize, Deserialize)]
pub(super) struct Receipt {
    pub(super) version: u64,
    pub(super) revision: u64,
    pub(super) block_hash: String,
    pub(super) prefix_hash: String,
    pub(super) separator: String,
}
impl Location {
    pub(super) fn content(&self, newline: &str) -> String {
        format!(
            "{}\n{}\n{}\n",
            files::START,
            TEMPLATE
                .replace("{{skill_path}}", self.skill_path)
                .trim_end(),
            files::END
        )
        .replace('\n', newline)
    }
}
impl Installer {
    pub(super) fn instruction_locations(&self) -> Vec<Location> {
        let mut locations = Vec::new();
        if self.home.join(".codex").is_dir() {
            locations.push(Location {
                id: "codex",
                name: "Codex",
                skill_id: "agents",
                path: self.home.join(".codex/AGENTS.md"),
                skill_path: "~/.agents/skills/branchline-git-approval/SKILL.md",
            });
        }
        if self.home.join(".claude").is_dir() {
            locations.push(Location {
                id: "claude",
                name: "Claude Code",
                skill_id: "claude",
                path: self.home.join(".claude/CLAUDE.md"),
                skill_path: "~/.claude/skills/branchline-git-approval/SKILL.md",
            });
        }
        locations
    }
    pub(super) fn instruction_receipt_path(&self, id: &str) -> PathBuf {
        self.data
            .join("instruction-receipts")
            .join(format!("{id}.json"))
    }
    pub(super) fn instruction_receipt(
        &self,
        location: &Location,
    ) -> Result<Option<Receipt>, String> {
        let path = self.instruction_receipt_path(location.id);
        safe_path(&self.home, &path)?;
        Ok(fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Receipt>(&bytes).ok())
            .filter(|receipt| receipt.version == 1))
    }
    pub(super) fn instruction_state(
        &self,
        location: &Location,
    ) -> Result<(&'static str, bool), String> {
        if location.id == "codex" && self.home.join(".codex/AGENTS.override.md").exists() {
            return Err(
                "Codex has AGENTS.override.md; its active instructions need manual integration."
                    .into(),
            );
        }
        let document = files::read(&self.home, &location.path)?;
        let Some(span) = files::span(&document.text)? else {
            return Ok(("available", false));
        };
        let receipt = self.instruction_receipt(location)?.ok_or(
            "An existing Branchline section is not managed by this installation; it was kept.",
        )?;
        if hash(&document.text[span.clone()]) != receipt.block_hash {
            return Err("The Branchline section was edited locally; it was kept.".into());
        }
        if receipt.revision > REVISION {
            return Ok(("installed", false));
        }
        let matches = document.text[span] == location.content(files::newline(&document.text));
        if !matches && receipt.revision == REVISION {
            return Err("A different reminder is installed at this revision; it was kept.".into());
        }
        Ok((if matches { "installed" } else { "update" }, true))
    }
    pub(super) fn instruction_targets(&self) -> Vec<Value> {
        self.instruction_locations().iter().map(|location| {
            let (status, removable, detail) = match self.instruction_state(location) {
                Ok(("available", _)) => ("available", false, "Add a reminder to global instructions".to_string()),
                Ok((status, removable)) => (status, removable, if status == "update" { "Reminder update available" } else { "Reminder installed" }.to_string()),
                Err(error) => ("conflict", false, error),
            };
            json!({"id":location.id,"name":location.name,"skillTargetId":location.skill_id,"path":location.path,"status":status,"removable":removable,"detail":detail,"content":location.content("\n")})
        }).collect()
    }
    pub(super) fn validate_instruction_targets(&self, ids: &[String]) -> Result<(), String> {
        if ids.iter().any(|id| {
            !self
                .instruction_locations()
                .iter()
                .any(|location| location.id == id)
        }) {
            return Err("Unknown instruction location".into());
        }
        Ok(())
    }
}
