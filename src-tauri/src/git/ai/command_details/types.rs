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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub push_targets: Vec<PushTarget>,
    #[serde(default)]
    pub push_targets_incomplete: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub atomic: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub head_branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pull_request_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_directory: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visibility: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_directory: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topics: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assets: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes_file: Option<String>,
    pub warnings: Vec<String>,
    pub evidence: String,
    #[serde(default)]
    pub evidence_kind: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PushTarget {
    pub source_ref: String,
    pub target_ref: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_branch: Option<String>,
    pub kind: PushTargetKind,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PushTargetKind {
    Branch,
    Tag,
    Ref,
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
            push_targets: Vec::new(),
            push_targets_incomplete: false,
            atomic: None,
            base_branch: None,
            head_branch: None,
            pull_request_title: None,
            body: None,
            working_directory: None,
            repository: None,
            visibility: None,
            source_directory: None,
            topics: None,
            tag: None,
            release_title: None,
            assets: None,
            notes_file: None,
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
