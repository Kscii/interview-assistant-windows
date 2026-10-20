use crate::model::{Source, hash, id};
use anyhow::{Result, bail, ensure};
use quick_xml::{Reader, events::Event};
use std::{
    io::{Cursor, Read},
    path::Path,
};

pub const MAX_FILE_BYTES: usize = 18 * 1024 * 1024;
pub const MAX_TEXT_BYTES: usize = 2 * 1024 * 1024;

pub fn import_bytes(name: &str, data: &[u8]) -> Result<Source> {
    ensure!(data.len() <= MAX_FILE_BYTES, "文件超过 18 MB，请拆分后导入");
    let ext = Path::new(name)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    let (mime, text) = match ext.as_str() {
        "txt" | "md" => ("text/plain", String::from_utf8(data.to_vec())?),
        "docx" => (
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            read_docx(data)?,
        ),
        "pdf" => {
            ensure!(data.starts_with(b"%PDF-"), "PDF 文件内容无效");
            ("application/pdf", String::new())
        }
        "png" => {
            ensure!(data.starts_with(b"\x89PNG\r\n\x1a\n"), "PNG 文件内容无效");
            ("image/png", String::new())
        }
        "jpg" | "jpeg" => {
            ensure!(data.starts_with(&[0xff, 0xd8]), "JPEG 文件内容无效");
            ("image/jpeg", String::new())
        }
        "webp" => {
            ensure!(
                data.len() > 12 && &data[8..12] == b"WEBP",
                "WebP 文件内容无效"
            );
            ("image/webp", String::new())
        }
        _ => bail!("支持 TXT、MD、DOCX、PDF、PNG、JPEG、WebP"),
    };
    ensure!(text.len() <= MAX_TEXT_BYTES, "文件文字过长，请拆分");
    let source_id = id();
    Ok(Source {
        id: source_id.clone(),
        name: name.into(),
        mime: mime.into(),
        text,
        blob: if matches!(ext.as_str(), "pdf" | "png" | "jpg" | "jpeg" | "webp") {
            Some(format!("{source_id}.{ext}"))
        } else {
            None
        },
        digest: hash(&base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            data,
        )),
        extracted: String::new(),
    })
}

fn read_docx(bytes: &[u8]) -> Result<String> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))?;
    let file = archive.by_name("word/document.xml")?;
    ensure!(
        file.size() <= MAX_TEXT_BYTES as u64,
        "DOCX 解压后的正文过大"
    );
    let mut xml = String::new();
    file.take(MAX_TEXT_BYTES as u64 + 1)
        .read_to_string(&mut xml)?;
    ensure!(xml.len() <= MAX_TEXT_BYTES, "DOCX 正文过大");
    let mut reader = Reader::from_str(&xml);
    let mut out = String::new();
    let mut in_text = false;
    loop {
        match reader.read_event()? {
            Event::Start(e) if e.local_name().as_ref() == b"t" => in_text = true,
            Event::End(e) if e.local_name().as_ref() == b"t" => in_text = false,
            Event::End(e) if matches!(e.local_name().as_ref(), b"p" | b"tr") => out.push('\n'),
            Event::End(e) if e.local_name().as_ref() == b"tc" => out.push('\t'),
            Event::Text(t) if in_text => out.push_str(&quick_xml::escape::unescape(&t.decode()?)?),
            Event::GeneralRef(r) if in_text => {
                let s = r.decode()?;
                out.push_str(&quick_xml::escape::unescape(&format!("&{s};"))?);
            }
            Event::Eof => break,
            _ => {}
        }
    }
    ensure!(
        !out.trim().is_empty(),
        "DOCX 没有可提取的正文；扫描文档请另存为 PDF"
    );
    Ok(out)
}

pub fn chunks(text: &str, max_chars: usize) -> Vec<String> {
    let mut chunks = vec![];
    let mut part = String::new();
    for (i, c) in text.chars().enumerate() {
        if i > 0 && i % max_chars == 0 {
            chunks.push(std::mem::take(&mut part));
        }
        part.push(c);
    }
    if !part.is_empty() {
        chunks.push(part);
    }
    chunks
}
