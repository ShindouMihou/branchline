use super::{
    hash, instruction_files as files,
    instructions::{Receipt, REVISION},
    safe_path, Installer, NAME,
};
use serde_json::{json, Value};
use std::fs;
impl Installer {
    pub(super) fn install_instructions(&self, id: &str) -> Result<(), String> {
        let location = self
            .instruction_locations()
            .into_iter()
            .find(|location| location.id == id)
            .ok_or("Unknown instruction location")?;
        let folder = self
            .locations()
            .into_iter()
            .find(|(skill_id, _, _)| skill_id == location.skill_id)
            .map(|(_, _, folder)| folder)
            .ok_or("Install this client's Branchline skill first")?;
        let text = files::read(&self.home, &folder.join("SKILL.md"))?.text;
        if text.is_empty() || self.receipt(&folder, &text)?.is_none() {
            return Err(format!(
                "Install the managed {NAME} skill for {} first.",
                location.name
            ));
        }
        self.instruction_state(&location)?;
        let document = files::read(&self.home, &location.path)?;
        let block = location.content(files::newline(&document.text));
        let span = files::span(&document.text)?;
        let (next, receipt) = if let Some(span) = span {
            let previous = self
                .instruction_receipt(&location)?
                .ok_or("Existing instructions are not managed; kept as-is")?;
            if previous.revision > REVISION
                || previous.revision == REVISION && document.text[span.clone()] != block
                || hash(&document.text[span.clone()]) != previous.block_hash
            {
                return Err("Newer or edited instructions were kept as-is.".into());
            }
            let mut next = document.text.clone();
            next.replace_range(span, &block);
            (
                next,
                Receipt {
                    version: 1,
                    revision: REVISION,
                    block_hash: hash(&block),
                    ..previous
                },
            )
        } else {
            let newline = files::newline(&document.text);
            let separator = if document.text.is_empty()
                || document.text.ends_with(&format!("{newline}{newline}"))
            {
                String::new()
            } else if document.text.ends_with(newline) {
                newline.into()
            } else {
                format!("{newline}{newline}")
            };
            (
                format!("{}{separator}{block}", document.text),
                Receipt {
                    version: 1,
                    revision: REVISION,
                    block_hash: hash(&block),
                    prefix_hash: hash(&document.text),
                    separator,
                },
            )
        };
        if document.text == next {
            return Ok(());
        }
        safe_path(&self.home, &self.instruction_receipt_path(id))?;
        self.backup_document("instruction-backups", &location.path, &document.text)?;
        files::write(&self.home, &location.path, &document, &next)?;
        if let Err(error) = self.write_json(
            &self.instruction_receipt_path(id),
            &serde_json::to_value(receipt).map_err(|error| error.to_string())?,
        ) {
            let restored = files::read(&self.home, &location.path).and_then(|current| {
                if current.text != next {
                    return Err(
                        "Instructions changed after installation; recovery was skipped.".into(),
                    );
                }
                files::write(&self.home, &location.path, &current, &document.text)
            });
            return Err(match restored {
                Ok(()) => format!("Could not save the reminder record; previous content restored: {error}"),
                Err(recovery) => format!("Could not save the reminder record: {error}. {recovery} The previous document is in instruction-backups."),
            });
        }
        Ok(())
    }
    pub(super) fn remove_instructions(&self, id: &str) -> Result<(), String> {
        let location = self
            .instruction_locations()
            .into_iter()
            .find(|location| location.id == id)
            .ok_or("Unknown instruction location")?;
        let document = files::read(&self.home, &location.path)?;
        let span = files::span(&document.text)?.ok_or("No Branchline section is installed")?;
        let receipt = self
            .instruction_receipt(&location)?
            .ok_or("This section is not managed by Branchline")?;
        if hash(&document.text[span.clone()]) != receipt.block_hash {
            return Err("The Branchline section was edited locally; it was kept.".into());
        }
        let prefix = &document.text[..span.start];
        let prefix = prefix
            .strip_suffix(&receipt.separator)
            .filter(|text| hash(text) == receipt.prefix_hash)
            .unwrap_or(prefix);
        let next = format!("{}{}", prefix, &document.text[span.end..]);
        self.backup_document("instruction-backups", &location.path, &document.text)?;
        files::write(&self.home, &location.path, &document, &next)?;
        let path = self.instruction_receipt_path(id);
        safe_path(&self.home, &path)?;
        fs::remove_file(path).map_err(|error| error.to_string())
    }
    pub(super) fn auto_update_instructions(&self) -> Vec<Value> {
        self.instruction_locations()
            .iter()
            .filter(|location| self.instruction_receipt_path(location.id).exists())
            .map(|location| {
                let result = match self.instruction_state(location) {
                    Ok(("update", _)) => self.install_instructions(location.id).map(|_| "updated"),
                    Ok((status, _)) => Ok(status),
                    Err(error) => Err(error),
                };
                match result {
                    Ok(status) => json!({"id":location.id,"status":status}),
                    Err(error) => json!({"id":location.id,"status":"customized","detail":error}),
                }
            })
            .collect()
    }
}
