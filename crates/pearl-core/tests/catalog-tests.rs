//! Stories for the candidate list: the models the human may choose between.
//!
//! A candidate is an endpoint and a model id. The house is read off the
//! endpoint rather than typed, because a second field that has to agree with
//! the first is a field that will disagree with it. The key is not here at
//! all: it belongs to whoever runs the machine, and pearl only looks for it.

use pearl_core::{Candidate, Settings};
use rstest::rstest;

#[rstest]
#[case("https://api.deepseek.com/v1", "deepseek-chat", "deepseek/deepseek-chat")]
#[case("https://api.openai.com/v1", "gpt-4o", "openai/gpt-4o")]
#[case("http://localhost:11434/v1", "qwen3", "localhost:11434/qwen3")]
fn given_an_endpoint_and_a_model_when_the_candidate_is_named_then_the_house_is_read_off_the_endpoint(
    #[case] endpoint: &str,
    #[case] model: &str,
    #[case] want: &str,
) {
    assert_eq!(Candidate::new(endpoint, model).label(), want);
}

#[rstest]
fn given_two_local_endpoints_when_they_are_named_then_the_names_tell_them_apart() {
    let ollama = Candidate::new("http://localhost:11434/v1", "qwen3");
    let llama = Candidate::new("http://localhost:8080/v1", "qwen3");
    assert_ne!(ollama.label(), llama.label(), "two servers became one row");
}

#[rstest]
fn given_a_typed_line_when_a_candidate_is_added_then_it_is_on_the_list() {
    let s = Settings::new("deepseek-chat");
    let said = s.apply("add http://localhost:11434/v1 qwen3").unwrap();
    assert!(said.contains("localhost:11434/qwen3"), "the reply did not name what was added: {said}");
    assert_eq!(s.candidates().len(), 2);
    assert_eq!(s.candidates()[1].model, "qwen3");
}

#[rstest]
fn given_a_candidate_on_the_list_when_it_is_removed_then_the_choice_stays_on_something() {
    let s = Settings::new("deepseek-chat");
    s.apply("add https://api.openai.com/v1 gpt-4o").unwrap();
    s.apply("choose 2").unwrap();
    assert_eq!(s.model(), "gpt-4o");
    s.apply("remove 2").unwrap();
    assert_eq!(s.candidates().len(), 1);
    assert_eq!(s.model(), "deepseek-chat", "removing the chosen row left nothing chosen");
}

#[rstest]
fn given_the_last_candidate_when_it_is_removed_then_the_refusal_says_why() {
    let s = Settings::new("deepseek-chat");
    let err = s.apply("remove 1").unwrap_err();
    assert!(err.contains("last"), "a refusal that does not say why: {err}");
    assert_eq!(s.candidates().len(), 1);
}

#[rstest]
fn given_a_candidate_when_its_endpoint_is_edited_then_the_house_follows_it() {
    let s = Settings::new("deepseek-chat");
    s.apply("edit 1 https://api.openai.com/v1 gpt-4o").unwrap();
    assert_eq!(s.candidates()[0].label(), "openai/gpt-4o");
    assert_eq!(s.model(), "gpt-4o");
}

#[rstest]
fn given_an_unknown_house_when_candidates_are_typed_then_the_cycle_key_walks_them() {
    let s = Settings::new("deepseek-chat");
    s.apply("add http://192.168.1.9:8000/v1 local-a").unwrap();
    s.apply("add http://192.168.1.9:8000/v1 local-b").unwrap();
    s.apply("choose 2").unwrap();
    s.cycle_model();
    assert_eq!(s.model(), "local-b", "a house with no table could not be cycled");
    s.cycle_model();
    assert_eq!(s.model(), "deepseek-chat");
}

#[rstest]
fn given_a_chosen_candidate_when_its_endpoint_is_asked_for_then_it_is_the_one_chosen() {
    let s = Settings::new("deepseek-chat");
    s.apply("add http://localhost:11434/v1 qwen3").unwrap();
    s.apply("choose 2").unwrap();
    assert_eq!(s.endpoint(), "http://localhost:11434/v1");
}

#[rstest]
fn given_a_list_of_candidates_when_it_is_saved_and_loaded_then_the_list_and_the_choice_survive() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config");
    let s = Settings::new("deepseek-chat");
    s.apply("add http://localhost:11434/v1 qwen3").unwrap();
    s.apply("choose 2").unwrap();
    s.save(&path).unwrap();

    let back = Settings::new("something-else");
    back.load(&path).unwrap();
    assert_eq!(back.candidates().len(), 2, "the list did not survive the file");
    assert_eq!(back.model(), "qwen3", "the choice did not survive the file");
    assert_eq!(back.endpoint(), "http://localhost:11434/v1");
}

#[rstest]
fn given_a_half_typed_candidate_line_when_it_is_applied_then_nothing_changes() {
    let s = Settings::new("deepseek-chat");
    assert!(s.apply("add http://localhost:11434/v1").is_err());
    assert!(s.apply("edit 1 https://api.openai.com/v1").is_err());
    assert_eq!(s.candidates().len(), 1, "a typo left half a candidate behind");
}

#[rstest]
fn given_a_list_when_it_is_asked_for_then_every_row_is_numbered_and_named() {
    let s = Settings::new("deepseek-chat");
    s.apply("add http://localhost:11434/v1 qwen3").unwrap();
    let said = s.apply("models").unwrap();
    assert!(said.contains("1 deepseek-chat"), "no numbered first row: {said}");
    assert!(said.contains("2 localhost:11434/qwen3"), "no numbered second row: {said}");
}
