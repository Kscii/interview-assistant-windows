use crate::native::Ocr;
use anyhow::Result;
use interview_core::{
    model::*,
    provider::{LanguageModel, RigModel, Secrets},
    runtime::Runtime,
    store::Store,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

pub struct CredentialStore;
impl Secrets for CredentialStore {
    fn read(&self, id: &str) -> Result<String> {
        Ok(keyring::Entry::new("dev.kscii.interview-assistant", id)?.get_password()?)
    }
}
pub fn save_key(id: &str, key: &str) -> Result<()> {
    keyring::Entry::new("dev.kscii.interview-assistant", id)?.set_password(key)?;
    Ok(())
}
pub fn delete_key(id: &str) {
    if let Ok(e) = keyring::Entry::new("dev.kscii.interview-assistant", id) {
        let _ = e.delete_credential();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: String,
    pub profile_id: String,
    pub kind: String,
    pub status: String,
    pub step: usize,
    pub total: usize,
    pub detail: String,
}
pub struct AppState {
    pub store: Arc<Mutex<Store>>,
    pub runtime: Arc<Runtime>,
    pub model: Arc<dyn LanguageModel>,
    pub root: PathBuf,
    pub ocr: Mutex<Option<Ocr>>,
    pub ocr_status: Mutex<String>,
    pub capturing: AtomicBool,
    pub input_epoch: AtomicU64,
    pub visible: AtomicBool,
    pub adjusting: AtomicBool,
    pub selecting: AtomicBool,
    pub jobs: Mutex<HashMap<String, Job>>,
    pub selectors: Mutex<HashMap<String, MonitorInfo>>,
    pub last_image: Mutex<Option<image::RgbImage>>,
}
impl AppState {
    pub fn new(root: &Path) -> Result<Arc<Self>> {
        std::fs::create_dir_all(root.join("sources"))?;
        let store = Arc::new(Mutex::new(Store::open(&root.join("interview.db"))?));
        let model: Arc<dyn LanguageModel> = Arc::new(RigModel::new(Arc::new(CredentialStore))?);
        let mut jobs = HashMap::new();
        for mut j in store.lock().unwrap().list::<Job>("job")? {
            if j.status == "running" {
                j.status = "interrupted".into();
                j.detail = "上次任务被中断；已完成的文件和答案已保留，可继续".into();
            }
            jobs.insert(j.profile_id.clone(), j);
        }
        Ok(Arc::new(Self {
            runtime: Runtime::new(store.clone(), model.clone()),
            store,
            model,
            root: root.into(),
            ocr: Mutex::new(None),
            ocr_status: Mutex::new("正在准备 OCR…".into()),
            capturing: AtomicBool::new(false),
            input_epoch: AtomicU64::new(0),
            visible: AtomicBool::new(true),
            adjusting: AtomicBool::new(false),
            selecting: AtomicBool::new(false),
            jobs: Mutex::new(jobs),
            selectors: Mutex::new(HashMap::new()),
            last_image: Mutex::new(None),
        }))
    }
    pub fn settings(&self) -> Result<Settings> {
        Ok(self
            .store
            .lock()
            .unwrap()
            .get("settings", "main")?
            .unwrap_or_default())
    }
    pub fn profile(&self, id: &str) -> Result<Profile> {
        self.store
            .lock()
            .unwrap()
            .get("profile", id)?
            .ok_or_else(|| anyhow::anyhow!("档案不存在"))
    }
    pub fn connection(&self, id: &str) -> Result<Connection> {
        self.store
            .lock()
            .unwrap()
            .get("connection", id)?
            .ok_or_else(|| anyhow::anyhow!("模型连接不存在"))
    }
    pub fn preparation_connection(&self) -> Result<Connection> {
        self.connection(
            &self
                .settings()?
                .preparation_connection
                .ok_or_else(|| anyhow::anyhow!("请选择资料整理模型"))?,
        )
    }
    pub fn idle_profile(&self, id: &str) -> Result<()> {
        anyhow::ensure!(
            !self
                .jobs
                .lock()
                .unwrap()
                .get(id)
                .is_some_and(|j| j.status == "running"),
            "此档案正在处理，请先取消任务"
        );
        Ok(())
    }
    pub fn job_current(&self, profile: &str, id: &str) -> bool {
        self.jobs
            .lock()
            .unwrap()
            .get(profile)
            .is_some_and(|j| j.id == id && j.status == "running")
    }
    pub fn is_visible(&self) -> bool {
        self.visible.load(Ordering::SeqCst)
    }
}
