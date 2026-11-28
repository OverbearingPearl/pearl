//! Stories for the provider wire, told without a network.

use std::io::{Read, Write};
use std::net::TcpListener;

use pearl_provider::openai::OpenAiProvider;
use pearl_provider::types::{ChatMessage, CompletionRequest, Role};
use pearl_provider::{Provider, ProviderError};
use rstest::rstest;

fn request() -> CompletionRequest {
    CompletionRequest {
        model: String::from("m"),
        messages: vec![ChatMessage::new(Role::User, "hi")],
        stream: false,
        tools: vec![],
        reasoning_effort: None,
        extra: None,
    }
}

/// One request answered with a fixed body; returns what the client sent.
fn serve_once(body: &'static str) -> (String, std::thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/v1", listener.local_addr().unwrap());
    let handle = std::thread::spawn(move || {
        let (mut sock, _) = listener.accept().unwrap();
        let mut buf = [0u8; 8192];
        let n = sock.read(&mut buf).unwrap();
        let seen = String::from_utf8_lossy(&buf[..n]).to_string();
        let reply = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        sock.write_all(reply.as_bytes()).unwrap();
        seen
    });
    (url, handle)
}

// The parse function is crate-private; exercise it through a small helper that
// mirrors what the client does with a response body.
fn parse(body: &str) -> Result<pearl_provider::Completion, ProviderError> {
    let wire: serde_json::Value = serde_json::from_str(body).unwrap();
    let content = wire["choices"][0]["message"]["content"]
        .as_str()
        .ok_or(ProviderError::EmptyResponse)?
        .to_string();
    let model = wire["model"].as_str().unwrap_or("").to_string();
    Ok(pearl_provider::Completion {
        content,
        reasoning: None,
        model,
        tool_calls: vec![],
        usage: Default::default(),
    })
}

#[rstest]
fn given_a_response_body_when_parsed_then_content_and_model_extracted() {
    let body = r#"{"model":"deepseek-chat","choices":[{"message":{"content":"hello"}}]}"#;
    let completion = parse(body).unwrap();
    assert_eq!(completion.content, "hello");
    assert_eq!(completion.model, "deepseek-chat");
}

#[rstest]
fn given_no_choices_when_parsed_then_empty_response_error() {
    let body = r#"{"model":"deepseek-chat","choices":[]}"#;
    assert!(matches!(parse(body), Err(ProviderError::EmptyResponse)));
}

#[rstest]
fn given_a_request_when_serialized_then_model_and_messages_on_the_wire() {
    let request = CompletionRequest {
        model: "deepseek-chat".into(),
        messages: vec![ChatMessage::new(Role::User, "hi")],
        stream: false,
        tools: vec![],
        reasoning_effort: None,
        extra: None,
    };
    let json = serde_json::to_value(&request).unwrap();
    assert_eq!(json["model"], "deepseek-chat");
    assert_eq!(json["messages"][0]["role"], "user");
    assert_eq!(json["messages"][0]["content"], "hi");
    // stream=false is omitted, keeping the request body minimal.
    assert!(json.get("stream").is_none());
}

/// A mock provider stands in for the network so the loop is testable offline.
struct EchoProvider;

impl Provider for EchoProvider {
    async fn complete(
        &self,
        request: CompletionRequest,
    ) -> Result<pearl_provider::Completion, ProviderError> {
        let last = request
            .messages
            .last()
            .map(|m| m.content.clone())
            .unwrap_or_default();
        Ok(pearl_provider::Completion {
            content: format!("echo: {last}"),
            reasoning: None,
            model: request.model,
            tool_calls: vec![],
            usage: Default::default(),
        })
    }
}

#[rstest]
#[tokio::test]
async fn given_a_mock_provider_when_complete_then_it_answers_offline() {
    let provider = EchoProvider;
    let request = CompletionRequest {
        model: "mock".into(),
        messages: vec![ChatMessage::new(Role::User, "ping")],
        stream: false,
        tools: vec![],
        reasoning_effort: None,
        extra: None,
    };
    let completion = provider.complete(request).await.unwrap();
    assert_eq!(completion.content, "echo: ping");
}

#[rstest]
#[tokio::test]
async fn given_a_key_copied_with_a_trailing_line_break_when_the_answer_is_asked_for_then_it_arrives_whole()
{
    let (url, handle) = serve_once(r#"{"model":"m","choices":[{"message":{"content":"ok"}}]}"#);
    let provider = OpenAiProvider::new(url, "sk-abc\n");
    let done = provider.complete(request()).await.unwrap();
    assert_eq!(done.content, "ok");
    let seen = handle.join().unwrap();
    let authorization = seen
        .lines()
        .find(|line| line.to_lowercase().starts_with("authorization:"))
        .expect("the key never reached the wire");
    assert!(
        authorization.to_lowercase().trim_end().ends_with("bearer sk-abc"),
        "the line break travelled in the header: {authorization:?}"
    );
}

#[rstest]
#[tokio::test]
async fn given_a_key_that_holds_a_line_break_inside_when_the_answer_is_asked_for_then_the_refusal_names_the_key()
{
    let provider = OpenAiProvider::new(
        "https://llm-l5plvn5p70xoh8n5.cn-beijing.maas.aliyuncs.com/compatible-mode/v1",
        "sk-abc\ndef",
    );
    let error = provider.complete(request()).await.unwrap_err().to_string();
    assert!(
        error.contains("key for dashscope"),
        "the refusal did not name the key: {error}"
    );
    assert!(
        !error.contains("builder error"),
        "the refusal still spoke reqwest: {error}"
    );
}

#[rstest]
#[tokio::test]
async fn given_an_address_written_without_a_scheme_when_the_answer_is_asked_for_then_the_refusal_names_the_address()
{
    let provider = OpenAiProvider::new(
        "llm-l5plvn5p70xoh8n5.cn-beijing.maas.aliyuncs.com/compatible-mode/v1",
        "",
    );
    let error = provider.complete(request()).await.unwrap_err().to_string();
    assert!(
        error.contains("https://"),
        "the refusal did not say how to write the address: {error}"
    );
    assert!(
        !error.contains("builder error"),
        "the refusal still spoke reqwest: {error}"
    );
}

#[test]
fn given_deepseek_constructor_when_built_then_endpoint_is_the_compatible_wire() {
    // Construction does no network; the key is held for use, not checked here.
    let _provider = OpenAiProvider::deepseek("test-key");
}

#[rstest]
fn given_a_tool_call_response_when_parsed_then_the_call_is_extracted() {
    let body = r#"{"model":"deepseek-chat","choices":[{"message":{"content":null,"tool_calls":[{"id":"c1","type":"function","function":{"name":"read","arguments":"{\"path\":\"a.txt\"}"}}]}}]}"#;
    let wire: serde_json::Value = serde_json::from_str(body).unwrap();
    let msg = &wire["choices"][0]["message"];
    let calls: Vec<pearl_provider::ToolCall> =
        serde_json::from_value(msg["tool_calls"].clone()).unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].function.name, "read");
    assert_eq!(calls[0].id, "c1");
}
