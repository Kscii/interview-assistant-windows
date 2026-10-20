use crate::{
    documents::chunks,
    model::*,
    prompt::{COMPILE_PROMPT, EXTRACT_PROMPT},
    provider::{Attachment, LanguageModel},
};
use anyhow::{Result, ensure};
use serde::de::DeserializeOwned;
use std::{path::Path, time::Duration};

pub fn parse_json<T: DeserializeOwned>(text: &str) -> Result<T> {
    let text = text.trim();
    let text = if text.starts_with("```") {
        text.split_once('\n')
            .map(|(_, t)| t.trim_end_matches('`').trim())
            .unwrap_or(text)
    } else {
        text
    };
    Ok(serde_json::from_str(text)?)
}

pub async fn extract_source(
    model: &dyn LanguageModel,
    connection: &Connection,
    source: &Source,
    root: &Path,
) -> Result<String> {
    let inputs = if source.text.is_empty() {
        vec![format!("Source name: {}", source.name)]
    } else {
        chunks(&source.text, 24_000)
    };
    let mut files = vec![];
    if let Some(blob) = &source.blob {
        ensure!(Path::new(blob).components().count() == 1, "资料路径无效");
        files.push(Attachment {
            mime: source.mime.clone(),
            bytes: std::fs::read(root.join(blob))?,
        });
    }
    let mut extracted = vec![];
    for text in inputs {
        let reply = tokio::time::timeout(
            Duration::from_secs(180),
            model.generate(connection, EXTRACT_PROMPT, &text, &files, 8192),
        )
        .await??;
        extracted.push(reply.text);
    }
    Ok(extracted.join("\n\n"))
}

pub async fn compile(
    model: &dyn LanguageModel,
    connection: &Connection,
    profile: &Profile,
) -> Result<CompiledProfile> {
    let sources: Vec<_> = profile
        .sources
        .iter()
        .map(|s| serde_json::json!({"id":s.id,"name":s.name,"material":s.extracted}))
        .collect();
    let input = serde_json::to_string(
        &serde_json::json!({"goal":profile.goal,"preferences":profile.rules,"sources":sources,"clarification":profile.clarification}),
    )?;
    ensure!(input.len() <= 500_000, "整理后的资料仍过长，请拆为多套档案");
    let reply = tokio::time::timeout(
        Duration::from_secs(180),
        model.generate(connection, COMPILE_PROMPT, &input, &[], 12000),
    )
    .await??;
    let result: CompiledProfile = parse_json(&reply.text)?;
    result.validate()?;
    let ids: std::collections::HashSet<_> = profile.sources.iter().map(|s| &s.id).collect();
    ensure!(
        result
            .cards
            .iter()
            .all(|c| c.source_ids.iter().all(|id| ids.contains(id))),
        "整理结果引用了不存在的来源，请重试"
    );
    Ok(result)
}
