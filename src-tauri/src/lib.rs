mod commands;
mod desktop;
mod native;
mod state;

use state::AppState;
use std::sync::{Arc, atomic::Ordering};
use tauri::{
    Emitter, Manager,
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
};

/// Installer smoke test: exercise the same OCR path using installed resources.
/// No window, API key, model request, or user data is involved.
pub fn check_installed_ocr(input: &std::path::Path, report: &std::path::Path) -> bool {
    let started = std::time::Instant::now();
    let result = (|| -> anyhow::Result<String> {
        let executable = std::env::current_exe()?;
        let root = executable
            .parent()
            .ok_or_else(|| anyhow::anyhow!("No application directory"))?;
        let mut ocr = native::Ocr::load(root)?;
        ocr.recognize(&image::open(input)?.into_rgb8())
    })();
    let (ok, text, error) = match result {
        Ok(text) => (true, text, String::new()),
        Err(error) => (false, String::new(), error.to_string()),
    };
    let json = serde_json::json!({"ok":ok,"text":text,"error":error,"elapsed_ms":started.elapsed().as_millis()});
    std::fs::write(report, serde_json::to_vec_pretty(&json).unwrap()).is_ok() && ok
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![commands::rpc])
        .setup(|app| {
            let root = app.path().app_local_data_dir()?;
            let state = AppState::new(&root)?;
            app.manage(state.clone());
            let settings = state.settings()?;
            let mut receiver = state.runtime.events.subscribe();
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    match receiver.recv().await {
                        Ok(caption) => {
                            let _ = handle.emit("caption", caption);
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(_) => break,
                    }
                }
            });
            let resources = app.path().resource_dir()?;
            let handle = app.handle().clone();
            let loading = state.clone();
            tauri::async_runtime::spawn_blocking(move || {
                match native::Ocr::load(&resources) {
                    Ok(ocr) => {
                        *loading.ocr.lock().unwrap() = Some(ocr);
                        *loading.ocr_status.lock().unwrap() = "本地 OCR 已就绪".into();
                    }
                    Err(e) => *loading.ocr_status.lock().unwrap() = e.to_string(),
                }
                let _ = handle.emit("settings-changed", ());
            });
            if let Err(e) = desktop::register_hotkeys(app.handle(), &settings, None) {
                // Keep the settings app usable when a shortcut is occupied.
                let _ =
                    state
                        .store
                        .lock()
                        .unwrap()
                        .put("startup", "shortcut_error", &e.to_string());
            } else {
                let _ = state
                    .store
                    .lock()
                    .unwrap()
                    .delete("startup", "shortcut_error");
            }
            desktop::sync_overlay(app.handle(), &state)?;
            let show = MenuItem::with_id(app, "show", "打开设置", true, None::<&str>)?;
            let capture = MenuItem::with_id(app, "capture", "识别并回答", true, None::<&str>)?;
            let toggle = MenuItem::with_id(app, "toggle", "显示 / 隐藏字幕", true, None::<&str>)?;
            let adjust = MenuItem::with_id(app, "adjust", "调整字幕位置", true, None::<&str>)?;
            let select = MenuItem::with_id(app, "select", "重新框选 OCR 区域", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &capture, &toggle, &adjust, &select, &quit])?;
            let mut tray = TrayIconBuilder::new()
                .menu(&menu)
                .tooltip("Interview Assistant");
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.on_menu_event(|app, event| {
                let state = app.state::<Arc<AppState>>();
                let result = match event.id.as_ref() {
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                        Ok(())
                    }
                    "capture" => {
                        let app = app.clone();
                        tauri::async_runtime::spawn(async move {
                            if let Err(e) = desktop::capture_question(&app, false).await {
                                let _ = app.emit("notice", e.to_string());
                            }
                        });
                        Ok(())
                    }
                    "toggle" => desktop::toggle(app, &state),
                    "adjust" => {
                        state.adjusting.store(true, Ordering::SeqCst);
                        state.visible.store(true, Ordering::SeqCst);
                        desktop::sync_overlay(app, &state)
                    }
                    "select" => desktop::select_region(app, &state),
                    "quit" => {
                        app.exit(0);
                        Ok(())
                    }
                    _ => Ok(()),
                };
                if let Err(e) = result {
                    let _ = app.emit("notice", e.to_string());
                }
            })
            .build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main"
                && let tauri::WindowEvent::CloseRequested { api, .. } = event
            {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("无法启动面试字幕助手");
}
