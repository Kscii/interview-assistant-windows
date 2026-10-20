use interview_core::{
    documents, model::*, normalize::question_key, prompt::answer_prompt, store::Store,
};
use std::io::{Cursor, Write};
fn profile() -> Profile {
    let mut p = Profile::new("Example");
    p.draft = Some(CompiledProfile {
        identity: "A fictional engineer.".into(),
        ..Default::default()
    });
    p.activate().unwrap();
    p
}
#[test]
fn changing_rules_requires_activation_and_invalidates_old_cache() {
    let db = Store::memory().unwrap();
    let mut p = profile();
    let old = p.active.clone().unwrap();
    let a = Answer {
        id: id(),
        profile_id: p.id.clone(),
        revision: old.revision.clone(),
        question: "Why this program?".into(),
        text: "My answer.".into(),
        prepared: true,
        enabled: true,
        origin: "test".into(),
    };
    db.save_answer(&a).unwrap();
    p.rules.duration_seconds = 120;
    assert_eq!(p.active.as_ref().unwrap().revision, old.revision);
    p.activate().unwrap();
    assert!(
        db.lookup(&p.id, &p.active.unwrap().revision, "Why this program?")
            .unwrap()
            .is_none()
    );
}
#[test]
fn prepared_answers_beat_runtime_cache_and_manual_edits_are_exact() {
    let db = Store::memory().unwrap();
    let p = profile();
    let revision = p.active.unwrap().revision;
    let mut a = Answer {
        id: id(),
        profile_id: p.id.clone(),
        revision: revision.clone(),
        question: "WHY AI?".into(),
        text: "Prepared text.".into(),
        prepared: true,
        enabled: true,
        origin: "test".into(),
    };
    db.save_answer(&a).unwrap();
    let b = Answer {
        id: id(),
        prepared: false,
        text: "Cached text.".into(),
        ..a.clone()
    };
    db.save_answer(&b).unwrap();
    assert_eq!(
        db.lookup(&p.id, &revision, "why ai!")
            .unwrap()
            .unwrap()
            .text,
        "Prepared text."
    );
    a.text = "My simple wording, exactly. | Extra one. Extra two.".into();
    db.save_answer(&a).unwrap();
    assert_eq!(
        db.lookup(&p.id, &revision, "Why AI?")
            .unwrap()
            .unwrap()
            .text,
        a.text
    );
    a.enabled = false;
    db.save_answer(&a).unwrap();
    assert_eq!(
        db.lookup(&p.id, &revision, "Why AI?")
            .unwrap()
            .unwrap()
            .text,
        b.text
    );
}
#[test]
fn cache_never_crosses_profiles() {
    let db = Store::memory().unwrap();
    let p = profile();
    let a = Answer {
        id: id(),
        profile_id: p.id,
        revision: "v".into(),
        question: "Why?".into(),
        text: "Answer".into(),
        prepared: false,
        enabled: true,
        origin: "test".into(),
    };
    db.save_answer(&a).unwrap();
    assert!(db.lookup("another", "v", "why").unwrap().is_none());
}
#[test]
fn database_survives_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.db");
    {
        let db = Store::open(&path).unwrap();
        db.put("settings", "main", &Settings::default()).unwrap();
    }
    assert_eq!(
        Store::open(&path)
            .unwrap()
            .get::<Settings>("settings", "main")
            .unwrap()
            .unwrap()
            .timeout_seconds,
        60
    );
}
#[test]
fn docx_preserves_table_paragraphs_and_entities() {
    let mut bytes = Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut bytes);
        z.start_file(
            "word/document.xml",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        z.write_all(br#"<w:document xmlns:w="w"><w:p><w:r><w:t>Research &amp; development</w:t></w:r></w:p><w:tbl><w:tr><w:tc><w:p><w:r><w:t>Year</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>2026</w:t></w:r></w:p></w:tc></w:tr></w:tbl></w:document>"#).unwrap();
        z.finish().unwrap();
    }
    let source = documents::import_bytes("cv.docx", &bytes.into_inner()).unwrap();
    assert!(source.text.contains("Research & development"));
    assert!(source.text.contains("Year"));
    assert!(source.text.contains("2026"));
    assert!(source.text.contains('\t'));
}
#[test]
fn binary_validation_and_unicode_chunking() {
    assert!(documents::import_bytes("scan.pdf", b"not pdf").is_err());
    assert!(documents::import_bytes("doc.doc", b"old format").is_err());
    let text = "中文材料，保留技术术语。".repeat(100);
    let chunks = documents::chunks(&text, 17);
    assert_eq!(chunks.concat(), text);
}
#[test]
fn region_rejects_layout_and_scale_changes() {
    let m = MonitorInfo {
        id: "left".into(),
        x: -1920,
        y: 0,
        width: 1920,
        height: 1080,
        scale: 1.5,
    };
    let r = OcrRegion {
        monitor: m.clone(),
        x: 10,
        y: 20,
        width: 1000,
        height: 200,
    };
    r.validate(std::slice::from_ref(&m)).unwrap();
    let mut moved = m.clone();
    moved.x = 0;
    assert!(r.validate(&[moved]).is_err());
    let mut scaled = m;
    scaled.scale = 1.;
    assert!(r.validate(&[scaled]).is_err());
}
#[test]
fn preferences_and_current_question_do_not_become_personal_facts() {
    let p = profile();
    let active = p.active.unwrap();
    let prompt = answer_prompt(&active, "A new question", &[]);
    assert!(prompt.contains("A fictional engineer."));
    assert!(prompt.contains("about 72 words"));
    assert!(prompt.contains("exactly two"));
    assert!(!prompt.contains("A new question"));
}
#[test]
fn rejects_credential_in_url_and_instruction_override() {
    let mut c = Connection {
        id: id(),
        name: "demo".into(),
        kind: ProviderKind::Openai,
        base_url: "https://api.openai.com/v1?key=secret".into(),
        model: "example".into(),
        vision: false,
        pdf: false,
        parameters: serde_json::Value::Null,
    };
    assert!(c.validate().is_err());
    c.base_url = "https://api.openai.com/v1".into();
    c.parameters = serde_json::json!({"messages":[]});
    assert!(c.validate().is_err());
    c.parameters = serde_json::json!({"reasoning":{"effort":"medium"}});
    c.validate().unwrap();
}
#[test]
fn matching_preserves_negation_and_numbers() {
    assert_ne!(
        question_key("What do you like?"),
        question_key("What do you not like?")
    );
    assert_ne!(question_key("10000 years"), question_key("1000 years"));
}
