use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub fn id() -> String {
    Uuid::new_v4().to_string()
}
pub fn hash(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AnswerRules {
    pub duration_seconds: u32,
    pub words_per_minute: u32,
    pub language: String,
    pub difficulty: String,
    pub style: String,
    pub instructions: String,
    pub optional_ending: bool,
}
impl Default for AnswerRules {
    fn default() -> Self {
        Self { duration_seconds: 60, words_per_minute: 90, language: "English".into(),
            difficulty: "Simple vocabulary, approximately 4000-word learner level; preserve necessary technical terms.".into(),
            style: "Professional spoken English. Short sentences and clear connecting words. Commas may mark speaking breaks; periods mark pauses.".into(),
            instructions: String::new(), optional_ending: true }
    }
}
impl AnswerRules {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            (15..=600).contains(&self.duration_seconds),
            "朗读时长必须为 15–600 秒"
        );
        ensure!(
            (40..=250).contains(&self.words_per_minute),
            "语速必须为每分钟 40–250 词"
        );
        ensure!(!self.language.trim().is_empty(), "请选择回答语言");
        ensure!(self.instructions.len() <= 32_000, "自定义提示词过长");
        Ok(())
    }
    pub fn word_budget(&self) -> (u32, u32) {
        let total = self.duration_seconds * self.words_per_minute / 60;
        let extra = if self.optional_ending { total / 5 } else { 0 };
        (total - extra, extra)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    pub id: String,
    pub name: String,
    pub mime: String,
    pub text: String,
    // Relative, app-owned filename. Never export a machine-local path.
    pub blob: Option<String>,
    pub digest: String,
    #[serde(default)]
    pub extracted: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MaterialCard {
    pub title: String,
    pub text: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub source_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CompiledProfile {
    pub identity: String,
    #[serde(default)]
    pub facts: Vec<String>,
    #[serde(default)]
    pub boundaries: Vec<String>,
    #[serde(default)]
    pub cards: Vec<MaterialCard>,
    #[serde(default)]
    pub questions: Vec<String>,
    #[serde(default)]
    pub unknowns: Vec<String>,
}
impl CompiledProfile {
    pub fn validate(&self) -> Result<()> {
        ensure!(!self.identity.trim().is_empty(), "整理结果缺少个人定位");
        ensure!(
            serde_json::to_string(self)?.len() <= 150_000,
            "整理结果过长，请分成多套档案"
        );
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveProfile {
    pub revision: String,
    pub compiled: CompiledProfile,
    pub rules: AnswerRules,
    pub goal: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub goal: String,
    pub rules: AnswerRules,
    pub sources: Vec<Source>,
    pub clarification: String,
    pub draft: Option<CompiledProfile>,
    pub active: Option<ActiveProfile>,
}
impl Profile {
    pub fn new(name: &str) -> Self {
        Self {
            id: id(),
            name: name.into(),
            goal: String::new(),
            rules: AnswerRules::default(),
            sources: vec![],
            clarification: String::new(),
            draft: None,
            active: None,
        }
    }
    pub fn activate(&mut self) -> Result<()> {
        self.rules.validate()?;
        let compiled = self
            .draft
            .clone()
            .ok_or_else(|| anyhow::anyhow!("请先整理或填写档案预览"))?;
        compiled.validate()?;
        let revision = hash(&serde_json::to_string(&(
            &compiled,
            &self.rules,
            &self.goal,
        ))?);
        self.active = Some(ActiveProfile {
            revision,
            compiled,
            rules: self.rules.clone(),
            goal: self.goal.clone(),
        });
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    Openai,
    Compatible,
    Gemini,
    Anthropic,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connection {
    pub id: String,
    pub name: String,
    pub kind: ProviderKind,
    pub base_url: String,
    pub model: String,
    pub vision: bool,
    pub pdf: bool,
    #[serde(default)]
    pub parameters: serde_json::Value,
}
impl Connection {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.id.is_empty() && !self.model.trim().is_empty(),
            "请填写模型 ID"
        );
        let url = url::Url::parse(&self.base_url)?;
        ensure!(
            url.username().is_empty()
                && url.password().is_none()
                && url.query().is_none()
                && url.fragment().is_none(),
            "接口地址不能包含密钥、查询参数或片段"
        );
        let local = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
        ensure!(
            url.scheme() == "https" || (url.scheme() == "http" && local),
            "远程 API 必须使用 HTTPS"
        );
        ensure!(
            self.parameters.is_null() || self.parameters.is_object(),
            "高级参数必须是 JSON 对象"
        );
        for forbidden in [
            "model",
            "messages",
            "input",
            "contents",
            "system",
            "systemInstruction",
            "instructions",
            "stream",
            "api_key",
            "tools",
        ] {
            ensure!(
                self.parameters.get(forbidden).is_none(),
                "高级参数不能覆盖 {forbidden}"
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MonitorInfo {
    pub id: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub scale: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrRegion {
    pub monitor: MonitorInfo,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}
impl OcrRegion {
    pub fn validate(&self, monitors: &[MonitorInfo]) -> Result<()> {
        ensure!(
            monitors.iter().any(|m| m == &self.monitor),
            "显示器布局或缩放已变化，请重新框选 OCR 区域"
        );
        ensure!(
            self.width >= 10
                && self.height >= 10
                && self.x as u64 + self.width as u64 <= self.monitor.width as u64
                && self.y as u64 + self.height as u64 <= self.monitor.height as u64,
            "OCR 区域超出显示器"
        );
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct OverlaySettings {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub font_size: u32,
    pub font_family: String,
}
impl Default for OverlaySettings {
    fn default() -> Self {
        Self {
            x: 100,
            y: 80,
            width: 1100,
            height: 430,
            font_size: 28,
            font_family: "Segoe UI".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub active_profile: Option<String>,
    pub preparation_connection: Option<String>,
    pub answer_connections: Vec<String>,
    pub race: bool,
    pub timeout_seconds: u64,
    pub region: Option<OcrRegion>,
    pub overlay: OverlaySettings,
    pub capture_shortcut: String,
    pub toggle_shortcut: String,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            active_profile: None,
            preparation_connection: None,
            answer_connections: vec![],
            race: false,
            timeout_seconds: 60,
            region: None,
            overlay: OverlaySettings::default(),
            capture_shortcut: "Ctrl+Alt+Space".into(),
            toggle_shortcut: "Ctrl+Alt+H".into(),
        }
    }
}
impl Settings {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            (10..=300).contains(&self.timeout_seconds),
            "请求超时必须为 10–300 秒"
        );
        ensure!(
            self.capture_shortcut != self.toggle_shortcut,
            "两个快捷键不能相同"
        );
        ensure!(
            (14..=80).contains(&self.overlay.font_size),
            "字号必须为 14–80"
        );
        ensure!(
            self.overlay.width >= 300 && self.overlay.height >= 120,
            "字幕窗口太小"
        );
        ensure!(self.answer_connections.len() <= 5, "最多选择 5 路模型");
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Answer {
    pub id: String,
    pub profile_id: String,
    pub revision: String,
    pub question: String,
    pub text: String,
    pub prepared: bool,
    pub enabled: bool,
    pub origin: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CaptionState {
    pub request_id: String,
    pub profile_id: String,
    pub revision: String,
    pub question: String,
    pub answer: String,
    pub status: String,
    pub detail: String,
    pub elapsed_ms: u64,
    pub cached: bool,
}
