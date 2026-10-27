use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Instant, SystemTime, UNIX_EPOCH},
};
use tauri_plugin_updater::Update;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Release {
    pub version: String,
    pub notes: String,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub revision: u64,
    pub current_version: String,
    pub automatic: bool,
    pub phase: String,
    pub available: Option<Release>,
    pub downloaded: u64,
    pub total: Option<u64>,
    pub last_checked_at: Option<u64>,
    pub error: Option<String>,
}
#[derive(Deserialize, Serialize)]
pub struct Preferences {
    pub automatic: bool,
}
pub struct Inner {
    pub snapshot: Snapshot,
    pub candidate: Option<Update>,
    pub last_attempt: Option<Instant>,
}
pub struct Updates {
    pub inner: Mutex<Inner>,
    pub gate: Arc<Mutex<()>>,
    pub path: PathBuf,
}
impl Updates {
    pub fn new(path: PathBuf, gate: Arc<Mutex<()>>) -> Self {
        let automatic = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Preferences>(&bytes).ok())
            .map(|prefs| prefs.automatic)
            .unwrap_or(true);
        Self {
            path,
            gate,
            inner: Mutex::new(Inner {
                snapshot: Snapshot {
                    revision: 0,
                    current_version: env!("CARGO_PKG_VERSION").into(),
                    automatic,
                    phase: "idle".into(),
                    available: None,
                    downloaded: 0,
                    total: None,
                    last_checked_at: None,
                    error: None,
                },
                candidate: None,
                last_attempt: None,
            }),
        }
    }
    pub fn snapshot(&self) -> Result<Snapshot, String> {
        self.inner
            .lock()
            .map(|inner| inner.snapshot.clone())
            .map_err(|_| "Update state unavailable".into())
    }
    pub fn fail(&self, error: String) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.snapshot.revision += 1;
            inner.snapshot.phase = "error".into();
            inner.snapshot.error = Some(error);
        }
    }
}
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
pub fn busy(phase: &str) -> bool {
    matches!(phase, "checking" | "downloading" | "installing")
}
pub fn should_check(inner: &Inner, manual: bool) -> bool {
    !busy(&inner.snapshot.phase)
        && (manual
            || inner.snapshot.automatic
                && inner
                    .last_attempt
                    .is_none_or(|at| at.elapsed().as_secs() >= 6 * 60 * 60))
}
