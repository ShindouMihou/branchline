use crate::{windows, Connector};
use std::sync::atomic::{AtomicI32, Ordering};
use tauri::{Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

pub struct Attention {
    next_notification: AtomicI32,
}
impl Default for Attention {
    fn default() -> Self {
        Self {
            next_notification: AtomicI32::new(1),
        }
    }
}

pub fn install(app: &tauri::AppHandle) -> Result<(), String> {
    let handle = app.clone();
    app.notification()
        .on_action(move |action| {
            if action.action_id() != "tap" {
                return;
            }
            let Some(id) = action
                .notification()
                .and_then(|notification| notification.extra().get("branchlinePlanId"))
                .and_then(|value| value.as_str())
                .map(str::to_owned)
            else {
                return;
            };
            let app = handle.clone();
            let _ = handle.run_on_main_thread(move || {
                if let Err(error) = show_request(&app, &id) {
                    eprintln!("Could not open the notification's request: {error}");
                }
            });
        })
        .map_err(|error| error.to_string())
}

fn show_request(app: &tauri::AppHandle, id: &str) -> Result<(), String> {
    let root = app
        .state::<Connector>()
        .0
        .as_ref()
        .map_err(Clone::clone)?
        .lock()
        .map_err(|_| "Connector unavailable")?
        .review_repository(id)
        .ok_or("This request is no longer available")?;
    let label = app.state::<windows::Windows>().route(app, Some(root), id)?;
    if let Some(window) = app.get_webview_window(&label) {
        window
            .emit("ai-review-requested", id)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub fn dispatch(app: &tauri::AppHandle, id: &str) {
    let options = app.state::<Connector>().0.as_ref().ok().and_then(|broker| {
        broker
            .lock()
            .ok()
            .and_then(|broker| broker.review_alert_options(id))
    });
    let Some((auto_open, notifications)) = options else {
        return;
    };
    if notifications {
        let notification_id = app
            .state::<Attention>()
            .next_notification
            .fetch_add(1, Ordering::Relaxed);
        if let Err(error) = app
            .notification()
            .builder()
            .id(notification_id)
            .title("AI request ready to review")
            .body("Open Branchline to approve or deny. Nothing runs automatically.")
            .extra("branchlinePlanId", id)
            .show()
        {
            eprintln!("Could not prepare the AI request notification: {error}");
        }
    }
    if auto_open {
        if let Err(error) = show_request(app, id) {
            eprintln!("Could not focus the AI request: {error}");
        }
    }
}
