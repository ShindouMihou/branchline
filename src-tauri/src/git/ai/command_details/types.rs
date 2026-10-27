use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Detail {
    pub command_index: usize,
    pub kind: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<String>>,
    #[serde(default)]
    pub file_previews: Vec<FileSelection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub head_branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pull_request_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    pub warnings: Vec<String>,
    pub evidence: String,
    #[serde(default)]
    pub evidence_kind: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FileSelection {
    pub path: String,
    pub source: FileSource,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FileSource {
    Index,
    Worktree,
}

impl Detail {
    pub(super) fn new(command_index: usize, kind: &str, title: &str) -> Self {
        Self {
            command_index,
            kind: kind.into(),
            title: title.into(),
            message: None,
            files: None,
            file_previews: Vec::new(),
            source_branch: None,
            target_branch: None,
            remote: None,
            base_branch: None,
            head_branch: None,
            pull_request_title: None,
            body: None,
            warnings: Vec::new(),
            evidence: "Literal command arguments; effects are not verified.".into(),
            evidence_kind: "command".into(),
        }
    }
    pub(super) fn unavailable(command_index: usize, warning: &str) -> Self {
        let mut detail = Self::new(command_index, "details_unavailable", "Review exact command");
        detail.warnings.push(warning.into());
        detail.evidence_kind = "unknown".into();
        detail
    }
}
