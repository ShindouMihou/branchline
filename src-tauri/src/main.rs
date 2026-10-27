#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod ai;
mod git;
mod skills;
mod windows;
mod review_attention;
mod updates;
use std::sync::Arc;
use tauri::{Emitter, Manager};
#[tauri::command]
fn frontend_ready(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, windows::Windows>,
) -> Result<(), String> {
    if state.ready(window.label())? {
        let handle = window.app_handle().clone();
        let label = window.label().to_string();
        let app = handle.clone();
        handle.run_on_main_thread(move || {
            if let Err(error) = windows::reveal(&app, &label) {
                eprintln!("Could not reveal the ready window: {error}");
            }
        }).map_err(|error| error.to_string())?;
    }
    Ok(())
}
#[tauri::command]
fn window_context(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, windows::Windows>,
) -> Result<serde_json::Value, String> {
    state.context(window.label())
}
#[tauri::command]
fn new_window(
    app: tauri::AppHandle,
    state: tauri::State<'_, windows::Windows>,
) -> Result<String, String> {
    state.create(&app, None)
}
#[tauri::command]
async fn skill_setup(
    request: skills::Request,
    window: tauri::WebviewWindow,
    state: tauri::State<'_, Arc<skills::Installer>>,
) -> Result<serde_json::Value, String> {
    window
        .app_handle()
        .state::<windows::Windows>()
        .service(window.label())?;
    let installer = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || installer.handle(request))
        .await
        .map_err(|e| e.to_string())?
}
struct Connector(Result<ai::Shared, String>);
#[tauri::command]
async fn app_update(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    request: updates::Request,
    connector: tauri::State<'_, Connector>,
) -> Result<updates::Snapshot, String> {
    app.state::<windows::Windows>().service(window.label())?;
    updates::handle(app, request, connector.0.as_ref().ok().cloned()).await
}

#[tauri::command]
async fn request(
    request: git::Request,
    state: tauri::State<'_, windows::Windows>,
    window: tauri::WebviewWindow,
) -> Result<serde_json::Value, String> {
    let service = state.service(window.label())?;
    let handle = window.app_handle().clone();
    let label = window.label().to_string();
    let gate = handle.state::<updates::Updates>().gate.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _queue = gate.lock().map_err(|_| "Execution queue unavailable")?;
        let result = service
            .lock()
            .map_err(|_| "Repository service unavailable".to_string())?
            .handle(request)?;
        if let Some(root) = result.get("root").and_then(|r| r.as_str()) {
            handle.state::<windows::Windows>().opened(&label, root);
            if let Some(window) = handle.get_webview_window(&label) {
                let name = std::path::Path::new(root)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy();
                let _ = window.set_title(&format!("{name} — Branchline"));
            }
        }
        Ok(result)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn ai_request(
    request: ai::Native,
    state: tauri::State<'_, Connector>,
    window: tauri::WebviewWindow,
) -> Result<serde_json::Value, String> {
    window
        .app_handle()
        .state::<windows::Windows>()
        .service(window.label())?;
    let focus = window
        .app_handle()
        .state::<windows::Windows>()
        .review(window.label());
    let broker = state.0.as_ref().map_err(Clone::clone)?.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut result = ai::Broker::native(&broker, request)?;
        if let Some(object) = result.as_object_mut() {
            if object.contains_key("focusPlanId") {
                object.insert("focusPlanId".into(), serde_json::json!(focus));
            }
        }
        Ok(result)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
fn open_setup_docs(provider: String) -> Result<(), String> {
    let url = match provider.as_str() {
        "claude-code" => "https://code.claude.com/docs/en/mcp",
        "chatgpt-desktop" => "https://learn.chatgpt.com/docs/extend/mcp",
        "codex" => "https://learn.chatgpt.com/docs/extend/mcp?surface=cli",
        "cursor" => "https://cursor.com/docs/mcp",
        _ => "https://modelcontextprotocol.io/docs/develop/connect-local-servers",
    };
    let status = std::process::Command::new("/usr/bin/open")
        .arg(url)
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err("Could not open setup documentation".into())
    }
}
fn markdown_link_url(url: &str) -> Result<tauri::Url, String> {
    let parsed = tauri::Url::parse(url).map_err(|_| "Invalid web link")?;
    if !matches!(parsed.scheme(), "https" | "http")
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return Err("Only HTTP and HTTPS web links are supported".into());
    }
    Ok(parsed)
}
#[tauri::command]
fn open_markdown_link(url: String) -> Result<(), String> {
    let parsed = markdown_link_url(&url)?;
    let status = std::process::Command::new("/usr/bin/open")
        .arg(parsed.as_str())
        .status()
        .map_err(|error| error.to_string())?;
    if status.success() { Ok(()) } else { Err("Could not open web link".into()) }
}
#[tauri::command]
fn open_github_pull_request(url: String) -> Result<(), String> {
    let path = url
        .strip_prefix("https://github.com/")
        .ok_or("Invalid GitHub pull request URL")?;
    let parts: Vec<_> = path.split('/').collect();
    if parts.len() != 4
        || parts[2] != "pull"
        || parts[3].is_empty()
        || !parts[3].bytes().all(|b| b.is_ascii_digit())
        || parts[..2].iter().any(|p| {
            p.is_empty()
                || *p == "."
                || *p == ".."
                || !p
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        })
    {
        return Err("Invalid GitHub pull request URL".into());
    }
    if !std::process::Command::new("/usr/bin/open")
        .arg(url)
        .status()
        .map_err(|e| e.to_string())?
        .success()
    {
        return Err("Could not open pull request".into());
    }
    Ok(())
}
#[cfg(target_os = "macos")]
fn set_dock_icon() {
    use objc2::{AllocAnyThread, MainThreadMarker};
    use objc2_app_kit::{NSApplication, NSImage};
    use objc2_foundation::NSData;
    let Some(main_thread) = MainThreadMarker::new() else {
        return;
    };
    let data = NSData::with_bytes(include_bytes!("../icons/icon.png"));
    if let Some(icon) = NSImage::initWithData(NSImage::alloc(), &data) {
        // AppKit retains the image. This call runs only on the native main thread.
        unsafe {
            NSApplication::sharedApplication(main_thread).setApplicationIconImage(Some(&icon));
        }
    }
}
fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    if args.get(1).map(String::as_str) == Some("--mcp") {
        let result = if args.get(2).map(String::as_str) == Some("--credential-file") {
            args.get(3)
                .ok_or("Missing credential file".to_string())
                .and_then(|p| ai::mcp::run(std::path::Path::new(p)))
        } else {
            Err("Use --mcp --credential-file <private file>".into())
        };
        if let Err(e) = result {
            eprintln!("Branchline MCP: {e}");
            std::process::exit(1)
        }
        return;
    }
    // Configure visibility before Tauri creates windows, avoiding a startup flash.
    let mut context = tauri::generate_context!();
    let background = args.iter().any(|arg| arg == "--mcp-background");
    for window in &mut context.config_mut().app.windows {
        window.visible = false;
        window.focus = false;
    }
    tauri::Builder::default()
        .manage(windows::Windows::new(!background))
        .manage(review_attention::Attention::default())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let menu = tauri::menu::Menu::default(app.handle())?;
            let item = tauri::menu::MenuItem::with_id(
                app,
                "new-window",
                "New Window",
                true,
                Some("CmdOrCtrl+N"),
            )?;
            let update_item = tauri::menu::MenuItem::with_id(app, "check-app-updates", "Check for Updates…", true, None::<&str>)?;
            for entry in menu.items()? {
                if let tauri::menu::MenuItemKind::Submenu(submenu) = entry {
                    if submenu.text()?.replace('&', "") == "Branchline" {
                        submenu.insert(&update_item, 1)?;
                    }
                    if submenu.text()?.replace('&', "") == "File" {
                        submenu.prepend(&item)?;
                    }
                }
            }
            app.set_menu(menu)?;
            let broker = app
                .path()
                .app_local_data_dir()
                .map_err(|e| e.to_string())
                .and_then(|p| ai::Broker::start(p.join("ai")));
            if let Ok(shared) = &broker {
                let handle = app.handle().clone();
                if let Ok(mut broker) = shared.lock() {
                    broker.set_notifier(Arc::new(move |id| {
                        let app = handle.clone();
                        let plan_id = id.to_string();
                        let _ = handle.run_on_main_thread(move || {
                            review_attention::dispatch(&app, &plan_id);
                        });
                    }));
                }
            }
            let installer = Arc::new(skills::Installer::new(
                app.path().home_dir()?,
                app.path().app_local_data_dir()?,
            ));
            app.manage(installer.clone());
            tauri::async_runtime::spawn_blocking(move || {
                let _ = installer.auto_update();
            });
            let gate = broker.as_ref().ok().and_then(|broker| broker.lock().ok().map(|broker| broker.operation_gate.clone())).unwrap_or_default();
            app.manage(updates::Updates::new(app.path().app_local_data_dir()?.join("updates.json"), gate));
            app.manage(Connector(broker));
            if let Err(error) = review_attention::install(app.handle()) {
                eprintln!("Could not initialize notification clicks: {error}");
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            request,
            ai_request,
            frontend_ready,
            window_context,
            new_window,
            skill_setup,
            open_setup_docs,
            open_github_pull_request,
            open_markdown_link,
            app_update
        ])
        .on_menu_event(|app, event| {
            if event.id().as_ref() == "check-app-updates" {
                let windows = app.webview_windows();
                if let Some(window) = windows.values().find(|window| window.is_focused().unwrap_or(false)).or_else(|| windows.values().next()) {
                    let _ = window.unminimize();
                    let _ = window.show();
                    let _ = window.set_focus();
                    let _ = window.emit("check-for-app-updates", ());
                }
            }
            if event.id().as_ref() == "new-window" {
                let _ = app.state::<windows::Windows>().create(app, None);
            }
        })
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::Destroyed) {
                window.state::<windows::Windows>().remove(window.label());
            }
        })
        .build(context)
        .expect("Unable to start Branchline")
        .run(|app, event| {
            #[cfg(target_os = "macos")]
            if matches!(&event, tauri::RunEvent::Ready) {
                set_dock_icon();
            }
            // A deliberate Dock/Finder open should reveal a background instance.
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = event {
                app.state::<windows::Windows>().reopen(app);
            }
        });
}

#[cfg(test)]
mod markdown_link_tests {
    #[test]
    fn allows_only_web_links_without_credentials() {
        for url in ["https://github.com/owner/repo", "http://localhost:3000/docs"] {
            assert!(super::markdown_link_url(url).is_ok());
        }
        for url in ["javascript:alert(1)", "file:///etc/passwd", "data:text/html,test", "/relative", "https://user:password@example.com"] {
            assert!(super::markdown_link_url(url).is_err(), "{url}");
        }
    }
}
