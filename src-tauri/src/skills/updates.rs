use super::*;
use std::io::Write;

#[derive(Deserialize)]
pub(super) struct Receipt {
    version: u64,
    #[serde(default = "initial_revision")]
    pub(super) revision: u64,
    sha256: String,
}
fn initial_revision() -> u64 {
    1
}
impl Installer {
    pub(super) fn receipt(&self, folder: &Path, text: &str) -> Result<Option<Receipt>, String> {
        let path = folder.join(".branchline-skill.json");
        safe_path(&self.home, &path)?;
        let receipt = fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Receipt>(&bytes).ok());
        Ok(receipt.filter(|r| r.version == 1 && r.sha256 == hash(text)))
    }
    fn write_json(&self, path: &Path, value: &Value) -> Result<(), String> {
        safe_path(&self.home, path)?;
        let parent = path.parent().ok_or("Invalid update record location")?;
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
        serde_json::to_writer(&mut temp, value).map_err(|e| e.to_string())?;
        temp.as_file().sync_all().map_err(|e| e.to_string())?;
        safe_path(&self.home, path)?;
        temp.persist(path).map_err(|e| e.to_string())?;
        Ok(())
    }
    pub(super) fn write_receipt(&self, folder: &Path) -> Result<(), String> {
        self.write_json(
            &folder.join(".branchline-skill.json"),
            &json!({"version":1,"revision":REVISION,"sha256":hash(CONTENT)}),
        )
    }
    pub(super) fn backup(&self, folder: &Path, text: &str) -> Result<(), String> {
        let key = hash(&folder.to_string_lossy());
        let destination =
            self.data
                .join("skill-backups")
                .join(format!("{}-{}.md", &key[..16], hash(text)));
        safe_path(&self.home, &destination)?;
        if destination.exists() {
            return if fs::read_to_string(&destination).map_err(|e| e.to_string())? == text {
                Ok(())
            } else {
                Err("The previous guide backup could not be verified; update skipped".into())
            };
        }
        let parent = destination.parent().ok_or("Invalid backup location")?;
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
        temp.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
        temp.as_file().sync_all().map_err(|e| e.to_string())?;
        temp.persist_noclobber(destination)
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    pub(super) fn last_report(&self) -> Option<Value> {
        self.last_auto_update
            .lock()
            .ok()
            .and_then(|report| report.clone())
            .or_else(|| {
                fs::read(self.data.join("skill-updates.json"))
                    .ok()
                    .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            })
    }
    pub fn auto_update(&self) -> Result<Value, String> {
        let _guard = self
            .gate
            .lock()
            .map_err(|_| "Skill installer unavailable")?;
        let mut results = Vec::new();
        for (id, _, folder) in self.locations() {
            let path = folder.join("SKILL.md");
            let result = (|| -> Result<&str, String> {
                safe_path(&self.home, &path)?;
                if !path.exists() {
                    return Ok("not_installed");
                }
                let meta = fs::metadata(&path).map_err(|e| e.to_string())?;
                if !meta.is_file() || meta.len() > 1_000_000 {
                    return Ok("customized");
                }
                let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
                let Some(receipt) = self.receipt(&folder, &text)? else {
                    return Ok("customized");
                };
                if receipt.revision > REVISION {
                    return Ok("newer");
                }
                if text == CONTENT {
                    if receipt.revision < REVISION {
                        self.write_receipt(&folder)?;
                    }
                    return Ok("up_to_date");
                }
                if receipt.revision == REVISION {
                    return Ok("different_revision_content");
                }
                self.install(&folder)?;
                Ok("updated")
            })();
            results.push(match result {
                Ok(status) => json!({"id":id,"status":status}),
                Err(error) => json!({"id":id,"status":"failed","detail":error}),
            });
        }
        let mut report = json!({"schemaVersion":1,"revision":REVISION,"checkedAt":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),"updatedCount":results.iter().filter(|r|r["status"]=="updated").count(),"attentionCount":results.iter().filter(|r|["customized","different_revision_content","failed"].contains(&r["status"].as_str().unwrap_or(""))).count(),"results":results});
        if let Err(error) = self.write_json(&self.data.join("skill-updates.json"), &report) {
            report["recordError"] = json!(error);
        }
        *self
            .last_auto_update
            .lock()
            .map_err(|_| "Skill update report unavailable")? = Some(report.clone());
        Ok(report)
    }
}
