#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use base64::Engine;
mod network;
mod panel;
mod update;
use drift_core::{Config, Target};
use serde::Serialize;
use std::collections::BTreeMap;
use std::{path::PathBuf, sync::Mutex};
use tauri::{
    Manager, State,
    menu::{AboutMetadata, Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};
use uuid::Uuid;
struct Store {
    path: PathBuf,
    config: Mutex<Config>,
    runtime: network::Runtime,
    operations: Mutex<()>,
}
#[derive(Serialize)]
struct Snapshot {
    config: Config,
    available: bool,
    state: &'static str,
    reason: String,
    icons: BTreeMap<String, String>,
}
fn snapshot(config: Config, runtime: &network::Runtime) -> Snapshot {
    let (state, reason) = runtime.state();
    Snapshot {
        config,
        icons: BTreeMap::new(),
        available: true,
        state,
        reason,
    }
}
fn with_icons(
    config: Config,
    config_path: &std::path::Path,
    runtime: &network::Runtime,
) -> Snapshot {
    let directory = config_path.parent().unwrap().join("icons");
    let mut result = snapshot(config, runtime);
    for rule in &result.config.rules {
        if let Ok(bytes) = std::fs::read(directory.join(format!("{}.png", rule.id)))
            && bytes.len() <= 256 * 1024
            && bytes.starts_with(b"\x89PNG\r\n\x1a\n")
        {
            result.icons.insert(
                rule.id.to_string(),
                format!(
                    "data:image/png;base64,{}",
                    base64::engine::general_purpose::STANDARD.encode(bytes)
                ),
            );
        }
    }
    result
}
fn extract_app_icon(app_path: &std::path::Path, output: &std::path::Path) -> Result<(), String> {
    std::fs::create_dir_all(output.parent().ok_or("Icon directory missing")?)
        .map_err(|e| e.to_string())?;
    #[cfg(target_os = "macos")]
    {
        let info = plist::Value::from_file(app_path.join("Contents/Info.plist"))
            .map_err(|e| e.to_string())?;
        let name = info
            .as_dictionary()
            .and_then(|d| d.get("CFBundleIconFile"))
            .and_then(|v| v.as_string())
            .ok_or("No app icon")?;
        let root = app_path
            .join("Contents/Resources")
            .canonicalize()
            .map_err(|e| e.to_string())?;
        let mut source = root.join(name);
        if source.extension().is_none() {
            source.set_extension("icns");
        }
        let source = source.canonicalize().map_err(|e| e.to_string())?;
        if !source.starts_with(&root) {
            return Err("Invalid icon path".into());
        }
        let status = std::process::Command::new("/usr/bin/sips")
            .args(["-s", "format", "png", "-z", "64", "64"])
            .arg(source)
            .arg("--out")
            .arg(output)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map_err(|e| e.to_string())?;
        if !status.success() {
            return Err("Icon conversion failed".into());
        }
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let quote = |p: &std::path::Path| format!("'{}'", p.to_string_lossy().replace('\'', "''"));
        let script = format!(
            "Add-Type -AssemblyName System.Drawing; $i=[System.Drawing.Icon]::ExtractAssociatedIcon({}); if($null -eq $i){{exit 1}}; $b=$i.ToBitmap(); try{{$b.Save({},[System.Drawing.Imaging.ImageFormat]::Png)}}finally{{$b.Dispose();$i.Dispose()}}",
            quote(app_path),
            quote(output)
        );
        let bytes: Vec<u8> = script
            .encode_utf16()
            .flat_map(|c| c.to_le_bytes())
            .collect();
        let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
        let status = std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-EncodedCommand", &encoded])
            .creation_flags(0x08000000)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map_err(|e| e.to_string())?;
        if !status.success() {
            return Err("Icon extraction failed".into());
        }
    }
    #[cfg(target_os = "linux")]
    {
        let entry = drift_network::apps::linux_desktop_entry(app_path)
            .map_err(|_| "This app does not provide a desktop icon.")?;
        let source = entry
            .icon
            .as_deref()
            .and_then(drift_network::apps::linux_icon_path)
            .ok_or("This app does not provide a desktop icon.")?;
        let bytes = std::fs::read(source).map_err(|e| e.to_string())?;
        if bytes.len() > 256 * 1024 || !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            return Err("This app does not provide a PNG desktop icon.".into());
        }
        std::fs::write(output, bytes).map_err(|e| e.to_string())?;
    }
    Ok(())
}
#[tauri::command]
fn read_state(store: State<'_, Store>) -> Result<Snapshot, String> {
    let config = store
        .config
        .lock()
        .map_err(|_| "Could not open the configuration.")?
        .clone();
    Ok(with_icons(config, &store.path, &store.runtime))
}

async fn mutate(
    app: tauri::AppHandle,
    f: impl FnOnce(&mut Config) -> Result<(), drift_core::Error> + Send + 'static,
) -> Result<Snapshot, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let store = app.state::<Store>();
        let _operation = store
            .operations
            .lock()
            .map_err(|_| "Configuration is busy")?;
        let previous = store
            .config
            .lock()
            .map_err(|_| "Could not open the configuration")?
            .clone();
        let mut candidate = previous.clone();
        f(&mut candidate).map_err(|e| e.to_string())?;
        let was_active = store.runtime.active();
        if was_active && let Err(error) = store.runtime.set(true, &candidate) {
            let _ = store.runtime.set(true, &previous);
            return Err(error);
        }
        if let Err(error) = candidate.save(&store.path) {
            if was_active {
                let _ = store.runtime.set(true, &previous);
            }
            return Err(error.to_string());
        }
        *store.config.lock().unwrap() = candidate.clone();
        Ok(with_icons(candidate, &store.path, &store.runtime))
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn add_site(input: String, app: tauri::AppHandle) -> Result<Snapshot, String> {
    mutate(app, move |c| c.add_site(&input)).await
}
#[tauri::command]
async fn change_rule(id: Uuid, enabled: bool, app: tauri::AppHandle) -> Result<Snapshot, String> {
    mutate(app, move |c| c.set_enabled(id, enabled)).await
}
#[tauri::command]
async fn remove_rule(id: Uuid, app: tauri::AppHandle) -> Result<Snapshot, String> {
    let icon_path = app
        .state::<Store>()
        .path
        .parent()
        .unwrap()
        .join("icons")
        .join(format!("{id}.png"));
    let result = mutate(app, move |c| c.remove(id)).await?;
    let _ = std::fs::remove_file(icon_path);
    Ok(result)
}
#[tauri::command]
async fn set_access(enabled: bool, app: tauri::AppHandle) -> Result<Snapshot, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let store = app.state::<Store>();
        let _operation = store
            .operations
            .lock()
            .map_err(|_| "Configuration is busy")?;
        let config = store
            .config
            .lock()
            .map_err(|_| "Could not open the configuration")?
            .clone();
        store.runtime.set(enabled, &config)?;
        Ok(with_icons(config, &store.path, &store.runtime))
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn install_update(app: tauri::AppHandle) -> Result<(), String> {
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let store = handle.state::<Store>();
        let _operation = store
            .operations
            .lock()
            .map_err(|_| "Configuration is busy")?;
        let config = store
            .config
            .lock()
            .map_err(|_| "Could not open the configuration")?
            .clone();
        let was_active = store.runtime.active();
        store.runtime.set(false, &config)?;
        if let Err(error) = update::launch_ready(&handle) {
            if was_active {
                let _ = store.runtime.set(true, &config);
            }
            return Err(error);
        }
        Ok::<(), String>(())
    })
    .await
    .map_err(|error| error.to_string())??;
    app.exit(0);
    Ok(())
}
#[tauri::command]
async fn pick_app(app: tauri::AppHandle) -> Result<Option<Snapshot>, String> {
    struct PickerGuard(tauri::AppHandle);
    impl Drop for PickerGuard {
        fn drop(&mut self) {
            self.0
                .state::<panel::PanelState>()
                .picking
                .store(false, std::sync::atomic::Ordering::Relaxed);
            panel::show(&self.0);
        }
    }
    app.state::<panel::PanelState>()
        .picking
        .store(true, std::sync::atomic::Ordering::Relaxed);
    let _guard = PickerGuard(app.clone());
    let selected = tauri::async_runtime::spawn_blocking(|| {
        let dialog = rfd::FileDialog::new().set_title("Add an app to Driftnet");
        #[cfg(target_os = "macos")]
        let dialog = dialog
            .set_directory("/Applications")
            .add_filter("Application", &["app"]);
        #[cfg(target_os = "windows")]
        let dialog = dialog.add_filter("Application", &["exe"]);
        #[cfg(target_os = "linux")]
        let dialog = dialog.set_directory("/usr/share/applications");
        dialog.pick_file()
    })
    .await
    .map_err(|e| e.to_string())?;
    let Some(path) = selected else {
        return Ok(None);
    };
    let path = path.canonicalize().map_err(|e| e.to_string())?;
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or("Could not read the app name.")?
        .to_string();
    #[cfg(target_os = "linux")]
    let name = if path
        .extension()
        .is_some_and(|extension| extension == "desktop")
    {
        drift_network::apps::linux_desktop_entry(&path)
            .map_err(|e| e.to_string())?
            .name
    } else {
        name
    };
    #[cfg(target_os = "macos")]
    let identity = {
        if path.extension().and_then(|x| x.to_str()) != Some("app") {
            return Err("Select a .app application.".into());
        }
        let info = plist::Value::from_file(path.join("Contents/Info.plist"))
            .map_err(|_| "Could not read the app information.")?;
        let id = info
            .as_dictionary()
            .and_then(|d| d.get("CFBundleIdentifier"))
            .and_then(|v| v.as_string())
            .ok_or("Could not find the app identity.")?;
        format!("macos:{id}")
    };
    #[cfg(target_os = "windows")]
    let identity = {
        if path
            .extension()
            .and_then(|x| x.to_str())
            .is_none_or(|x| !x.eq_ignore_ascii_case("exe"))
        {
            return Err("Select a .exe application.".into());
        }
        format!("windows:{}", path.to_string_lossy().to_lowercase())
    };
    #[cfg(target_os = "linux")]
    let identity = {
        let executable = drift_network::apps::resolve(&format!("linux:{}", path.to_string_lossy()))
            .map_err(|error| error.to_string())?;
        format!("linux:{}", executable.to_string_lossy())
    };
    let store = app.state::<Store>();
    // Discord already exists as a preset; don't add a duplicate custom app rule.
    if identity == "macos:com.hnc.Discord"
        && store
            .config
            .lock()
            .map_err(|_| "Could not open the configuration.")?
            .rules
            .iter()
            .any(|r| matches!(r.target, Target::Discord))
    {
        return Err("Discord is already in your list.".into());
    }
    let added_identity = identity.clone();
    let result = mutate(app.clone(), move |c| c.add_app(added_identity, name)).await?;
    if let Some(rule) = result
        .config
        .rules
        .iter()
        .find(|r| matches!(&r.target,Target::App{identity:existing,..} if existing==&identity))
    {
        let store = app.state::<Store>();
        let output = store
            .path
            .parent()
            .unwrap()
            .join("icons")
            .join(format!("{}.png", rule.id));
        let _ = extract_app_icon(&path, &output);
    }
    read_state(app.state::<Store>()).map(Some)
}
#[tauri::command]
fn set_panel_view(settings: bool, window: tauri::WebviewWindow) -> Result<(), String> {
    window
        .app_handle()
        .state::<panel::PanelState>()
        .settings
        .store(settings, std::sync::atomic::Ordering::Relaxed);
    panel::position(&window)
}
#[tauri::command]
fn hide_panel(window: tauri::WebviewWindow) {
    panel::hide(&window);
}
#[tauri::command]
fn panel_ready(app: tauri::AppHandle) {
    panel::ready(&app);
}
fn open_uninstaller(app: &tauri::AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let source = app
            .path()
            .resource_dir()
            .map_err(|e| e.to_string())?
            .join("uninstall/Drift-Uninstall.pkg");
        let destination =
            std::env::temp_dir().join(format!("Drift-Uninstall-{}.pkg", Uuid::new_v4()));
        std::fs::copy(source, &destination).map_err(|e| e.to_string())?;
        let status = std::process::Command::new("/usr/bin/open")
            .arg(destination)
            .status()
            .map_err(|e| e.to_string())?;
        if !status.success() {
            return Err("macOS could not open the uninstaller".into());
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let path = std::env::current_exe()
            .map_err(|e| e.to_string())?
            .parent()
            .ok_or("Missing install folder")?
            .join("uninstall.exe");
        let path: Vec<_> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let verb: Vec<_> = "open".encode_utf16().chain(Some(0)).collect();
        // SAFETY: both arguments are live NUL-terminated UTF-16 strings. The
        // signed/packaged uninstaller owns its normal Windows UAC consent.
        let result = unsafe {
            windows_sys::Win32::UI::Shell::ShellExecuteW(
                std::ptr::null_mut(),
                verb.as_ptr(),
                path.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                1,
            )
        };
        if result as isize <= 32 {
            return Err("Windows could not open the uninstaller".into());
        }
        let _ = app;
    }
    #[cfg(target_os = "linux")]
    {
        // The system's polkit dialog owns administrator consent. No password
        // is collected by Driftnet and no shell command is constructed.
        let child = std::process::Command::new("/usr/bin/pkexec")
            .args(["/usr/bin/dpkg", "--remove", "driftnet"])
            .spawn()
            .map_err(|error| format!("Could not open the package uninstaller: {error}"))?;
        drop(child);
        let _ = app;
    }
    Ok(())
}
fn main() {
    // Packaged-engine smoke check for installers/CI. No GUI or OS routing is
    // started, and the loopback listener is closed before this process exits.
    if std::env::args_os()
        .nth(1)
        .is_some_and(|arg| arg == "--check-engine")
    {
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            let executable = std::env::current_exe()?;
            let engine =
                executable
                    .parent()
                    .ok_or("Missing bundle directory")?
                    .join(if cfg!(windows) {
                        "drift-engine.exe"
                    } else {
                        "drift-engine"
                    });
            let mut engine = drift_engine::Engine::start(&engine)?;
            engine.stop()?;
            Ok(())
        })();
        match result {
            Ok(()) => println!("Bundled engine ready; listener closed; system network unchanged."),
            Err(error) => {
                eprintln!("Engine check failed: {error}");
                std::process::exit(1);
            }
        }
        return;
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            panel::show(app)
        }))
        .setup(|app| {
            app.manage(panel::PanelState::default());
            let path = app.path().app_config_dir()?.join("rules.json");
            let config = Config::load(&path)?;
            #[cfg(target_os = "macos")]
            {
                if let Some(rule) = config
                    .rules
                    .iter()
                    .find(|r| matches!(r.target, Target::Discord))
                {
                    let output = path
                        .parent()
                        .unwrap()
                        .join("icons")
                        .join(format!("{}.png", rule.id));
                    if !output.exists() {
                        let _ = extract_app_icon(
                            std::path::Path::new("/Applications/Discord.app"),
                            &output,
                        );
                    }
                }
            }
            app.manage(Store {
                path,
                config: Mutex::new(config),
                runtime: network::Runtime::initialize(),
                operations: Mutex::new(()),
            });
            app.manage(update::Updates::default());
            update::start_background(app.handle().clone());
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            let about = PredefinedMenuItem::about(app, Some("About Driftnet"), Some(AboutMetadata {
                name: Some("Driftnet".into()),
                version: Some(env!("CARGO_PKG_VERSION").into()),
                copyright: Some("© 2026 Driftnet contributors".into()),
                comments: Some("Local routing for selected apps and websites. Built with Tauri, Rust, sing-box and ByeDPI.".into()),
                license: Some("Driftnet UI/control: MIT. Router: GPL-3.0-or-later. Engine: MIT. Font: OFL-1.1. Full notices accompany the installer.".into()),
                website: Some("https://github.com/ernilmz/driftnet".into()),
                website_label: Some("Source and documentation".into()),
                credits: Some("Local routing for selected apps and websites.\n\nBuilt with Tauri, Rust, sing-box and ByeDPI.\nDriftnet UI/control: MIT · Router: GPL-3.0-or-later\nEngine: MIT · IBM Plex Mono: OFL-1.1\n\nSource and documentation:\nhttps://github.com/ernilmz/driftnet\n\nFull notices accompany the installer.\nNot affiliated with Discord or upstream projects.".into()),
                icon: app.default_window_icon().cloned(),
                ..Default::default()
            }))?;
            let quit = MenuItem::with_id(app, "quit", "Quit Driftnet", true, None::<&str>)?;
            let uninstall =
                MenuItem::with_id(app, "uninstall", "Uninstall Driftnet…", true, None::<&str>)?;
            let update_menu =
                MenuItem::with_id(app, "updates", "Check for updates", true, None::<&str>)?;
            #[cfg(target_os = "linux")]
            let open = MenuItem::with_id(app, "open", "Open Driftnet", true, None::<&str>)?;
            #[cfg(target_os = "linux")]
            let menu = Menu::with_items(app, &[&open, &about, &update_menu, &uninstall, &quit])?;
            #[cfg(not(target_os = "linux"))]
            let menu = Menu::with_items(app, &[&about, &update_menu, &uninstall, &quit])?;
            let mut tray = TrayIconBuilder::with_id("drift-tray")
                .tooltip("Driftnet")
                .menu(&menu)
                .show_menu_on_left_click(cfg!(target_os = "linux"))
                .on_menu_event(move |app, event| {
                    #[cfg(target_os = "linux")]
                    if event.id.as_ref() == "open" {
                        panel::show(app);
                    }
                    if event.id.as_ref() == "updates" {
                        let handle = app.clone();
                        let item = update_menu.clone();
                        let _ = item.set_enabled(false);
                        let _ = item.set_text("Checking for updates…");
                        tauri::async_runtime::spawn(async move {
                            let label = match update::check_for_updates(handle.clone()).await {
                                Ok(status) if status.state == "ready" => format!(
                                    "Update {} ready — open Driftnet",
                                    status.version.unwrap_or_default()
                                ),
                                Ok(status) if status.state == "current" => {
                                    "Up to date — check again".into()
                                }
                                _ => "Updates unavailable — retry".into(),
                            };
                            let _ = item.set_text(label);
                            let _ = item.set_enabled(true);
                            panel::show(&handle);
                        });
                    }
                    if event.id.as_ref() == "uninstall" {
                        let handle = app.clone();
                        tauri::async_runtime::spawn_blocking(move || {
                            if let Some(store) = handle.try_state::<Store>() {
                                let config = store.config.lock().unwrap().clone();
                                let _ = store.runtime.set(false, &config);
                            }
                            if let Err(error) = open_uninstaller(&handle) {
                                eprintln!("Could not open uninstaller: {error}");
                                panel::show(&handle);
                            } else {
                                handle.exit(0);
                            }
                        });
                    }
                    if event.id.as_ref() == "quit" {
                        app.exit(0)
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        rect,
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                        && let Some(w) = tray.app_handle().get_webview_window("main")
                    {
                        let same_anchor = panel::remember(tray.app_handle(), rect);
                        let just_hidden = tray
                            .app_handle()
                            .state::<panel::PanelState>()
                            .hidden_at
                            .lock()
                            .unwrap()
                            .take()
                            .is_some_and(|t| t.elapsed() < std::time::Duration::from_millis(250));
                        if (panel::is_open(tray.app_handle()) || just_hidden) && same_anchor {
                            panel::hide(&w);
                        } else {
                            panel::show(tray.app_handle());
                        }
                    }
                });
            #[cfg(target_os = "macos")]
            {
                tray = tray.title("d_");
            }
            #[cfg(not(target_os = "macos"))]
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            let _tray = tray.build(app)?;
            #[cfg(target_os = "macos")]
            let _tray_diagnostics = app.path().app_config_dir()?.join("tray-diagnostics.json");
            #[cfg(target_os = "macos")]
            let _ = _tray.with_inner_tray_icon(move |inner| {
                if let Some(item) = inner.ns_status_item()
                    && let Some(mtm) = objc2::MainThreadMarker::new()
                    && let Some(button) = item.button(mtm)
                {
                    button.setFont(Some(
                        &objc2_app_kit::NSFont::monospacedSystemFontOfSize_weight(14.0, 0.0),
                    ));
                    inner.set_title(Some("d_"));
                    #[cfg(debug_assertions)]
                    {
                        let font_size = button.font().map(|f| f.pointSize());
                        let _ = std::fs::write(
                            _tray_diagnostics,
                            serde_json::json!({"native_title":"d_","font_points":font_size})
                                .to_string(),
                        );
                    }
                }
            });
            if let Some(w) = app.get_webview_window("main") {
                let window = w.clone();
                w.on_window_event(move |event| {
                    #[cfg(debug_assertions)]
                    if matches!(
                        event,
                        tauri::WindowEvent::Moved(_) | tauri::WindowEvent::Resized(_)
                    ) {
                        panel::record_actual(&window);
                    }
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        panel::hide(&window);
                    }
                    if let tauri::WindowEvent::Focused(false) = event {
                        let state = window.app_handle().state::<panel::PanelState>();
                        if panel::is_open(window.app_handle()) && !state.picking.load(std::sync::atomic::Ordering::Relaxed)
                            && window.is_visible().unwrap_or(false)
                            && !window.is_focused().unwrap_or(true)
                        {
                            panel::hide_on_blur(&window);
                        }
                    }
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            read_state,
            add_site,
            change_rule,
            remove_rule,
            set_access,
            pick_app,
            set_panel_view,
            hide_panel,
            panel_ready,
            update::update_status,
            update::check_for_updates,
            install_update
        ])
        .build(tauri::generate_context!())
        .expect("Driftnet could not start")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event
                && let Some(store) = app.try_state::<Store>()
            {
                let config = store.config.lock().unwrap().clone();
                let _ = store.runtime.set(false, &config);
            }
            if let tauri::RunEvent::Ready = event {
                panel::show(app);
            }
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = event {
                panel::show(app);
            }
        });
}
