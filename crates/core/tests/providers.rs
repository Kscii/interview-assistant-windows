//! Exercise the real HTTP adapters against localhost, without keys or paid calls.
use anyhow::Result;
use interview_core::{
    model::{Connection, ProviderKind},
    provider::{Attachment, LanguageModel, RigModel, Secrets},
};
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::Arc,
    thread,
    time::Duration,
};

struct TestSecret;
impl Secrets for TestSecret {
    fn read(&self, _: &str) -> Result<String> {
        Ok("local-fixture-credential".into())
    }
}

fn server(status: u16, response: Value) -> (String, thread::JoinHandle<(String, Value)>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        let mut bytes = Vec::new();
        let mut buf = [0; 8192];
        let (header_len, content_len) = loop {
            let n = stream.read(&mut buf).unwrap();
            assert!(n > 0);
            bytes.extend_from_slice(&buf[..n]);
            if let Some(end) = bytes.windows(4).position(|b| b == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&bytes[..end]);
                let length: usize = headers
                    .lines()
                    .find_map(|line| {
                        let (key, value) = line.split_once(':')?;
                        key.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse().unwrap())
                    })
                    .unwrap();
                break (end + 4, length);
            }
        };
        while bytes.len() < header_len + content_len {
            let n = stream.read(&mut buf).unwrap();
            assert!(n > 0);
            bytes.extend_from_slice(&buf[..n]);
        }
        let headers = String::from_utf8(bytes[..header_len].to_vec()).unwrap();
        let body = serde_json::from_slice(&bytes[header_len..header_len + content_len]).unwrap();
        let response = response.to_string();
        write!(stream, "HTTP/1.1 {status} Fixture\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}", response.len()).unwrap();
        (headers, body)
    });
    (url, handle)
}

fn connection(kind: ProviderKind, url: String) -> Connection {
    Connection {
        id: "fixture".into(),
        name: "Fixture".into(),
        kind,
        base_url: url,
        model: "example-model".into(),
        vision: true,
        pdf: true,
        parameters: json!({}),
    }
}

#[tokio::test]
async fn all_four_adapters_send_instructions_and_return_only_answer_text() {
    let fixtures = [
        (
            ProviderKind::Openai,
            "/responses",
            json!({
                "id":"resp_local","object":"response","created_at":1,"status":"completed","model":"example-model",
                "output":[{"type":"reasoning","id":"r","summary":[{"type":"summary_text","text":"PRIVATE_REASONING"}]},
                    {"type":"message","id":"m","role":"assistant","status":"completed","content":[{"type":"output_text","text":"A complete answer.","annotations":[]}]}],
                "usage":{"input_tokens":10,"output_tokens":5,"total_tokens":15}
            }),
        ),
        (
            ProviderKind::Compatible,
            "/chat/completions",
            json!({
                "id":"chat_local","object":"chat.completion","created":1,"model":"example-model",
                "choices":[{"index":0,"message":{"role":"assistant","content":"A complete answer.","reasoning_content":"PRIVATE_REASONING"},"finish_reason":"stop"}],
                "usage":{"prompt_tokens":10,"completion_tokens":5,"total_tokens":15}
            }),
        ),
        (
            ProviderKind::Gemini,
            ":generateContent",
            json!({
                "candidates":[{"content":{"role":"model","parts":[{"text":"PRIVATE_REASONING","thought":true},{"text":"A complete answer."}]},"finishReason":"STOP","index":0}],
                "usageMetadata":{"promptTokenCount":10,"candidatesTokenCount":5,"totalTokenCount":15}
            }),
        ),
        (
            ProviderKind::Anthropic,
            "/messages",
            json!({
                "id":"msg_local","type":"message","role":"assistant","model":"example-model",
                "content":[{"type":"thinking","thinking":"PRIVATE_REASONING","signature":"sig"},{"type":"text","text":"A complete answer."}],
                "stop_reason":"end_turn","stop_sequence":null,"usage":{"input_tokens":10,"output_tokens":5}
            }),
        ),
    ];
    for (kind, path, response) in fixtures {
        let (url, server) = server(200, response);
        let model = RigModel::new(Arc::new(TestSecret)).unwrap();
        let reply = model
            .generate(
                &connection(kind, url),
                "PERSONAL_PROFILE_RULES",
                "CURRENT_QUESTION",
                &[Attachment {
                    mime: "image/png".into(),
                    bytes: b"test-image-bytes".to_vec(),
                }],
                512,
            )
            .await
            .unwrap();
        assert_eq!(reply.text.trim(), "A complete answer.");
        let (headers, body) = server.join().unwrap();
        assert!(headers.lines().next().unwrap().contains(path), "{headers}");
        let body = body.to_string();
        assert!(body.contains("PERSONAL_PROFILE_RULES"));
        assert!(body.contains("CURRENT_QUESTION"));
        assert!(body.contains("dGVzdC1pbWFnZS1ieXRlcw=="));
        assert!(!body.contains("local-fixture-credential"));
    }
}

#[tokio::test]
async fn responses_pdf_is_sent_inline() {
    let (url, server) = server(
        200,
        json!({"id":"r","status":"completed","output":[
        {"type":"message","id":"m","role":"assistant","content":[{"type":"output_text","text":"Extracted text.","annotations":[]}]}]}),
    );
    let model = RigModel::new(Arc::new(TestSecret)).unwrap();
    model
        .generate(
            &connection(ProviderKind::Openai, url),
            "Read the document.",
            "Extract facts.",
            &[Attachment {
                mime: "application/pdf".into(),
                bytes: b"%PDF-fixture".to_vec(),
            }],
            512,
        )
        .await
        .unwrap();
    let (_, body) = server.join().unwrap();
    assert!(body.to_string().contains("application/pdf"));
    assert!(body.to_string().contains("input_file"));
}

#[tokio::test]
async fn errors_never_expose_key_or_provider_body() {
    let (url, server) = server(
        401,
        json!({"error":{"message":"secret-source-text local-fixture-credential","type":"auth","code":"401"}}),
    );
    let model = RigModel::new(Arc::new(TestSecret)).unwrap();
    let error = model
        .generate(
            &connection(ProviderKind::Openai, url),
            "Rules",
            "Question",
            &[],
            512,
        )
        .await
        .unwrap_err()
        .to_string();
    server.join().unwrap();
    assert!(!error.contains("secret-source-text"));
    assert!(!error.contains("local-fixture-credential"));
    assert!(error.contains("API Key"));
}

#[tokio::test]
async fn truncated_answers_are_not_cached_as_success() {
    let (url, server) = server(
        200,
        json!({"id":"r","object":"chat.completion","created":1,"model":"example-model",
        "choices":[{"index":0,"message":{"role":"assistant","content":"An incomplete"},"finish_reason":"length"}]}),
    );
    let model = RigModel::new(Arc::new(TestSecret)).unwrap();
    let result = model
        .generate(
            &connection(ProviderKind::Compatible, url),
            "Rules",
            "Question",
            &[],
            512,
        )
        .await;
    server.join().unwrap();
    assert!(result.is_err());
}
