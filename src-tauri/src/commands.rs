use crate::{
    desktop,
    state::{AppState, CredentialStore, Job, delete_key, save_key},
};
use anyhow::{Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use interview_core::{
    documents,
    model::*,
    preparation,
    prompt::{answer_prompt, validate_answer},
    provider::Secrets,
};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::{
    path::{Component, Path},
    sync::{Arc, atomic::Ordering},
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

fn field<T: DeserializeOwned>(payload: &Value, name: &str) -> Result<T> {
    Ok(serde_json::from_value(
        payload
            .get(name)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("缺少参数 {name}"))?,
    )?)
}
fn safe_blob(blob: &str) -> Result<()> {
    ensure!(
        matches!(
            Path::new(blob).components().next(),
            Some(Component::Normal(_))
        ) && Path::new(blob).components().count() == 1,
        "无效的资料文件名"
    );
    Ok(())
}
fn progress(app: &AppHandle, state: &AppState, job: Job) {
    let _ = state
        .store
        .lock()
        .unwrap()
        .put("job", &job.profile_id, &job);
    state
        .jobs
        .lock()
        .unwrap()
        .insert(job.profile_id.clone(), job.clone());
    let _ = app.emit("job", &job);
}
fn new_job(
    app: &AppHandle,
    state: &AppState,
    profile: &str,
    kind: &str,
    total: usize,
) -> Result<Job> {
    state.idle_profile(profile)?;
    let job = Job {
        id: id(),
        profile_id: profile.into(),
        kind: kind.into(),
        status: "running".into(),
        step: 0,
        total,
        detail: "开始处理…".into(),
    };
    progress(app, state, job.clone());
    Ok(job)
}

#[tauri::command]
pub async fn rpc(
    app: AppHandle,
    window: WebviewWindow,
    action: String,
    payload: Option<Value>,
) -> std::result::Result<Value, String> {
    dispatch(app, window, &action, payload.unwrap_or(Value::Null))
        .await
        .map_err(|e| e.to_string())
}
async fn dispatch(app: AppHandle, window: WebviewWindow, action: &str, p: Value) -> Result<Value> {
    let state = app.state::<Arc<AppState>>().inner().clone();
    if window.label() != "main" {
        ensure!(
            matches!(
                action,
                "caption"
                    | "selector_info"
                    | "finish_selection"
                    | "cancel_selection"
                    | "finish_adjusting"
            ),
            "此窗口没有此操作权限"
        );
    }
    match action {
        "caption" => Ok(
            json!({"caption":state.runtime.snapshot(),"overlay":state.settings()?.overlay,"adjusting":state.adjusting.load(Ordering::SeqCst)}),
        ),
        "snapshot" => {
            let profiles: Vec<Profile> = state.store.lock().unwrap().list("profile")?;
            let connections: Vec<Connection> = state.store.lock().unwrap().list("connection")?;
            let keyed = connections
                .iter()
                .map(|c| (c.id.clone(), CredentialStore.read(&c.id).is_ok()))
                .collect::<std::collections::HashMap<_, _>>();
            let shortcut_error = state
                .store
                .lock()
                .unwrap()
                .get::<String>("startup", "shortcut_error")?;
            Ok(
                json!({"profiles":profiles,"connections":connections,"has_keys":keyed,"settings":state.settings()?,"caption":state.runtime.snapshot(),
                "shortcut_error":shortcut_error,
                "ocr_status":*state.ocr_status.lock().unwrap(),"ocr_ready":state.ocr.lock().unwrap().is_some(),"windows":cfg!(windows),
                "visible":state.is_visible(),"jobs":state.jobs.lock().unwrap().values().cloned().collect::<Vec<_>>()}),
            )
        }
        "create_profile" => {
            let profile = Profile::new(&field::<String>(&p, "name")?);
            ensure!(!profile.name.trim().is_empty(), "请填写档案名称");
            state
                .store
                .lock()
                .unwrap()
                .put("profile", &profile.id, &profile)?;
            Ok(json!(profile))
        }
        "save_profile" => {
            let mut profile: Profile = field(&p, "profile")?;
            state.idle_profile(&profile.id)?;
            profile.rules.validate()?;
            ensure!(!profile.name.trim().is_empty(), "档案名称不能为空");
            let existing = state.profile(&profile.id)?;
            profile.active = existing.active;
            // Imported binary files must remain inside the app's source directory.
            for source in &mut profile.sources {
                if let Some(blob) = &source.blob {
                    safe_blob(blob)?;
                }
                ensure!(
                    source.text.len() <= documents::MAX_TEXT_BYTES,
                    "粘贴资料过长，请拆分"
                );
                if let Some(old) = existing.sources.iter().find(|s| s.id == source.id)
                    && (old.text != source.text || old.blob != source.blob)
                {
                    source.extracted.clear();
                }
            }
            state
                .store
                .lock()
                .unwrap()
                .put("profile", &profile.id, &profile)?;
            Ok(json!(profile))
        }
        "activate_profile" => {
            let id: String = field(&p, "id")?;
            state.idle_profile(&id)?;
            let mut profile = state.profile(&id)?;
            profile.activate()?;
            state.store.lock().unwrap().put("profile", &id, &profile)?;
            let mut settings = state.settings()?;
            settings.active_profile = Some(id);
            state
                .store
                .lock()
                .unwrap()
                .put("settings", "main", &settings)?;
            state.input_epoch.fetch_add(1, Ordering::SeqCst);
            state.runtime.invalidate();
            Ok(json!(profile))
        }
        "delete_profile" => {
            let id: String = field(&p, "id")?;
            state.idle_profile(&id)?;
            let profile = state.profile(&id)?;
            state.store.lock().unwrap().delete_profile(&id)?;
            for s in profile.sources {
                if let Some(blob) = s.blob {
                    safe_blob(&blob)?;
                    let _ = std::fs::remove_file(state.root.join("sources").join(blob));
                }
            }
            let mut settings = state.settings()?;
            if settings.active_profile.as_deref() == Some(&id) {
                settings.active_profile = None;
                state
                    .store
                    .lock()
                    .unwrap()
                    .put("settings", "main", &settings)?;
                state.input_epoch.fetch_add(1, Ordering::SeqCst);
                state.runtime.invalidate();
            }
            Ok(Value::Null)
        }
        "save_connection" => {
            let c: Connection = field(&p, "connection")?;
            c.validate()?;
            let secret = p.get("secret").and_then(Value::as_str).unwrap_or("");
            let old = state
                .store
                .lock()
                .unwrap()
                .get::<Connection>("connection", &c.id)?;
            if let Some(old) = old {
                ensure!(
                    old.base_url == c.base_url || !secret.is_empty(),
                    "更换接口地址时，请重新填写此接口的 API Key"
                );
            }
            if !secret.is_empty() {
                save_key(&c.id, secret.trim())
                    .map_err(|_| anyhow::anyhow!("无法写入系统凭据存储，请检查当前用户权限"))?;
            }
            state.store.lock().unwrap().put("connection", &c.id, &c)?;
            Ok(json!(c))
        }
        "delete_connection" => {
            let id: String = field(&p, "id")?;
            state.store.lock().unwrap().delete("connection", &id)?;
            delete_key(&id);
            let mut settings = state.settings()?;
            settings.answer_connections.retain(|x| x != &id);
            if settings.preparation_connection.as_ref() == Some(&id) {
                settings.preparation_connection = None;
            }
            state
                .store
                .lock()
                .unwrap()
                .put("settings", "main", &settings)?;
            Ok(Value::Null)
        }
        "test_connection" => {
            let c = state.connection(&field::<String>(&p, "id")?)?;
            let start = Instant::now();
            let r = tokio::time::timeout(
                Duration::from_secs(60),
                state
                    .model
                    .generate(&c, "Answer briefly.", "Reply with OK.", &[], 2048),
            )
            .await??;
            Ok(json!({"text":r.text,"elapsed_ms":start.elapsed().as_millis(),"usage":r.usage}))
        }
        "save_settings" => {
            let settings: Settings = field(&p, "settings")?;
            settings.validate()?;
            if let Some(id) = &settings.active_profile {
                ensure!(state.profile(id)?.active.is_some(), "所选档案尚未确认启用");
            }
            let old = state.settings()?;
            if settings.capture_shortcut != old.capture_shortcut
                || settings.toggle_shortcut != old.toggle_shortcut
                || state
                    .store
                    .lock()
                    .unwrap()
                    .get::<String>("startup", "shortcut_error")?
                    .is_some()
            {
                desktop::register_hotkeys(&app, &settings, Some(&old))?;
                state
                    .store
                    .lock()
                    .unwrap()
                    .delete("startup", "shortcut_error")?;
            }
            state
                .store
                .lock()
                .unwrap()
                .put("settings", "main", &settings)?;
            if settings.active_profile != old.active_profile {
                state.input_epoch.fetch_add(1, Ordering::SeqCst);
                state.runtime.invalidate();
            }
            desktop::sync_overlay(&app, &state)?;
            Ok(json!(settings))
        }
        "monitors" => Ok(json!(desktop::monitors(&app)?)),
        "select_region" => {
            desktop::select_region(&app, &state)?;
            Ok(Value::Null)
        }
        "selector_info" => Ok(json!(
            state
                .selectors
                .lock()
                .unwrap()
                .get(window.label())
                .ok_or_else(|| anyhow::anyhow!("框选窗口已失效"))?
        )),
        "finish_selection" => {
            let region: OcrRegion = field(&p, "region")?;
            let monitor = state
                .selectors
                .lock()
                .unwrap()
                .get(window.label())
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("请通过区域选择窗口操作"))?;
            ensure!(region.monitor == monitor, "显示器不匹配");
            region.validate(&desktop::monitors(&app)?)?;
            let mut settings = state.settings()?;
            settings.region = Some(region);
            state
                .store
                .lock()
                .unwrap()
                .put("settings", "main", &settings)?;
            desktop::finish_selection(&app, &state, false)?;
            Ok(Value::Null)
        }
        "cancel_selection" => {
            desktop::finish_selection(&app, &state, true)?;
            Ok(Value::Null)
        }
        "capture" => {
            desktop::capture_question(&app, false).await?;
            Ok(Value::Null)
        }
        "cloud_retry" => {
            let epoch = state.input_epoch.fetch_add(1, Ordering::SeqCst) + 1;
            let image = state
                .last_image
                .lock()
                .unwrap()
                .clone()
                .ok_or_else(|| anyhow::anyhow!("请先截图一次，再使用云端识别"))?;
            let q = desktop::recognize_cloud(&state, &image).await?;
            if state.input_epoch.load(Ordering::SeqCst) == epoch {
                Ok(json!(desktop::submit_text(&state, q, false)?))
            } else {
                Ok(Value::Null)
            }
        }
        "submit_text" => Ok(json!(desktop::submit_text(
            &state,
            field(&p, "question")?,
            p.get("force").and_then(Value::as_bool).unwrap_or(false)
        )?)),
        "toggle" => {
            desktop::toggle(&app, &state)?;
            Ok(json!(state.is_visible()))
        }
        "adjust_overlay" => {
            state.adjusting.store(true, Ordering::SeqCst);
            state.visible.store(true, Ordering::SeqCst);
            desktop::sync_overlay(&app, &state)?;
            Ok(Value::Null)
        }
        "finish_adjusting" => {
            let w = app
                .get_webview_window("overlay")
                .ok_or_else(|| anyhow::anyhow!("字幕窗口不存在"))?;
            let pos = w.outer_position()?;
            let size = w.inner_size()?;
            let mut settings = state.settings()?;
            settings.overlay.x = pos.x;
            settings.overlay.y = pos.y;
            settings.overlay.width = size.width;
            settings.overlay.height = size.height;
            settings.validate()?;
            state
                .store
                .lock()
                .unwrap()
                .put("settings", "main", &settings)?;
            state.adjusting.store(false, Ordering::SeqCst);
            desktop::sync_overlay(&app, &state)?;
            app.emit("settings-changed", ())?;
            Ok(Value::Null)
        }
        "import_source" => {
            let id: String = field(&p, "profile_id")?;
            state.idle_profile(&id)?;
            let path: String = field(&p, "path")?;
            ensure!(
                std::fs::metadata(&path)?.len() <= documents::MAX_FILE_BYTES as u64,
                "文件超过 18 MB，请拆分"
            );
            let bytes = std::fs::read(&path)?;
            let name = Path::new(&path)
                .file_name()
                .and_then(|s| s.to_str())
                .ok_or_else(|| anyhow::anyhow!("文件名无效"))?;
            let source = documents::import_bytes(name, &bytes)?;
            let mut profile = state.profile(&id)?;
            ensure!(profile.sources.len() < 100, "每套档案最多 100 个资料条目");
            if let Some(blob) = &source.blob {
                std::fs::write(state.root.join("sources").join(blob), &bytes)?;
            }
            profile.sources.push(source);
            profile.draft = None;
            state.store.lock().unwrap().put("profile", &id, &profile)?;
            Ok(json!(profile))
        }
        "prepare_profile" => {
            let id: String = field(&p, "id")?;
            let profile = state.profile(&id)?;
            ensure!(
                !profile.sources.is_empty() || !profile.clarification.trim().is_empty(),
                "请先导入资料或填写个人背景"
            );
            let c = state.preparation_connection()?;
            let mut job = new_job(&app, &state, &id, "prepare", profile.sources.len() + 1)?;
            let job_id = job.id.clone();
            tauri::async_runtime::spawn(async move {
                let work = async {
                    let mut profile = profile;
                    for index in 0..profile.sources.len() {
                        ensure!(state.job_current(&id, &job.id), "任务已取消");
                        job.step = index;
                        job.detail = format!("读取 {}", profile.sources[index].name);
                        progress(&app, &state, job.clone());
                        if profile.sources[index].extracted.is_empty() {
                            profile.sources[index].extracted = preparation::extract_source(
                                state.model.as_ref(),
                                &c,
                                &profile.sources[index],
                                &state.root.join("sources"),
                            )
                            .await?;
                            ensure!(state.job_current(&id, &job.id), "任务已取消");
                            state.store.lock().unwrap().put("profile", &id, &profile)?;
                        }
                    }
                    job.step = profile.sources.len();
                    job.detail = "整理定位、经历与待确认问题…".into();
                    progress(&app, &state, job.clone());
                    let compiled = preparation::compile(state.model.as_ref(), &c, &profile).await?;
                    ensure!(state.job_current(&id, &job.id), "任务已取消");
                    profile.draft = Some(compiled);
                    state.store.lock().unwrap().put("profile", &id, &profile)?;
                    Ok::<_, anyhow::Error>(())
                }
                .await;
                if state.job_current(&id, &job.id) {
                    match work {
                        Ok(()) => {
                            job.status = "done".into();
                            job.step = job.total;
                            job.detail = "整理完成，请查看、补充并确认档案".into();
                        }
                        Err(e) => {
                            job.status = "failed".into();
                            job.detail = e.to_string();
                        }
                    }
                    progress(&app, &state, job);
                }
            });
            Ok(json!(job_id))
        }
        "cancel_job" => {
            let id: String = field(&p, "id")?;
            let job = state.jobs.lock().unwrap().get(&id).cloned();
            if let Some(mut job) = job {
                job.status = "cancelled".into();
                job.detail = "已取消后续处理，已完成内容已保留".into();
                progress(&app, &state, job);
            }
            Ok(Value::Null)
        }
        "suggest_questions" => {
            let profile = state.profile(&field::<String>(&p, "id")?)?;
            let active = profile
                .active
                .ok_or_else(|| anyhow::anyhow!("请先启用档案"))?;
            let c = state.preparation_connection()?;
            let r=tokio::time::timeout(Duration::from_secs(180),state.model.generate(&c,
                "Suggest 12 relevant interview questions based on the candidate and goal. Return ONLY a JSON array of question strings, no answers.",
                &serde_json::to_string(&active)?,&[],4096)).await??;
            let qs: Vec<String> = preparation::parse_json(&r.text)?;
            ensure!(qs.len() <= 50, "建议问题数量异常");
            Ok(json!(qs))
        }
        "generate_answers" => {
            let id: String = field(&p, "id")?;
            let questions: Vec<String> = field(&p, "questions")?;
            ensure!(
                !questions.is_empty()
                    && questions.len() <= 50
                    && questions
                        .iter()
                        .all(|q| !q.trim().is_empty() && q.len() < 6000),
                "每批支持 1–50 道题"
            );
            let profile = state.profile(&id)?;
            let active = profile
                .active
                .clone()
                .ok_or_else(|| anyhow::anyhow!("请先启用档案"))?;
            let c = state.preparation_connection()?;
            let mut job = new_job(&app, &state, &id, "answers", questions.len())?;
            let jid = job.id.clone();
            tauri::async_runtime::spawn(async move {
                let mut errors = vec![];
                for (i, q) in questions.into_iter().enumerate() {
                    if !state.job_current(&id, &job.id) {
                        return;
                    }
                    job.step = i;
                    job.detail = format!("生成第 {} / {} 题", i + 1, job.total);
                    progress(&app, &state, job.clone());
                    let result = tokio::time::timeout(
                        Duration::from_secs(180),
                        state
                            .model
                            .generate(&c, &answer_prompt(&active, &q, &[]), &q, &[], 8192),
                    )
                    .await;
                    if !state.job_current(&id, &job.id) {
                        return;
                    }
                    let result = result
                        .map_err(anyhow::Error::from)
                        .and_then(|r| r)
                        .and_then(|r| validate_answer(&r.text));
                    match result {
                        Ok(text) => {
                            let a = Answer {
                                id: id::new(),
                                profile_id: id.clone(),
                                revision: active.revision.clone(),
                                question: q,
                                text,
                                prepared: true,
                                enabled: true,
                                origin: format!("模型生成 · {}", c.name),
                            };
                            if let Err(e) = state.store.lock().unwrap().save_answer(&a) {
                                errors.push(e.to_string());
                            }
                        }
                        Err(e) => errors.push(format!("第 {} 题：{e}", i + 1)),
                    }
                }
                job.step = job.total;
                job.status = if errors.is_empty() { "done" } else { "failed" }.into();
                job.detail = if errors.is_empty() {
                    "答案已生成并启用".into()
                } else {
                    format!("已保留成功答案；{}", errors.join("；"))
                };
                progress(&app, &state, job);
            });
            Ok(json!(jid))
        }
        "answers" => Ok(json!(state.store.lock().unwrap().answers(&field::<
            String,
        >(
            &p, "id"
        )?)?)),
        "save_answer" => {
            let mut answer: Answer = field(&p, "answer")?;
            answer.text = validate_answer(&answer.text)?;
            ensure!(!answer.question.trim().is_empty(), "问题不能为空");
            answer.origin = "人工修改".into();
            state.store.lock().unwrap().save_answer(&answer)?;
            // Repeated hotkey now picks the edited database value, not stale UI state.
            Ok(json!(answer))
        }
        "delete_answer" => {
            state
                .store
                .lock()
                .unwrap()
                .delete_answer(&field::<String>(&p, "id")?)?;
            Ok(Value::Null)
        }
        "clear_cache" => {
            state
                .store
                .lock()
                .unwrap()
                .clear_cache(&field::<String>(&p, "id")?)?;
            Ok(Value::Null)
        }
        "import_answers" => {
            let id: String = field(&p, "id")?;
            let profile = state.profile(&id)?;
            let active = profile
                .active
                .ok_or_else(|| anyhow::anyhow!("请先启用档案"))?;
            let pairs: Vec<Value> = field(&p, "answers")?;
            ensure!(pairs.len() <= 500, "一次最多导入 500 条问答");
            let mut imported = vec![];
            for pair in pairs {
                let q: String = field(&pair, "question")?;
                let text: String = field(&pair, "text")?;
                ensure!(!q.trim().is_empty(), "导入问题不能为空");
                let text = validate_answer(&text)?;
                imported.push(Answer {
                    id: id::new(),
                    profile_id: id.clone(),
                    revision: active.revision.clone(),
                    question: q,
                    text,
                    prepared: true,
                    enabled: true,
                    origin: "人工导入".into(),
                });
            }
            for a in &imported {
                state.store.lock().unwrap().save_answer(a)?;
            }
            Ok(json!(imported.len()))
        }
        "export_profile" => {
            let profile = state.profile(&field::<String>(&p, "id")?)?;
            let path: String = field(&p, "path")?;
            let mut blobs = serde_json::Map::new();
            for source in &profile.sources {
                if let Some(blob) = &source.blob {
                    safe_blob(blob)?;
                    blobs.insert(
                        blob.clone(),
                        json!(
                            STANDARD.encode(std::fs::read(state.root.join("sources").join(blob))?)
                        ),
                    );
                }
            }
            let answers = state.store.lock().unwrap().answers(&profile.id)?;
            let data = json!({"format":1,"profile":profile,"answers":answers.into_iter().filter(|a|a.prepared).collect::<Vec<_>>(),"blobs":blobs});
            std::fs::write(path, serde_json::to_vec_pretty(&data)?)?;
            Ok(Value::Null)
        }
        "import_profile" => {
            let path: String = field(&p, "path")?;
            ensure!(
                std::fs::metadata(&path)?.len() < 150 * 1024 * 1024,
                "档案包超过 150 MB"
            );
            let data: Value = serde_json::from_slice(&std::fs::read(path)?)?;
            ensure!(data.get("format") == Some(&json!(1)), "不支持此档案版本");
            let mut profile: Profile = field(&data, "profile")?;
            profile.rules.validate()?;
            profile.id = id::new();
            profile.name = format!("{}（导入）", profile.name);
            for source in &mut profile.sources {
                if let Some(old) = &source.blob {
                    safe_blob(old)?;
                    let encoded = data["blobs"][old]
                        .as_str()
                        .ok_or_else(|| anyhow::anyhow!("档案包缺少源文件"))?;
                    let bytes = STANDARD.decode(encoded)?;
                    ensure!(bytes.len() <= documents::MAX_FILE_BYTES, "源文件过大");
                    let parsed = documents::import_bytes(&source.name, &bytes)?;
                    let new_blob = parsed
                        .blob
                        .ok_or_else(|| anyhow::anyhow!("源文件类型不匹配"))?;
                    std::fs::write(state.root.join("sources").join(&new_blob), bytes)?;
                    source.blob = Some(new_blob);
                }
            }
            if let Some(active) = &profile.active {
                active.compiled.validate()?;
                active.rules.validate()?;
                ensure!(
                    active.revision
                        == hash(&serde_json::to_string(&(
                            &active.compiled,
                            &active.rules,
                            &active.goal
                        ))?),
                    "档案版本校验失败"
                );
            }
            let mut answers: Vec<Answer> = field(&data, "answers")?;
            for a in &mut answers {
                a.id = id::new();
                a.profile_id = profile.id.clone();
                a.text = validate_answer(&a.text)?;
            }
            state
                .store
                .lock()
                .unwrap()
                .put("profile", &profile.id, &profile)?;
            for a in answers {
                state.store.lock().unwrap().save_answer(&a)?;
            }
            Ok(json!(profile))
        }
        "export_prompt" => {
            let profile = state.profile(&field::<String>(&p, "id")?)?;
            let active = profile
                .active
                .ok_or_else(|| anyhow::anyhow!("请先启用档案"))?;
            std::fs::write(
                field::<String>(&p, "path")?,
                format!(
                    "{}\n\n## All material cards\n{}\n",
                    answer_prompt(&active, "", &[]),
                    serde_json::to_string_pretty(&active.compiled.cards)?
                ),
            )?;
            Ok(Value::Null)
        }
        _ => anyhow::bail!("未知操作"),
    }
}
// Avoid shadowing the UUID function with user-facing profile IDs.
mod id {
    pub fn new() -> String {
        interview_core::model::id()
    }
}
