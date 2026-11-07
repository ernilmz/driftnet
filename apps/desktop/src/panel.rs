use drift_core::panel::{self, Edge, Rect};
use std::sync::{
    Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use tauri::{AppHandle, Manager, WebviewWindow};
#[derive(Default)]
pub struct PanelState {
    pub settings: AtomicBool,
    pub picking: AtomicBool,
    anchor: Mutex<Option<Rect>>,
    pub hidden_at: Mutex<Option<std::time::Instant>>,
    ready: AtomicBool,
    open: AtomicBool,
    presentation: AtomicU64,
}
fn to_rect(rect: tauri::Rect) -> Rect {
    let p = rect.position.to_physical::<i32>(1.0);
    let s = rect.size.to_physical::<u32>(1.0);
    Rect {
        x: p.x,
        y: p.y,
        width: s.width,
        height: s.height,
    }
}
pub fn remember(app: &AppHandle, rect: tauri::Rect) -> bool {
    let state = app.state::<PanelState>();
    let value = to_rect(rect);
    let mut anchor = state.anchor.lock().unwrap();
    let same = anchor.is_some_and(|old| (old.x - value.x).abs() < 8 && (old.y - value.y).abs() < 8);
    *anchor = Some(value);
    same
}
#[cfg(not(target_os = "macos"))]
pub fn position(window: &WebviewWindow) -> Result<(), String> {
    let app = window.app_handle();
    let state = app.state::<PanelState>();
    let anchor = (*state.anchor.lock().map_err(|e| e.to_string())?).or_else(|| {
        app.tray_by_id("drift-tray")
            .and_then(|t| t.rect().ok().flatten())
            .map(to_rect)
    });
    let Some(anchor) = anchor else {
        #[cfg(target_os = "linux")]
        {
            // AppIndicator does not report click rectangles on Linux. Put the
            // reopened panel on the primary display even after a monitor move.
            let monitor = window
                .primary_monitor()
                .map_err(|e| e.to_string())?
                .ok_or("No primary monitor available")?;
            let work = monitor.work_area();
            let scale = monitor.scale_factor();
            let height = if state.settings.load(Ordering::Relaxed) {
                444.0
            } else {
                248.0
            };
            let width = ((424.0 * scale).round() as u32).min(work.size.width);
            let height = ((height * scale).round() as u32).min(work.size.height);
            window
                .set_size(tauri::PhysicalSize::new(width, height))
                .map_err(|e| e.to_string())?;
            return window
                .set_position(tauri::PhysicalPosition::new(
                    work.position.x + ((work.size.width - width) / 2) as i32,
                    work.position.y + ((work.size.height - height) / 2) as i32,
                ))
                .map_err(|e| e.to_string());
        }
        #[cfg(not(target_os = "linux"))]
        return window.center().map_err(|e| e.to_string());
    };
    // Select the monitor containing the clicked tray, NEVER the previous popup monitor.
    let monitor = window
        .monitor_from_point(
            anchor.x as f64 + anchor.width as f64 / 2.,
            anchor.y as f64 + anchor.height as f64 / 2.,
        )
        .map_err(|e| e.to_string())?
        .or(window.primary_monitor().map_err(|e| e.to_string())?)
        .ok_or("No monitor available")?;
    let scale = monitor.scale_factor();
    let height = if state.settings.load(Ordering::Relaxed) {
        444.0
    } else {
        248.0
    };
    let work = monitor.work_area();
    let target = panel::place(
        anchor,
        Rect {
            x: work.position.x,
            y: work.position.y,
            width: work.size.width,
            height: work.size.height,
        },
        (
            (424. * scale).round() as u32,
            (height * scale).round() as u32,
        ),
        if cfg!(target_os = "macos") {
            Edge::Below
        } else {
            Edge::Above
        },
        (6. * scale).round() as u32,
        (8. * scale).round() as u32,
    );
    // Move first so the OS applies the destination display's scale before sizing.
    window
        .set_position(tauri::PhysicalPosition::new(target.x, target.y))
        .map_err(|e| e.to_string())?;
    window
        .set_size(tauri::PhysicalSize::new(target.width, target.height))
        .map_err(|e| e.to_string())?;
    #[cfg(debug_assertions)]
    if let Ok(directory) = app.path().app_config_dir() {
        let _ = std::fs::create_dir_all(&directory);
        let actual = window.outer_position().ok();
        let size = window.outer_size().ok();
        let diagnostics = serde_json::json!({"anchor":{"x":anchor.x,"y":anchor.y,"width":anchor.width,"height":anchor.height},"work_area":{"x":work.position.x,"y":work.position.y,"width":work.size.width,"height":work.size.height},"scale":scale,"requested":{"x":target.x,"y":target.y,"width":target.width,"height":target.height},"actual_position":actual,"actual_size":size});
        let _ = std::fs::write(
            directory.join("panel-diagnostics.json"),
            diagnostics.to_string(),
        );
    }
    Ok(())
}
// AppKit and CoreGraphics use logical desktop points on macOS. Multiplying
// global origins by a display's backing scale breaks mixed-DPI monitor lookup.
#[cfg(target_os = "macos")]
pub fn position(window: &WebviewWindow) -> Result<(), String> {
    let tray = window
        .app_handle()
        .tray_by_id("drift-tray")
        .ok_or("Tray unavailable")?;
    let window = window.clone();
    tray.with_inner_tray_icon(move |inner| -> Result<(), String> {
        let mtm = objc2::MainThreadMarker::new().ok_or("AppKit requires the main thread")?;
        let item = inner.ns_status_item().ok_or("Status item unavailable")?;
        let status_window = item
            .button(mtm)
            .and_then(|b| b.window())
            .ok_or("Status window unavailable")?;
        let screen = status_window.screen().ok_or("Status screen unavailable")?;
        let screens = objc2_app_kit::NSScreen::screens(mtm);
        let primary = screens.firstObject().ok_or("Primary screen unavailable")?;
        let primary_top = primary.frame().origin.y + primary.frame().size.height;
        let frame = status_window.frame();
        let visible = screen.visibleFrame();
        let anchor = Rect {
            x: frame.origin.x.round() as i32,
            y: (primary_top - frame.origin.y - frame.size.height).round() as i32,
            width: frame.size.width.round() as u32,
            height: frame.size.height.round() as u32,
        };
        let work = Rect {
            x: visible.origin.x.round() as i32,
            y: (primary_top - visible.origin.y - visible.size.height).round() as i32,
            width: visible.size.width.round() as u32,
            height: visible.size.height.round() as u32,
        };
        let height = if window
            .app_handle()
            .state::<PanelState>()
            .settings
            .load(Ordering::Relaxed)
        {
            444
        } else {
            248
        };
        let target = panel::place(anchor, work, (424, height), Edge::Below, 6, 8);
        window
            .set_position(tauri::LogicalPosition::new(
                target.x as f64,
                target.y as f64,
            ))
            .map_err(|e| e.to_string())?;
        window
            .set_size(tauri::LogicalSize::new(
                target.width as f64,
                target.height as f64,
            ))
            .map_err(|e| e.to_string())?;
        #[cfg(debug_assertions)]
        if let Ok(directory) = window.app_handle().path().app_config_dir() {
            let _ = std::fs::create_dir_all(&directory);
            let diagnostics = serde_json::json!({
                "coordinate_space":"macOS logical desktop points",
                "anchor":{"x":anchor.x,"y":anchor.y,"width":anchor.width,"height":anchor.height},
                "work_area":{"x":work.x,"y":work.y,"width":work.width,"height":work.height},
                "scale":screen.backingScaleFactor(),
                "requested":{"x":target.x,"y":target.y,"width":target.width,"height":target.height}
            });
            let _ = std::fs::write(
                directory.join("panel-diagnostics.json"),
                diagnostics.to_string(),
            );
        }
        Ok(())
    })
    .map_err(|e| e.to_string())?
}

/// Record settled OS coordinates from window events, after async AppKit setters.
#[cfg(debug_assertions)]
pub fn record_actual(window: &WebviewWindow) {
    if let Ok(directory) = window.app_handle().path().app_config_dir() {
        let scale = window.scale_factor().unwrap_or(1.0);
        let position = window
            .outer_position()
            .ok()
            .map(|p| p.to_logical::<f64>(scale));
        let size = window.outer_size().ok().map(|s| s.to_logical::<f64>(scale));
        let _ = std::fs::write(
            directory.join("panel-actual.json"),
            serde_json::json!({"logical_position":position,"logical_size":size,"scale":scale})
                .to_string(),
        );
    }
}

pub fn show(app: &AppHandle) {
    let state = app.state::<PanelState>();
    state.open.store(true, Ordering::Release);
    state.presentation.fetch_add(1, Ordering::AcqRel);
    if !state.ready.load(Ordering::Acquire) {
        return;
    }
    if let Some(window) = app.get_webview_window("main") {
        if let Ok(mut at) = app.state::<PanelState>().hidden_at.lock() {
            *at = None;
        }
        let _ = position(&window);
        let _ = window.set_ignore_cursor_events(false);
        let _ = window.show();
        let _ = window.set_focus();
        // The hidden document already has the closed style. Two paint frames
        // prevent the OS show and the CSS destination from collapsing together.
        let _ = window.eval("requestAnimationFrame(() => requestAnimationFrame(() => document.documentElement.classList.add('panel-open')))");
    }
}

pub fn ready(app: &AppHandle) {
    let state = app.state::<PanelState>();
    state.ready.store(true, Ordering::Release);
    if state.open.load(Ordering::Acquire) {
        show(app);
    }
}

pub fn is_open(app: &AppHandle) -> bool {
    app.state::<PanelState>().open.load(Ordering::Acquire)
}

pub fn hide(window: &WebviewWindow) {
    let app = window.app_handle();
    let state = app.state::<PanelState>();
    state.open.store(false, Ordering::Release);
    let generation = state.presentation.fetch_add(1, Ordering::AcqRel) + 1;
    let _ = window.eval("document.documentElement.classList.remove('panel-open')");
    let _ = window.set_ignore_cursor_events(true);
    let window = window.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(145));
        let app = window.app_handle().clone();
        let _ = app.run_on_main_thread(move || {
            let state = window.app_handle().state::<PanelState>();
            // A rapid second click reverses the transition and cancels this hide.
            if state.presentation.load(Ordering::Acquire) == generation
                && !state.open.load(Ordering::Acquire)
            {
                let _ = window.hide();
            }
        });
    });
}

pub fn hide_on_blur(window: &WebviewWindow) {
    hide(window);
    if let Ok(mut at) = window.app_handle().state::<PanelState>().hidden_at.lock() {
        *at = Some(std::time::Instant::now());
    }
}
