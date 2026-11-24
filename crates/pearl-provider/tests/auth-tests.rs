//! Stories for where the key comes from.
//!
//! The environment says only "here is a key". An authinfo line names the
//! machine, so it can also say who the provider is -- and a key that belongs to
//! one house must not be sent to another.

use pearl_provider::auth::{self, Credential};
use rstest::rstest;
use tempfile::TempDir;

const FILE: &str = "\
# my keys
machine api.deepseek.com login apikey password sk-deep port 443
machine api.openai.com login apikey password sk-oai
machine localhost login user password local-secret port 11434
";

#[rstest]
fn given_an_authinfo_file_when_it_is_read_then_each_machine_yields_its_key() {
    let entries = auth::parse(FILE);
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].machine, "api.deepseek.com");
    assert_eq!(entries[0].password, "sk-deep");
    assert_eq!(entries[0].login.as_deref(), Some("apikey"));
}

#[rstest]
fn given_a_line_without_a_password_when_the_file_is_read_then_it_is_not_a_credential() {
    let entries = auth::parse("machine api.deepseek.com login apikey\n");
    assert!(entries.is_empty(), "half a credential was accepted");
}

#[rstest]
fn given_a_host_when_the_file_is_searched_then_its_own_key_is_found() {
    let entries = auth::parse(FILE);
    let found = auth::find(&entries, "api.openai.com").unwrap();
    assert_eq!(found, Credential { key: "sk-oai".into(), provider: "openai".into() });
}

#[rstest]
fn given_a_machine_written_without_the_api_prefix_when_the_host_has_it_then_it_still_matches() {
    let entries = auth::parse("machine deepseek.com login k password sk-x\n");
    assert!(auth::find(&entries, "api.deepseek.com").is_some());
}

#[rstest]
fn given_a_host_when_the_provider_is_named_then_the_house_is_named() {
    assert_eq!(auth::provider_name("api.deepseek.com"), "deepseek");
    assert_eq!(auth::provider_name("api.openai.com"), "openai");
    assert_eq!(auth::provider_name("localhost:11434"), "localhost");
}

#[rstest]
fn given_an_endpoint_url_when_the_host_is_taken_then_the_scheme_and_path_go() {
    assert_eq!(auth::host_of("https://api.deepseek.com/v1"), "api.deepseek.com");
    assert_eq!(auth::host_of("http://127.0.0.1:8931/v1"), "127.0.0.1");
}

#[rstest]
fn given_both_sources_when_the_key_is_resolved_then_the_environment_wins() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("authinfo");
    std::fs::write(&path, FILE).unwrap();
    let got = auth::resolve("api.deepseek.com", Some("sk-env".into()), &path).unwrap();
    assert_eq!(got.key, "sk-env", "a key set for this run must override the shared file");
}

#[rstest]
fn given_no_environment_key_when_the_key_is_resolved_then_authinfo_serves_it() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("authinfo");
    std::fs::write(&path, FILE).unwrap();
    let got = auth::resolve("api.deepseek.com", None, &path).unwrap();
    assert_eq!(got, Credential { key: "sk-deep".into(), provider: "deepseek".into() });
}

#[rstest]
fn given_neither_source_when_the_key_is_resolved_then_there_is_none() {
    let dir = TempDir::new().unwrap();
    assert!(auth::resolve("api.deepseek.com", None, &dir.path().join("absent")).is_none());
}
