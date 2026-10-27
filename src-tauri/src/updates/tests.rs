use super::{state, trusted_download_url, Updates};
use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

#[test]
fn automatic_checks_are_coalesced_and_manual_checks_can_retry() {
    let root = tempfile::tempdir().unwrap();
    let updates = Updates::new(root.path().join("updates.json"), Arc::new(Mutex::new(())));
    let mut inner = updates.inner.lock().unwrap();
    assert!(state::should_check(&inner, false));
    inner.last_attempt = Some(Instant::now());
    assert!(!state::should_check(&inner, false));
    assert!(state::should_check(&inner, true));
    inner.snapshot.phase = "downloading".into();
    assert!(!state::should_check(&inner, true));
    inner.snapshot.phase = "idle".into();
    inner.last_attempt = None;
    inner.snapshot.automatic = false;
    assert!(!state::should_check(&inner, false));
    assert!(state::should_check(&inner, true));
}

#[test]
fn automatic_preference_is_global_and_persisted() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("updates.json");
    std::fs::write(&path, r#"{"automatic":false}"#).unwrap();
    let updates = Updates::new(path, Arc::new(Mutex::new(())));
    assert!(!updates.snapshot().unwrap().automatic);
}

#[test]
fn only_official_https_release_downloads_are_accepted() {
    for url in [
        "https://github.com/ShindouMihou/branchline/releases/download/v0.4.3/Branchline.app.tar.gz",
    ] {
        assert!(trusted_download_url(&tauri::Url::parse(url).unwrap()));
    }
    for url in [
        "http://github.com/ShindouMihou/branchline/releases/download/v1/file",
        "https://github.com/other/branchline/releases/download/v1/file",
        "https://example.com/file",
        "https://user:secret@github.com/ShindouMihou/branchline/releases/download/v1/file",
    ] {
        assert!(!trusted_download_url(&tauri::Url::parse(url).unwrap()));
    }
}

#[test]
fn restart_is_refused_during_git_work_and_install_holds_the_gate() {
    let gate = Mutex::new(());
    let running = gate.lock().unwrap();
    let mut installed = false;
    let error = super::guarded_install(&gate, None, || {
        installed = true;
        Ok(())
    })
    .unwrap_err();
    assert!(error.contains("Git operation is running"));
    assert!(!installed);
    drop(running);
    super::guarded_install(&gate, None, || {
        assert!(gate.try_lock().is_err());
        installed = true;
        Ok(())
    })
    .unwrap();
    assert!(installed);
    assert!(gate.try_lock().is_ok());
}
