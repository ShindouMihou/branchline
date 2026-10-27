use crate::git;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::Manager;
struct Session {
    service: Arc<Mutex<git::Service>>,
    root: Option<String>,
    fresh: bool,
    ready: bool,
    reveal: bool,
    review: Option<String>,
}
impl Session {
    fn new(root: Option<String>, fresh: bool, reveal: bool) -> Self {
        Self {
            service: Arc::new(Mutex::new(git::Service::default())),
            root,
            fresh,
            ready: false,
            reveal,
            review: None,
        }
    }
}
pub struct Windows {
    sessions: Mutex<HashMap<String, Session>>,
    next: std::sync::atomic::AtomicU64,
}
impl Windows {
    pub fn new(reveal: bool) -> Self {
        Self {
            sessions: Mutex::new(HashMap::from([(
                "main".into(),
                Session::new(None, false, reveal),
            )])),
            next: std::sync::atomic::AtomicU64::new(1),
        }
    }
    pub fn service(&self, label: &str) -> Result<Arc<Mutex<git::Service>>, String> {
        self.sessions
            .lock()
            .map_err(|_| "Window state unavailable")?
            .get(label)
            .map(|s| s.service.clone())
            .ok_or("Unknown Branchline window".into())
    }
    pub fn opened(&self, label: &str, root: &str) {
        if let Ok(mut sessions) = self.sessions.lock() {
            if let Some(s) = sessions.get_mut(label) {
                s.root = Some(root.into());
            }
        }
    }
    pub fn context(&self, label: &str) -> Result<Value, String> {
        let sessions = self
            .sessions
            .lock()
            .map_err(|_| "Window state unavailable")?;
        let s = sessions.get(label).ok_or("Unknown Branchline window")?;
        Ok(json!({"root":s.root,"fresh":s.fresh}))
    }
    pub fn review(&self, label: &str) -> Option<String> {
        self.sessions.lock().ok()?.get(label)?.review.clone()
    }
    pub fn ready(&self, label: &str) -> Result<bool, String> {
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| "Window state unavailable")?;
        let s = sessions.get_mut(label).ok_or("Unknown Branchline window")?;
        s.ready = true;
        Ok(s.reveal)
    }
    pub fn remove(&self, label: &str) {
        if let Ok(mut sessions) = self.sessions.lock() {
            sessions.remove(label);
        }
    }
    pub fn create(&self, app: &tauri::AppHandle, root: Option<String>) -> Result<String, String> {
        let label = format!(
            "repo-{}",
            self.next.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        );
        self.sessions
            .lock()
            .map_err(|_| "Window state unavailable")?
            .insert(label.clone(), Session::new(root, true, true));
        if let Err(error) = build(app, &label) {
            self.remove(&label);
            return Err(error);
        }
        Ok(label)
    }
    fn matching(&self, root: &Option<String>) -> Option<String> {
        let sessions = self.sessions.lock().ok()?;
        sessions
            .iter()
            .find(|(_, s)| root.is_some() && s.root == *root)
            .map(|(label, _)| label.clone())
    }
    pub fn route(
        &self,
        app: &tauri::AppHandle,
        root: Option<String>,
        plan: &str,
    ) -> Result<String, String> {
        let matching = self.matching(&root);
        let label = if let Some(label) = matching {
            label
        } else {
            self.create(app, root)?
        };
        let ready = {
            let mut sessions = self
                .sessions
                .lock()
                .map_err(|_| "Window state unavailable")?;
            let s = sessions.get_mut(&label).ok_or("Window closed")?;
            s.review = Some(plan.into());
            s.reveal = true;
            s.ready
        };
        if ready {
            reveal(app, &label)?;
        }
        Ok(label)
    }
    pub fn reopen(&self, app: &tauri::AppHandle) {
        let target = {
            let mut sessions = match self.sessions.lock() {
                Ok(s) => s,
                Err(_) => return,
            };
            let label = sessions
                .keys()
                .find(|label| {
                    app.get_webview_window(label)
                        .is_some_and(|w| w.is_focused().unwrap_or(false))
                })
                .cloned()
                .or_else(|| sessions.keys().next().cloned());
            label.map(|label| {
                let s = sessions.get_mut(&label).unwrap();
                s.reveal = true;
                (label, s.ready)
            })
        };
        if let Some((label, true)) = target {
            if let Err(error) = reveal(app, &label) {
                eprintln!("Could not reopen the repository window: {error}");
            }
        } else if target.is_none() {
            let _ = self.create(app, None);
        }
    }
}
pub fn reveal(app: &tauri::AppHandle, label: &str) -> Result<(), String> {
    let window = app.get_webview_window(label).ok_or("Window closed")?;
    #[cfg(target_os = "macos")]
    app.show().map_err(|error| error.to_string())?;
    window.unminimize().map_err(|error| error.to_string())?;
    window.show().map_err(|error| error.to_string())?;
    #[cfg(target_os = "macos")]
    if let Some(main_thread) = objc2::MainThreadMarker::new() {
        #[allow(deprecated)]
        objc2_app_kit::NSApplication::sharedApplication(main_thread).activateIgnoringOtherApps(true);
    }
    window.set_focus().map_err(|error| error.to_string())
}
fn build(app: &tauri::AppHandle, label: &str) -> Result<(), String> {
    tauri::WebviewWindowBuilder::new(app, label, tauri::WebviewUrl::App("index.html".into()))
        .visible(false)
        .title("Branchline")
        .inner_size(1280., 820.)
        .min_inner_size(850., 600.)
        .build()
        .map(|_| ())
        .map_err(|e| e.to_string())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn opening_one_repository_does_not_change_another_window() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        for root in [&a, &b] {
            std::fs::create_dir(root).unwrap();
            assert!(std::process::Command::new("git")
                .args(["init", "-q"])
                .arg(root)
                .status()
                .unwrap()
                .success());
        }
        let state = Windows::new(true);
        state
            .sessions
            .lock()
            .unwrap()
            .insert("repo-1".into(), Session::new(None, true, true));
        let first = state.service("main").unwrap();
        let second = state.service("repo-1").unwrap();
        first
            .lock()
            .unwrap()
            .handle(git::Request::OpenQuick {
                path: a.to_string_lossy().into(),
            })
            .unwrap();
        second
            .lock()
            .unwrap()
            .handle(git::Request::OpenQuick {
                path: b.to_string_lossy().into(),
            })
            .unwrap();
        assert_eq!(
            first.lock().unwrap().handle(git::Request::Status).unwrap()["root"],
            std::fs::canonicalize(&a)
                .unwrap()
                .to_string_lossy()
                .as_ref()
        );
        assert_eq!(
            second.lock().unwrap().handle(git::Request::Status).unwrap()["root"],
            std::fs::canonicalize(&b)
                .unwrap()
                .to_string_lossy()
                .as_ref()
        );
    }
    #[test]
    fn windows_have_independent_services_and_repository_state() {
        let state = Windows::new(true);
        state.sessions.lock().unwrap().insert(
            "repo-1".into(),
            Session::new(Some("/repo/b".into()), true, true),
        );
        state.opened("main", "/repo/a");
        assert_eq!(state.context("main").unwrap()["root"], "/repo/a");
        assert_eq!(state.context("repo-1").unwrap()["root"], "/repo/b");
        assert_eq!(
            state.matching(&Some("/repo/b".into())).as_deref(),
            Some("repo-1")
        );
        assert_eq!(state.matching(&Some("/repo/c".into())), None);
        assert_eq!(state.matching(&None), None);
        assert!(!Arc::ptr_eq(
            &state.service("main").unwrap(),
            &state.service("repo-1").unwrap()
        ));
        state.remove("repo-1");
        assert!(state.service("repo-1").is_err());
        assert!(state.service("main").is_ok());
    }
}
