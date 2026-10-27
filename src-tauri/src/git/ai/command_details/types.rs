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
    pub operation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_untracked: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stash_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upstream_ref: Option<String>,
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
            operation: None,
            include_untracked: None,
            stash_ref: None,
            base_ref: None,
            upstream_ref: None,
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
    pub(super) fn hide_runtime_values(&mut self, marker: &str) {
        for field in [&mut self.message, &mut self.operation, &mut self.stash_ref, &mut self.base_ref, &mut self.upstream_ref, &mut self.source_branch, &mut self.target_branch, &mut self.remote, &mut self.base_branch, &mut self.head_branch, &mut self.pull_request_title, &mut self.body, &mut self.working_directory, &mut self.repository, &mut self.visibility, &mut self.source_directory, &mut self.tag, &mut self.release_title, &mut self.notes_file] {
            if field.as_ref().is_some_and(|value| value.contains(marker)) { *field = None; }
        }
        for field in [&mut self.files, &mut self.topics, &mut self.assets] {
            if field.as_ref().is_some_and(|values| values.iter().any(|value| value.contains(marker))) { *field = None; }
        }
        self.file_previews.clear();
        let before = self.push_targets.len();
        self.push_targets.retain(|target| !target.source_ref.contains(marker) && !target.target_ref.contains(marker));
        self.push_targets_incomplete |= self.push_targets.len() != before;
        for warning in &mut self.warnings { *warning = warning.replace(marker, "<runtime value>"); }
        self.evidence_kind = "unknown".into();
        self.warnings.push("Runtime expressions may execute additional commands or change argument values and boundaries. They are not evaluated by this preview.".into());
    }

    pub(super) fn unavailable(command_index: usize, warning: &str) -> Self {
        let mut detail = Self::new(command_index, "details_unavailable", "Review exact command");
        detail.warnings.push(warning.into());
        detail.evidence_kind = "unknown".into();
        detail
    }
}
