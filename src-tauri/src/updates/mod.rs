mod state;
#[cfg(test)]
mod tests;
use serde::Deserialize;
pub use state::{Snapshot, Updates};
use std::time::{Duration, Instant};
use tauri::{Emitter, Manager};
use tauri_plugin_updater::UpdaterExt;

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    Status,
    Check { manual: bool },
    Preferences { automatic: bool },
    Install { version: String },
}
fn emit(app: &tauri::AppHandle) {
    if let Ok(snapshot) = app.state::<Updates>().snapshot() {
        let _ = app.emit("branchline-app-update", snapshot);
    }
}
pub async fn handle(
    app: tauri::AppHandle,
    request: Request,
    broker: Option<crate::ai::Shared>,
) -> Result<Snapshot, String> {
    match request {
        Request::Status => {}
        Request::Preferences { automatic } => {
            let updates = app.state::<Updates>();
            let mut inner = updates
                .inner
                .lock()
                .map_err(|_| "Update state unavailable")?;
            let parent = updates
                .path
                .parent()
                .ok_or("Update preferences path unavailable")?;
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            let bytes = serde_json::to_vec(&state::Preferences { automatic })
                .map_err(|error| error.to_string())?;
            let temporary = updates.path.with_extension("tmp");
            std::fs::write(&temporary, bytes)
                .and_then(|_| std::fs::rename(&temporary, &updates.path))
                .map_err(|error| format!("Could not save update preferences: {error}"))?;
            inner.snapshot.revision += 1;
            inner.snapshot.automatic = automatic;
            drop(inner);
            emit(&app);
        }
        Request::Check { manual } => check(&app, manual).await?,
        Request::Install { version } => install(&app, &version, broker).await?,
    }
    app.state::<Updates>().snapshot()
}
async fn check(app: &tauri::AppHandle, manual: bool) -> Result<(), String> {
    {
        let updates = app.state::<Updates>();
        let mut inner = updates
            .inner
            .lock()
            .map_err(|_| "Update state unavailable")?;
        if !state::should_check(&inner, manual) {
            return Ok(());
        }
        inner.last_attempt = Some(Instant::now());
        inner.snapshot.revision += 1;
        inner.snapshot.phase = "checking".into();
        inner.snapshot.error = None;
    }
    emit(app);
    let result = async {
        app.updater_builder().timeout(Duration::from_secs(30)).build().map_err(|error| error.to_string())?.check().await.map_err(|error| match error {
            tauri_plugin_updater::Error::ReleaseNotFound => "The update service is unavailable. Try again later.".to_string(),
            tauri_plugin_updater::Error::Reqwest(_) => "Check your internet connection and try again.".to_string(),
            _ => "Update information could not be read. Try again later.".to_string(),
        })
    }.await;
    let updates = app.state::<Updates>();
    match result {
        Ok(mut candidate) => {
            if let Some(update) = &mut candidate {
                update.timeout = Some(Duration::from_secs(300));
                if !trusted_download_url(&update.download_url) {
                    updates
                        .fail("The update download is not an official Branchline release.".into());
                    emit(app);
                    return Ok(());
                }
            }
            let mut inner = updates
                .inner
                .lock()
                .map_err(|_| "Update state unavailable")?;
            inner.snapshot.revision += 1;
            inner.snapshot.available = candidate.as_ref().map(|update| state::Release {
                version: update.version.clone(),
                notes: update.body.clone().unwrap_or_default(),
            });
            inner.snapshot.phase = if candidate.is_some() {
                "available"
            } else {
                "current"
            }
            .into();
            inner.snapshot.last_checked_at = Some(state::now());
            inner.candidate = candidate;
        }
        Err(error) => updates.fail(error),
    }
    emit(app);
    Ok(())
}
fn trusted_download_url(url: &tauri::Url) -> bool {
    url.scheme() == "https"
        && url.host_str() == Some("github.com")
        && url.port().is_none()
        && url.username().is_empty()
        && url.password().is_none()
        && url
            .path()
            .starts_with("/ShindouMihou/branchline/releases/download/")
}
async fn install(
    app: &tauri::AppHandle,
    version: &str,
    broker: Option<crate::ai::Shared>,
) -> Result<(), String> {
    let update = {
        let updates = app.state::<Updates>();
        let mut inner = updates
            .inner
            .lock()
            .map_err(|_| "Update state unavailable")?;
        if state::busy(&inner.snapshot.phase) {
            return Err("An update operation is already in progress.".into());
        }
        let candidate = inner
            .candidate
            .as_ref()
            .filter(|candidate| candidate.version == version)
            .cloned()
            .ok_or("This update is no longer available. Check for updates again.")?;
        inner.snapshot.revision += 1;
        inner.snapshot.phase = "downloading".into();
        inner.snapshot.downloaded = 0;
        inner.snapshot.total = None;
        inner.snapshot.error = None;
        candidate
    };
    emit(app);
    let mut last_progress = Instant::now();
    let downloaded = update
        .download(
            |chunk, total| {
                if let Ok(mut inner) = app.state::<Updates>().inner.lock() {
                    inner.snapshot.revision += 1;
                    inner.snapshot.downloaded =
                        inner.snapshot.downloaded.saturating_add(chunk as u64);
                    inner.snapshot.total = total;
                }
                if last_progress.elapsed() >= Duration::from_millis(300) {
                    emit(app);
                    last_progress = Instant::now();
                }
            },
            || {},
        )
        .await;
    match downloaded {
        Err(error) => app.state::<Updates>().fail(format!(
            "The update could not be downloaded or verified. Your app was not replaced. {error}"
        )),
        Ok(bytes) => {
            let gate = app.state::<Updates>().gate.clone();
            let handle = app.clone();
            let outcome = tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
                guarded_install(&gate, broker.as_ref(), || {
                    if let Ok(mut inner) = handle.state::<Updates>().inner.lock() {
                        inner.snapshot.revision += 1;
                        inner.snapshot.phase = "installing".into();
                    }
                    emit(&handle);
                    update
                        .install(bytes)
                        .map_err(|error| format!("Could not install the update: {error}"))?;
                    handle.restart();
                })
            })
            .await;
            match outcome {
                Ok(Ok(())) => {}
                Ok(Err(error)) => app.state::<Updates>().fail(error),
                Err(error) => app
                    .state::<Updates>()
                    .fail(format!("The update was interrupted: {error}")),
            }
        }
    }
    emit(app);
    Ok(())
}

fn guarded_install(
    gate: &std::sync::Mutex<()>,
    broker: Option<&crate::ai::Shared>,
    install: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    let _operation = gate.try_lock().map_err(|_| {
        "A Git operation is running. Wait for it to finish, then try installing again."
    })?;
    let guard = broker
        .map(|broker| {
            broker
                .try_lock()
                .map_err(|_| "An AI request is being handled. Try installing again shortly.")
        })
        .transpose()?;
    if guard
        .as_ref()
        .is_some_and(|broker| broker.has_active_execution())
    {
        return Err("An approved AI command is waiting or running. Wait for it to finish, then try installing again.".into());
    }
    install()
}
