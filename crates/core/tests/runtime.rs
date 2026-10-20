use anyhow::Result;
use async_trait::async_trait;
use interview_core::{
    model::*,
    provider::{Attachment, LanguageModel, LlmReply},
    runtime::Runtime,
    store::Store,
};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

struct Mock {
    calls: AtomicUsize,
    fail_first: bool,
}
#[async_trait]
impl LanguageModel for Mock {
    async fn generate(
        &self,
        c: &Connection,
        _: &str,
        q: &str,
        _: &[Attachment],
        _: u64,
    ) -> Result<LlmReply> {
        let n = self.calls.fetch_add(1, Ordering::SeqCst);
        let delay = if q == "old question" {
            12
        } else {
            c.model.parse().unwrap_or(1)
        };
        tokio::time::sleep(Duration::from_secs(delay)).await;
        if c.name == "fail" || (self.fail_first && n == 0) {
            anyhow::bail!("simulated network failure");
        }
        Ok(LlmReply {
            text: if c.name == "empty" {
                String::new()
            } else {
                format!("A complete spoken answer to {q}. | One more thought. Another thought.")
            },
            usage: serde_json::json!({"input_tokens":10,"output_tokens":20}),
        })
    }
}
fn profile() -> Profile {
    let mut p = Profile::new("Fictional demo");
    p.draft = Some(CompiledProfile {
        identity: "A software engineering student.".into(),
        ..Default::default()
    });
    p.activate().unwrap();
    p
}
fn conn(name: &str, seconds: u64) -> Connection {
    Connection {
        id: id(),
        name: name.into(),
        kind: ProviderKind::Openai,
        base_url: "https://api.openai.com/v1".into(),
        model: seconds.to_string(),
        vision: true,
        pdf: true,
        parameters: serde_json::Value::Null,
    }
}
fn setup(fail_first: bool) -> (Arc<Runtime>, Arc<Mock>) {
    let mock = Arc::new(Mock {
        calls: AtomicUsize::new(0),
        fail_first,
    });
    (
        Runtime::new(Arc::new(Mutex::new(Store::memory().unwrap())), mock.clone()),
        mock,
    )
}
async fn tick(seconds: u64) {
    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_secs(seconds)).await;
    tokio::task::yield_now().await;
}
#[tokio::test(start_paused = true)]
async fn late_result_is_displayed_and_cached() {
    let (r, m) = setup(false);
    let p = profile();
    r.submit(
        p.clone(),
        "Why this program?".into(),
        vec![conn("slow", 8)],
        Duration::from_secs(60),
        false,
    )
    .unwrap();
    tick(5).await;
    assert_eq!(r.snapshot().status, "delayed");
    tick(3).await;
    assert_eq!(r.snapshot().status, "ready");
    assert!(!r.snapshot().answer.is_empty());
    let a = r.snapshot().answer;
    let repeat = r
        .submit(
            p,
            "WHY THIS PROGRAM!".into(),
            vec![],
            Duration::from_secs(60),
            false,
        )
        .unwrap();
    assert!(repeat.cached);
    assert_eq!(repeat.answer, a);
    assert_eq!(m.calls.load(Ordering::SeqCst), 1);
}
#[tokio::test(start_paused = true)]
async fn duplicate_while_waiting_does_not_restart() {
    let (r, m) = setup(false);
    let p = profile();
    let first = r
        .submit(
            p.clone(),
            "same?".into(),
            vec![conn("slow", 9)],
            Duration::from_secs(60),
            false,
        )
        .unwrap();
    tick(6).await;
    let second = r
        .submit(
            p,
            "SAME!".into(),
            vec![conn("slow", 9)],
            Duration::from_secs(60),
            false,
        )
        .unwrap();
    assert_eq!(first.request_id, second.request_id);
    tick(3).await;
    assert_eq!(m.calls.load(Ordering::SeqCst), 1);
    assert_eq!(r.snapshot().status, "ready");
}
#[tokio::test(start_paused = true)]
async fn failed_same_question_retries() {
    let (r, m) = setup(true);
    let p = profile();
    r.submit(
        p.clone(),
        "same".into(),
        vec![conn("main", 1)],
        Duration::from_secs(60),
        false,
    )
    .unwrap();
    tick(1).await;
    assert_eq!(r.snapshot().status, "failed");
    r.submit(
        p,
        "same".into(),
        vec![conn("main", 1)],
        Duration::from_secs(60),
        false,
    )
    .unwrap();
    tick(1).await;
    assert_eq!(r.snapshot().status, "ready");
    assert_eq!(m.calls.load(Ordering::SeqCst), 2);
}
#[tokio::test(start_paused = true)]
async fn new_question_cannot_be_overwritten_by_old_response() {
    let (r, _) = setup(false);
    let p = profile();
    r.submit(
        p.clone(),
        "old question".into(),
        vec![conn("main", 1)],
        Duration::from_secs(60),
        false,
    )
    .unwrap();
    tick(1).await;
    r.submit(
        p,
        "new question".into(),
        vec![conn("main", 1)],
        Duration::from_secs(60),
        false,
    )
    .unwrap();
    tick(1).await;
    let new = r.snapshot();
    assert_eq!(new.status, "ready");
    tick(15).await;
    assert_eq!(r.snapshot().request_id, new.request_id);
    assert_eq!(r.snapshot().answer, new.answer);
}
#[tokio::test(start_paused = true)]
async fn race_ignores_empty_answer_and_later_loser() {
    let (r, m) = setup(false);
    r.submit(
        profile(),
        "test".into(),
        vec![conn("empty", 1), conn("winner", 2), conn("loser", 9)],
        Duration::from_secs(60),
        false,
    )
    .unwrap();
    tick(1).await;
    assert_eq!(r.snapshot().status, "generating");
    tick(1).await;
    assert_eq!(r.snapshot().detail, "winner");
    let text = r.snapshot().answer;
    tick(20).await;
    assert_eq!(r.snapshot().answer, text);
    assert_eq!(r.snapshot().detail, "winner");
    assert_eq!(m.calls.load(Ordering::SeqCst), 3);
}
#[tokio::test(start_paused = true)]
async fn final_timeout_can_be_retried_without_clearing() {
    let (r, _) = setup(false);
    let p = profile();
    r.submit(
        p.clone(),
        "test".into(),
        vec![conn("slow", 99)],
        Duration::from_secs(10),
        false,
    )
    .unwrap();
    tick(11).await;
    assert_eq!(r.snapshot().status, "failed");
    r.submit(
        p,
        "test".into(),
        vec![conn("fast", 1)],
        Duration::from_secs(10),
        false,
    )
    .unwrap();
    tick(1).await;
    assert_eq!(r.snapshot().status, "ready");
}
#[tokio::test(start_paused = true)]
async fn force_regeneration_bypasses_cache() {
    let (r, m) = setup(false);
    let p = profile();
    r.submit(
        p.clone(),
        "test".into(),
        vec![conn("main", 1)],
        Duration::from_secs(60),
        false,
    )
    .unwrap();
    tick(1).await;
    r.submit(
        p,
        "test".into(),
        vec![conn("main", 1)],
        Duration::from_secs(60),
        true,
    )
    .unwrap();
    tick(1).await;
    assert_eq!(m.calls.load(Ordering::SeqCst), 2);
    assert!(!r.snapshot().cached);
}
#[tokio::test(start_paused = true)]
async fn switching_profile_invalidates_inflight_result() {
    let (r, _) = setup(false);
    r.submit(
        profile(),
        "test".into(),
        vec![conn("main", 2)],
        Duration::from_secs(60),
        false,
    )
    .unwrap();
    tick(1).await;
    r.invalidate();
    tick(10).await;
    assert_eq!(r.snapshot().status, "idle");
    assert!(r.snapshot().answer.is_empty());
}
