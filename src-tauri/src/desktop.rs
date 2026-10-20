use crate::{native, state::AppState};
use anyhow::{Result, ensure};
use interview_core::model::*;
use std::{
    sync::{Arc, atomic::Ordering},
    time::Duration,
};
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

pub fn monitors(app: &AppHandle) -> Result<Vec<MonitorInfo>> {
    Ok(app
        .available_monitors()?
        .iter()
        .enumerate()
        .map(|(i, m)| MonitorInfo {
            id: format!("{}-{i}", m.name().map(String::as_str).unwrap_or("Display")),
            x: m.position().x,
            y: m.position().y,
            width: m.size().width,
            height: m.size().height,
            scale: m.scale_factor(),
        })
        .collect())
}
pub fn sync_overlay(app: &AppHandle, state: &AppState) -> Result<()> {
    let window = app
        .get_webview_window("overlay")
        .ok_or_else(|| anyhow::anyhow!("字幕窗口不存在"))?;
    let settings = state.settings()?;
    window.set_position(PhysicalPosition::new(
        settings.overlay.x,
        settings.overlay.y,
    ))?;
    window.set_size(PhysicalSize::new(
        settings.overlay.width,
        settings.overlay.height,
    ))?;
    let adjust = state.adjusting.load(Ordering::SeqCst);
    window.set_ignore_cursor_events(!adjust)?;
    window.set_focusable(adjust)?;
    if state.is_visible()
        && !state.selecting.load(Ordering::SeqCst)
        && !state.capturing.load(Ordering::SeqCst)
    {
        window.show()?;
    } else {
        window.hide()?;
    }
    app.emit("overlay-settings", &settings.overlay)?;
    app.emit("overlay-mode", adjust)?;
    Ok(())
}
pub fn toggle(app: &AppHandle, state: &AppState) -> Result<()> {
    state.visible.fetch_xor(true, Ordering::SeqCst);
    if let Some(w) = app.get_webview_window("overlay") {
        if state.is_visible()
            && !state.selecting.load(Ordering::SeqCst)
            && !state.capturing.load(Ordering::SeqCst)
        {
            w.show()?;
        } else {
            w.hide()?;
        }
    }
    app.emit("visibility", state.is_visible())?;
    Ok(())
}
pub fn register_hotkeys(
    app: &AppHandle,
    settings: &Settings,
    old: Option<&Settings>,
) -> Result<()> {
    let capture: Shortcut = settings.capture_shortcut.parse()?;
    let toggle: Shortcut = settings.toggle_shortcut.parse()?;
    ensure!(capture != toggle, "快捷键冲突");
    let plugin = app.global_shortcut();
    plugin.unregister_all()?;
    let register = |capture: Shortcut, toggle: Shortcut| -> Result<()> {
        plugin.on_shortcut(capture, |app, _, event| {
            if event.state() == ShortcutState::Pressed {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = capture_question(&app, false).await {
                        let _ = app.emit("notice", e.to_string());
                    }
                });
            }
        })?;
        plugin.on_shortcut(toggle, |app, _, event| {
            if event.state() == ShortcutState::Pressed {
                let _ = self::toggle(app, &app.state::<Arc<AppState>>());
            }
        })?;
        Ok(())
    };
    if let Err(error) = register(capture, toggle) {
        plugin.unregister_all()?;
        if let Some(old) = old {
            register(old.capture_shortcut.parse()?, old.toggle_shortcut.parse()?)?;
        }
        return Err(error.context("快捷键已被占用或注册失败，原设置已保留"));
    }
    Ok(())
}
pub async fn capture_question(app: &AppHandle, cloud: bool) -> Result<()> {
    let state = app.state::<Arc<AppState>>().inner().clone();
    ensure!(!state.selecting.load(Ordering::SeqCst), "请先完成区域框选");
    if state.capturing.swap(true, Ordering::SeqCst) {
        return Ok(());
    }
    struct Reset(Arc<AppState>);
    impl Drop for Reset {
        fn drop(&mut self) {
            self.0.capturing.store(false, Ordering::SeqCst);
        }
    }
    let _reset = Reset(state.clone());
    let epoch = state.input_epoch.fetch_add(1, Ordering::SeqCst) + 1;
    app.emit("capture-feedback", ())?;
    let result = async {
        let region = state
            .settings()?
            .region
            .ok_or_else(|| anyhow::anyhow!("请先框选 OCR 区域"))?;
        region.validate(&monitors(app)?)?;
        let overlay = app.get_webview_window("overlay");
        if let Some(w) = &overlay {
            w.hide()?;
        }
        tokio::time::sleep(Duration::from_millis(80)).await;
        let image_result = tauri::async_runtime::spawn_blocking(move || {
            native::compositor_flush();
            native::capture(&region)
        })
        .await?;
        if state.is_visible()
            && let Some(w) = &overlay
        {
            w.show()?;
        }
        let image = image_result?;
        *state.last_image.lock().unwrap() = Some(image.clone());
        let question = if cloud {
            recognize_cloud(&state, &image).await?
        } else {
            let s = state.clone();
            tauri::async_runtime::spawn_blocking(move || {
                s.ocr
                    .lock()
                    .unwrap()
                    .as_mut()
                    .ok_or_else(|| anyhow::anyhow!("本地 OCR 尚未准备好"))?
                    .recognize(&image)
            })
            .await??
        };
        if state.input_epoch.load(Ordering::SeqCst) == epoch {
            submit_text(&state, question, false)?;
        }
        Ok(())
    }
    .await;
    // Restore even when the capture worker panics or returns an error.
    if state.is_visible()
        && !state.selecting.load(Ordering::SeqCst)
        && let Some(w) = app.get_webview_window("overlay")
    {
        let _ = w.show();
    }
    result
}
pub async fn recognize_cloud(state: &AppState, image: &image::RgbImage) -> Result<String> {
    use interview_core::provider::Attachment;
    let c = state.preparation_connection()?;
    let mut png = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(image.clone()).write_to(&mut png, image::ImageFormat::Png)?;
    let response=tokio::time::timeout(Duration::from_secs(state.settings()?.timeout_seconds),
        state.model.generate(&c,"Transcribe only the visible question text. Preserve numbers, negations and technical symbols. Do not answer the question. Return plain text.","Read this selected screen region.",
            &[Attachment{mime:"image/png".into(),bytes:png.into_inner()}],2048)).await??;
    Ok(response.text)
}
pub fn submit_text(state: &Arc<AppState>, question: String, force: bool) -> Result<CaptionState> {
    // Manual input also invalidates an older local/cloud OCR job.
    state.input_epoch.fetch_add(1, Ordering::SeqCst);
    let settings = state.settings()?;
    let profile = state.profile(
        &settings
            .active_profile
            .ok_or_else(|| anyhow::anyhow!("请选择已启用的面试档案"))?,
    )?;
    let ids = if settings.race {
        settings.answer_connections
    } else {
        settings.answer_connections.into_iter().take(1).collect()
    };
    let connections = ids
        .iter()
        .map(|id| state.connection(id))
        .collect::<Result<Vec<_>>>()?;
    state.runtime.submit(
        profile,
        question,
        connections,
        Duration::from_secs(settings.timeout_seconds),
        force,
    )
}
pub fn select_region(app: &AppHandle, state: &AppState) -> Result<()> {
    ensure!(
        !state.capturing.load(Ordering::SeqCst),
        "请等待本次截图完成"
    );
    if state.selecting.swap(true, Ordering::SeqCst) {
        return Ok(());
    }
    if let Some(w) = app.get_webview_window("overlay") {
        w.hide()?;
    }
    let result = (|| -> Result<()> {
        for (i, monitor) in monitors(app)?.into_iter().enumerate() {
            let label = format!("selector-{i}");
            state
                .selectors
                .lock()
                .unwrap()
                .insert(label.clone(), monitor.clone());
            let w = WebviewWindowBuilder::new(
                app,
                &label,
                WebviewUrl::App("index.html?window=selector".into()),
            )
            .title("框选 OCR 区域 · Esc 取消")
            .decorations(false)
            .transparent(true)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(false)
            .visible(false)
            .build()?;
            w.set_position(PhysicalPosition::new(monitor.x, monitor.y))?;
            w.set_size(PhysicalSize::new(monitor.width, monitor.height))?;
            w.show()?;
            w.set_focus()?;
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = finish_selection(app, state);
    }
    result
}
pub fn finish_selection(app: &AppHandle, state: &AppState) -> Result<()> {
    let labels = state
        .selectors
        .lock()
        .unwrap()
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    for label in labels {
        if let Some(w) = app.get_webview_window(&label) {
            w.destroy()?;
        }
    }
    state.selectors.lock().unwrap().clear();
    state.selecting.store(false, Ordering::SeqCst);
    if state.is_visible()
        && let Some(w) = app.get_webview_window("overlay")
    {
        w.show()?;
    }
    app.emit("settings-changed", ())?;
    Ok(())
}
