use crate::model::{Connection, ProviderKind};
use anyhow::{Result, bail, ensure};
use async_trait::async_trait;
use base64::{Engine, engine::general_purpose::STANDARD};
use rig_core::{
    completion::{
        CompletionRequest,
        message::{
            Document, DocumentMediaType, DocumentSourceKind, Image, ImageMediaType, Message,
            UserContent,
        },
    },
    providers::{anthropic::AnthropicConfig, gemini::GeminiConfig, openai::OpenAIConfig},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone)]
pub struct Attachment {
    pub mime: String,
    pub bytes: Vec<u8>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmReply {
    pub text: String,
    pub usage: serde_json::Value,
}

pub trait Secrets: Send + Sync {
    fn read(&self, connection_id: &str) -> Result<String>;
}
#[async_trait]
pub trait LanguageModel: Send + Sync {
    async fn generate(
        &self,
        connection: &Connection,
        system: &str,
        user: &str,
        files: &[Attachment],
        max_tokens: u64,
    ) -> Result<LlmReply>;
}
pub struct RigModel {
    secrets: Arc<dyn Secrets>,
    http: rig_reqwest::ReqwestClient,
}
impl RigModel {
    pub fn new(secrets: Arc<dyn Secrets>) -> Result<Self> {
        let http = rig_reqwest::reqwest::Client::builder()
            .redirect(rig_reqwest::reqwest::redirect::Policy::none())
            .connect_timeout(std::time::Duration::from_secs(10))
            .timeout(std::time::Duration::from_secs(180))
            .build()?;
        Ok(Self {
            secrets,
            http: http.into(),
        })
    }
}

/// Never expose SDK errors containing signed URLs, API keys or uploaded material.
fn safe_error(error: &str) -> anyhow::Error {
    let message = if error.contains("401") || error.contains("403") {
        "API Key 无效或没有此模型权限"
    } else if error.contains("429") {
        "服务限流或额度不足，请稍后重试"
    } else if error.contains("404") {
        "模型或接口地址不存在"
    } else if error.contains("400") {
        "模型不支持当前输入或高级参数"
    } else if error.to_lowercase().contains("timeout") {
        "网络请求超时，可以重新尝试"
    } else {
        "模型调用失败，请检查连接、模型和高级参数后重试"
    };
    anyhow::anyhow!(message)
}

#[async_trait]
impl LanguageModel for RigModel {
    async fn generate(
        &self,
        c: &Connection,
        system: &str,
        user: &str,
        files: &[Attachment],
        max_tokens: u64,
    ) -> Result<LlmReply> {
        c.validate()?;
        let key = self.secrets.read(&c.id)?;
        ensure!(!key.trim().is_empty(), "此连接尚未保存 API Key");
        let mut content = vec![UserContent::text(user)];
        for file in files {
            let data = DocumentSourceKind::base64(STANDARD.encode(&file.bytes));
            if file.mime == "application/pdf" {
                ensure!(c.pdf, "所选模型连接未启用 PDF 输入能力");
                content.push(UserContent::Document(Document {
                    data,
                    media_type: Some(DocumentMediaType::PDF),
                    additional_params: None,
                }));
            } else {
                ensure!(c.vision, "所选模型连接未启用图片输入能力");
                let media = match file.mime.as_str() {
                    "image/png" => ImageMediaType::PNG,
                    "image/jpeg" => ImageMediaType::JPEG,
                    "image/webp" => ImageMediaType::WEBP,
                    _ => bail!("不支持此图片格式"),
                };
                content.push(UserContent::Image(Image {
                    data,
                    media_type: Some(media),
                    ..Default::default()
                }));
            }
        }
        let request = CompletionRequest::new(Message::User { content })
            .preamble(system)
            .max_tokens(max_tokens)
            .additional_params(if c.parameters.is_null() {
                None
            } else {
                Some(c.parameters.clone())
            });
        let result = match c.kind {
            ProviderKind::Openai => {
                OpenAIConfig::new(key)
                    .with_base_url(&c.base_url)
                    .connect(self.http.clone())
                    .responses(&c.model)
                    .call(request)
                    .await
            }
            ProviderKind::Compatible => {
                OpenAIConfig::new(key)
                    .with_base_url(&c.base_url)
                    .connect(self.http.clone())
                    .chat(&c.model)
                    .call(request)
                    .await
            }
            ProviderKind::Gemini => {
                GeminiConfig::new(key)
                    .with_base_url(&c.base_url)
                    .connect(self.http.clone())
                    .completion(&c.model)
                    .call(request)
                    .await
            }
            ProviderKind::Anthropic => {
                AnthropicConfig::new(key)
                    .with_base_url(&c.base_url)
                    .connect(self.http.clone())
                    .completion(&c.model)
                    .call(request)
                    .await
            }
        }
        .map_err(|e| safe_error(&e.to_string()))?;
        ensure!(
            result.error.is_none() && result.aborted.is_none(),
            "模型未完成本次回答"
        );
        let finish = format!("{:?}", result.finish_reason());
        ensure!(
            !finish.contains("Length") && !finish.contains("MaxTokens"),
            "输出达到模型上限，请提高输出 token 上限后重试"
        );
        let text = result.text();
        ensure!(!text.trim().is_empty(), "模型没有返回文本");
        Ok(LlmReply {
            text,
            usage: serde_json::to_value(result.usage)?,
        })
    }
}
