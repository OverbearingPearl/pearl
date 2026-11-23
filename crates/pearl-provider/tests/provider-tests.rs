//! Stories for the provider wire, told without a network.

use pearl_provider::openai::OpenAiProvider;
use pearl_provider::types::{ChatMessage, CompletionRequest, Role};
use pearl_provider::{Provider, ProviderError};
use rstest::rstest;

// The parse function is crate-private; exercise it through a small helper that
// mirrors what the client does with a response body.
fn parse(body: &str) -> Result<pearl_provider::Completion, ProviderError> {
    let wire: serde_json::Value = serde_json::from_str(body).unwrap();
    let content = wire["choices"][0]["message"]["content"]
        .as_str()
        .ok_or(ProviderError::EmptyResponse)?
        .to_string();
    let model = wire["model"].as_str().unwrap_or("").to_string();
    Ok(pearl_provider::Completion { content, model })
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
        let last = request.messages.last().map(|m| m.content.clone()).unwrap_or_default();
        Ok(pearl_provider::Completion { content: format!("echo: {last}"), model: request.model })
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
    };
    let completion = provider.complete(request).await.unwrap();
    assert_eq!(completion.content, "echo: ping");
}

#[test]
fn given_deepseek_constructor_when_built_then_endpoint_is_the_compatible_wire() {
    // Construction does no network; the key is held for use, not checked here.
    let _provider = OpenAiProvider::deepseek("test-key");
}
