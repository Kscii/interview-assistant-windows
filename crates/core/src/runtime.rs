use crate::{
    model::*,
    normalize::question_key,
    prompt::{answer_prompt, validate_answer},
    provider::LanguageModel,
    store::Store,
};
use anyhow::{Result, ensure};
use futures::{StreamExt, stream::FuturesUnordered};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{sync::broadcast, time::Instant};

pub struct Runtime {
    pub store: Arc<Mutex<Store>>,
    pub state: Mutex<CaptionState>,
    model: Arc<dyn LanguageModel>,
    pub events: broadcast::Sender<CaptionState>,
}
impl Runtime {
    pub fn new(store: Arc<Mutex<Store>>, model: Arc<dyn LanguageModel>) -> Arc<Self> {
        let (events, _) = broadcast::channel(64);
        Arc::new(Self {
            store,
            state: Mutex::new(CaptionState::default()),
            model,
            events,
        })
    }
    pub fn snapshot(&self) -> CaptionState {
        self.state.lock().unwrap().clone()
    }
    fn publish(&self, state: &CaptionState) {
        let _ = self.events.send(state.clone());
    }
    pub fn invalidate(&self) {
        let mut state = self.state.lock().unwrap();
        *state = CaptionState {
            request_id: id(),
            status: "idle".into(),
            ..Default::default()
        };
        self.publish(&state);
    }
    pub fn submit(
        self: &Arc<Self>,
        profile: Profile,
        question: String,
        connections: Vec<Connection>,
        timeout: Duration,
        force: bool,
    ) -> Result<CaptionState> {
        ensure!(
            !question.trim().is_empty() && question.len() <= 12_000,
            "没有识别到有效问题，请重新框选或编辑文本"
        );
        let active = profile
            .active
            .clone()
            .ok_or_else(|| anyhow::anyhow!("请先确认启用面试档案"))?;
        let mut current = self.state.lock().unwrap();
        if !force
            && current.profile_id == profile.id
            && current.revision == active.revision
            && question_key(&current.question) == question_key(&question)
            && matches!(current.status.as_str(), "generating" | "delayed")
        {
            self.publish(&current);
            return Ok(current.clone());
        }
        let request_id = id();
        let mut state = CaptionState {
            request_id: request_id.clone(),
            profile_id: profile.id.clone(),
            revision: active.revision.clone(),
            question: question.trim().into(),
            status: "generating".into(),
            detail: "正在生成回答…".into(),
            ..Default::default()
        };
        if !force
            && let Some(cached) =
                self.store
                    .lock()
                    .unwrap()
                    .lookup(&profile.id, &active.revision, &question)?
        {
            state.answer = cached.text;
            state.status = "ready".into();
            state.cached = true;
            state.detail = if cached.prepared {
                "已准备答案"
            } else {
                "缓存命中"
            }
            .into();
            *current = state.clone();
            self.publish(&state);
            return Ok(state);
        }
        if connections.is_empty() {
            state.status = "failed".into();
            state.detail = "请选择即时回答模型".into();
            *current = state.clone();
            self.publish(&state);
            return Ok(state);
        }
        *current = state.clone();
        self.publish(&state);
        drop(current);
        let refs = self.store.lock().unwrap().answers(&profile.id)?;
        let system = answer_prompt(&active, &question, &refs);
        let runtime = self.clone();
        tokio::spawn(async move {
            let started = Instant::now();
            let mut calls = FuturesUnordered::new();
            for connection in connections {
                let model = runtime.model.clone();
                let system = system.clone();
                let question = question.clone();
                calls.push(async move {
                    let name = connection.name.clone();
                    let result = tokio::time::timeout(
                        timeout,
                        model.generate(&connection, &system, &question, &[], 4096),
                    )
                    .await;
                    let result = match result {
                        Ok(r) => r.and_then(|r| Ok((validate_answer(&r.text)?, r.usage))),
                        Err(_) => Err(anyhow::anyhow!("等待超过 {} 秒，请重试", timeout.as_secs())),
                    };
                    (name, result)
                });
            }
            let soft = tokio::time::sleep(Duration::from_secs(5));
            tokio::pin!(soft);
            let mut delayed = false;
            let mut errors = vec![];
            loop {
                tokio::select! {
                    _ = &mut soft, if !delayed => {
                        delayed = true;
                        let mut current = runtime.state.lock().unwrap();
                        if current.request_id != request_id { return; }
                        current.status = "delayed".into();
                        current.detail = "回答生成较慢，仍在等待…".into();
                        current.elapsed_ms = started.elapsed().as_millis() as u64;
                        runtime.publish(&current);
                    }
                    result = calls.next() => {
                        let Some((name, result)) = result else { break; };
                        let mut current = runtime.state.lock().unwrap();
                        if current.request_id != request_id { return; }
                        match result {
                            Ok((text, usage)) => {
                                let answer = Answer { id: id(), profile_id: profile.id.clone(), revision: active.revision.clone(),
                                    question: question.clone(), text: text.clone(), prepared: false, enabled: true, origin: name.clone() };
                                let store = runtime.store.lock().unwrap();
                                let saved = store.save_answer(&answer);
                                let _ = store.put("usage", &request_id, &usage);
                                current.answer = text; current.status = "ready".into();
                                current.detail = if saved.is_ok() { name } else { format!("{name} · 回答已显示，但缓存保存失败") };
                                current.elapsed_ms = started.elapsed().as_millis() as u64;
                                runtime.publish(&current);
                                return; // Dropping remaining futures cancels local HTTP reads.
                            }
                            Err(e) => errors.push(format!("{name}: {e}")),
                        }
                    }
                }
            }
            let mut current = runtime.state.lock().unwrap();
            if current.request_id == request_id {
                current.status = "failed".into();
                current.detail = errors.join("；");
                current.elapsed_ms = started.elapsed().as_millis() as u64;
                runtime.publish(&current);
            }
        });
        Ok(state)
    }
}
