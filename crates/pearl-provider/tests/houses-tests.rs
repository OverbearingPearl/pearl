//! Stories for choosing where a request goes, and who pays for it.
//!
//! The endpoint is part of the model choice, so it has to be read per request:
//! a client built at launch would send the new model's name to the old model's
//! server. A local endpoint that wants no key is a normal answer, not a reason
//! to refuse to run.

use std::io::{Read, Write};
use std::net::TcpListener;

use pearl_provider::types::{ChatMessage, CompletionRequest, Role};
use pearl_provider::{Houses, Provider};
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

#[rstest]
fn given_a_candidate_that_names_no_endpoint_when_it_is_chosen_then_the_launch_endpoint_stands() {
    let h = Houses::new("https://api.deepseek.com/v1", String::new);
    assert_eq!(h.endpoint(), "https://api.deepseek.com/v1");
}

#[rstest]
fn given_a_chosen_endpoint_when_the_choice_changes_then_the_next_request_goes_elsewhere() {
    let chosen = std::sync::Arc::new(std::sync::Mutex::new(String::from("http://a:1/v1")));
    let read = chosen.clone();
    let h = Houses::new("https://api.deepseek.com/v1", move || read.lock().unwrap().clone());
    assert_eq!(h.endpoint(), "http://a:1/v1");
    *chosen.lock().unwrap() = String::from("http://b:2/v1");
    assert_eq!(h.endpoint(), "http://b:2/v1", "the endpoint was decided once and never read again");
}

#[rstest]
fn given_an_endpoint_with_no_key_anywhere_when_it_is_asked_then_nothing_is_borrowed() {
    let h = Houses::new("http://127.0.0.1:11434/v1", String::new).with_authinfo(None);
    assert!(h.credential().is_none(), "a keyless endpoint was handed somebody else's key");
}

#[tokio::test]
async fn given_a_keyless_local_endpoint_when_a_request_is_sent_then_it_arrives_with_no_authorization() {
    let (url, handle) = serve_once(r#"{"model":"m","choices":[{"message":{"content":"ok"}}]}"#);
    let h = Houses::new(url, String::new).with_authinfo(None);
    let done = h.complete(request()).await.unwrap();
    assert_eq!(done.content, "ok");
    let seen = handle.join().unwrap();
    assert!(!seen.to_lowercase().contains("authorization"), "a keyless endpoint was sent a header: {seen}");
}

#[tokio::test]
async fn given_two_endpoints_when_the_choice_moves_then_the_request_follows_it() {
    let (url, handle) = serve_once(r#"{"model":"m","choices":[{"message":{"content":"second"}}]}"#);
    let h = Houses::new("http://127.0.0.1:1/v1", move || url.clone()).with_authinfo(None);
    let done = h.complete(request()).await.unwrap();
    assert_eq!(done.content, "second", "the request went to the launch endpoint instead");
    let _ = handle.join().unwrap();
}
