//! Stories for the candidate list: the models the human may choose between.
//!
//! A candidate is an endpoint and a model id. The house is read off the
//! endpoint rather than typed, because a second field that has to agree with
//! the first is a field that will disagree with it. The key is not here at
//! all: it belongs to whoever runs the machine, and pearl only looks for it.

use pearl_core::catalog::{
    complete_endpoint, normalize_endpoint, protocol_complaint, wire_url,
};
use pearl_core::{Candidate, Settings};
use rstest::rstest;

#[rstest]
#[case(
    "https://api.deepseek.com/v1",
    "deepseek-chat",
    "deepseek/deepseek-chat"
)]
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
    assert!(
        said.contains("localhost:11434/qwen3"),
        "the reply did not name what was added: {said}"
    );
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
    assert_eq!(
        s.model(),
        "deepseek-chat",
        "removing the chosen row left nothing chosen"
    );
}

#[rstest]
fn given_the_last_candidate_when_it_is_removed_then_the_refusal_says_why() {
    let s = Settings::new("deepseek-chat");
    let err = s.apply("remove 1").unwrap_err();
    assert!(
        err.contains("last"),
        "a refusal that does not say why: {err}"
    );
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
    assert_eq!(
        s.model(),
        "local-b",
        "a house with no table could not be cycled"
    );
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
    assert_eq!(
        back.candidates().len(),
        2,
        "the list did not survive the file"
    );
    assert_eq!(back.model(), "qwen3", "the choice did not survive the file");
    assert_eq!(back.endpoint(), "http://localhost:11434/v1");
}

#[rstest]
fn given_a_half_typed_candidate_line_when_it_is_applied_then_nothing_changes() {
    let s = Settings::new("deepseek-chat");
    assert!(s.apply("add http://localhost:11434/v1").is_err());
    assert!(s.apply("edit 1 https://api.openai.com/v1").is_err());
    assert_eq!(
        s.candidates().len(),
        1,
        "a typo left half a candidate behind"
    );
}

#[rstest]
fn given_a_list_when_it_is_asked_for_then_every_row_is_numbered_and_named() {
    let s = Settings::new("deepseek-chat");
    s.apply("add http://localhost:11434/v1 qwen3").unwrap();
    let said = s.apply("models").unwrap();
    assert!(
        said.contains("1 deepseek-chat"),
        "no numbered first row: {said}"
    );
    assert!(
        said.contains("2 localhost:11434/qwen3"),
        "no numbered second row: {said}"
    );
}

#[rstest]
fn given_a_model_id_alone_when_it_is_added_then_it_lands_on_the_launch_endpoint() {
    let s = Settings::new("deepseek-chat");
    let said = s.apply("add qwen3").unwrap();
    // The human names the two things a request needs; the house comes from
    // where the request goes, and no endpoint means the one we launched on.
    assert_eq!(s.candidates().len(), 2, "the row was not added: {said}");
    assert_eq!(s.candidates()[1].endpoint, "");
    assert_eq!(s.candidates()[1].model, "qwen3");
}

#[rstest]
#[case("https://api.deepseek.com/v1/", "https://api.deepseek.com/v1")]
#[case(
    "https://api.deepseek.com/v1/chat/completions",
    "https://api.deepseek.com/v1"
)]
#[case(
    "  https://api.deepseek.com/v1//chat/completions/  ",
    "https://api.deepseek.com/v1"
)]
fn given_a_copied_endpoint_when_it_is_normalised_then_the_base_is_what_is_kept(
    #[case] typed: &str,
    #[case] kept: &str,
) {
    // The provider appends `/chat/completions` itself, so a base that already
    // carries one would ask for it twice. A human copies the whole URL out of
    // a quickstart, so this is a normalisation rather than a complaint.
    let c = Candidate::new(typed, "m");
    assert_eq!(c.endpoint, kept);
}

#[rstest]
#[case(
    "https://api.anthropic.com/v1/messages",
    "https://api.anthropic.com/v1"
)]
#[case(
    "https://generativelanguage.googleapis.com/v1beta/models/gemini:generateContent",
    "https://generativelanguage.googleapis.com/v1beta/openai"
)]
#[case("http://localhost:11434/api/chat", "http://localhost:11434/v1")]
#[case("https://api.openai.com/v1/completions", "https://api.openai.com/v1")]
fn given_another_apis_path_when_it_is_read_then_the_complaint_names_the_address_that_works(
    #[case] typed: &str,
    #[case] named: &str,
) {
    let why = pearl_core::catalog::protocol_complaint(typed)
        .unwrap_or_else(|| panic!("{typed} was taken for a compatible base"));
    assert!(
        why.contains(named),
        "the complaint did not name the address: {why}"
    );
}

#[rstest]
#[case("https://api.deepseek.com/v1")]
#[case("https://openrouter.ai/api/v1")]
#[case("")]
#[case("http://localhost:8955")]
fn given_a_compatible_base_when_it_is_read_then_there_is_nothing_to_complain_about(
    #[case] typed: &str,
) {
    assert_eq!(pearl_core::catalog::protocol_complaint(typed), None);
}

#[rstest]
fn given_an_address_without_a_scheme_when_a_row_is_added_then_the_host_decides_which_one() {
    // The same reading the surface gives a typed address, reached through the
    // grammar: a house is https and this machine is http.
    let s = Settings::new("m");
    s.apply("add openrouter.ai/api/v1 gpt-oss-20b:free")
        .expect("a scheme a host can supply was refused");
    assert_eq!(
        s.candidates().last().unwrap().endpoint,
        "https://openrouter.ai/api/v1"
    );
    s.apply("add localhost:8955/v1 local-model")
        .expect("this machine without a scheme was refused");
    assert_eq!(
        s.candidates().last().unwrap().endpoint,
        "http://localhost:8955/v1"
    );
}

#[rstest]
fn given_anthropics_messages_address_when_a_row_is_added_then_the_base_is_read_as_chat() {
    let s = Settings::new("m");
    s.apply("add https://api.anthropic.com/v1/messages claude-sonnet-4-5")
        .expect("the address was refused instead of read");
    let added = s.candidates().last().cloned().unwrap();
    assert_eq!(
        added.endpoint, "https://api.anthropic.com/v1",
        "the messages path was kept, so the provider would append a second path"
    );
}

#[rstest]
fn given_an_address_whose_first_label_is_not_the_house_when_its_house_is_read_then_it_is_named() {
    // The house is what the machine belongs to, and a few machines in common
    // use put a word there instead: `open` is Zhipu, `x` is xAI,
    // `generativelanguage` is Google, and `spark-api-open` is iFlytek's Spark.
    // Reading the first label off those files the key under the wrong name and
    // hands the request the wrong dialect.
    for (address, house) in [
        ("https://open.bigmodel.cn/api/paas/v4", "bigmodel"),
        ("https://api.x.ai/v1", "xai"),
        (
            "https://generativelanguage.googleapis.com/v1beta/openai",
            "gemini",
        ),
        ("https://api.openai.com/v1", "openai"),
        (
            "https://dashscope.aliyuncs.com/compatible-mode/v1",
            "dashscope",
        ),
        ("https://spark-api-open.xf-yun.com/v1", "spark"),
        // A region of a house is still that house, even though the list
        // offers only one address per product.
        (
            "https://dashscope-intl.aliyuncs.com/compatible-mode/v1",
            "dashscope",
        ),
        (
            "https://dashscope-us.aliyuncs.com/compatible-mode/v1",
            "dashscope",
        ),
    ] {
        assert_eq!(
            pearl_core::catalog::house_of(address).as_deref(),
            Some(house),
            "the house of {address} was read as something else"
        );
    }
    assert_eq!(
        pearl_core::catalog::house_of("http://127.0.0.1:8955/v1"),
        None
    );
}

#[rstest]
fn given_a_model_gateway_whose_first_label_is_a_deployment_when_its_house_is_read_then_the_vendor_is_named(
) {
    // A dedicated Bailian deployment puts its own id where the house normally
    // sits -- `llm-…`, `token-plan` -- and the vendor is the domain it lives
    // under. The dialect belongs to the gateway, not to the deployment, so
    // reading the deployment id would keep the wrong field on the wire and
    // file the key under a name no shell can set.
    for (address, house) in [
        (
            "https://llm-l5plvn5p70xoh8n5.cn-beijing.maas.aliyuncs.com/compatible-mode/v1",
            "dashscope",
        ),
        (
            "https://token-plan.cn-beijing.maas.aliyuncs.com/compatible-mode/v1",
            "dashscope",
        ),
        (
            "https://token-plan.maas.qwencloudapi.com/compatible-mode/v1",
            "dashscope",
        ),
        (
            "https://token-plan.maas.qianwenaiapi.com/compatible-mode/v1",
            "dashscope",
        ),
    ] {
        assert_eq!(
            pearl_core::catalog::house_of(address).as_deref(),
            Some(house),
            "the house of {address} was read as something else"
        );
    }
    // A suffix names the vendor's whole domain, so the match ends at a dot: a
    // host that merely shares the letters is somebody else, and a storage
    // bucket is not a model gateway.
    assert_eq!(
        pearl_core::catalog::house_of("https://notmaas.aliyuncs.com/v1").as_deref(),
        Some("notmaas")
    );
    assert_eq!(
        pearl_core::catalog::house_of("https://oss-cn-beijing.aliyuncs.com").as_deref(),
        Some("oss-cn-beijing")
    );
}

#[rstest]
fn given_the_published_bases_when_they_are_read_then_each_is_normalised_unique_and_speaks_the_api()
{
    // A published base is a promise that a row pointed at it will be answered,
    // so the list may not carry a spelling the rest of pearl would rewrite, a
    // duplicate under another spelling, or an address `protocol_complaint`
    // would refuse when the human typed it.
    let mut seen: Vec<String> = Vec::new();
    for base in pearl_core::catalog::WELL_KNOWN {
        assert_eq!(*base, normalize_endpoint(base), "{base} is not normalised");
        assert!(!base.is_empty(), "an empty base is on the list");
        assert!(
            protocol_complaint(base).is_none(),
            "{base} is an address this program would refuse"
        );
        assert!(
            !seen.contains(&base.to_string()),
            "{base} is on the list twice"
        );
        seen.push(base.to_string());
    }
    assert!(
        seen.len() >= 15,
        "the list forgot most of the houses: {}",
        seen.len()
    );
}

#[rstest]
fn given_a_host_the_user_already_holds_a_key_for_when_its_base_is_asked_for_then_the_vendor_path_is_kept()
{
    // The machine is a host in authinfo; the path is the vendor's, not the
    // user's to remember. A deployment under a vendor domain gets that
    // vendor's documented path; a host under no known house gets no guess.
    assert_eq!(
        pearl_core::catalog::base_for_host("llm-l5plvn5p70xoh8n5.cn-beijing.maas.aliyuncs.com")
            .as_deref(),
        Some("https://llm-l5plvn5p70xoh8n5.cn-beijing.maas.aliyuncs.com/compatible-mode/v1")
    );
    assert_eq!(
        pearl_core::catalog::base_for_host("dashscope.aliyuncs.com").as_deref(),
        Some("https://dashscope.aliyuncs.com/compatible-mode/v1")
    );
    assert_eq!(
        pearl_core::catalog::base_for_host("api.deepseek.com").as_deref(),
        Some("https://api.deepseek.com")
    );
    assert_eq!(
        pearl_core::catalog::base_for_host("oss-cn-beijing.aliyuncs.com"),
        None,
        "a file store was offered as a model endpoint"
    );
    assert_eq!(
        pearl_core::catalog::base_for_host("example.com"),
        None,
        "an address nobody published was completed by guesswork"
    );
}

#[rstest]
#[case(
    "https://openrouter.ai/api/v1",
    "https://openrouter.ai/api/v1/chat/completions"
)]
#[case(
    "https://openrouter.ai/api/v1/chat/completions",
    "https://openrouter.ai/api/v1/chat/completions"
)]
#[case("", "")]
fn given_a_base_when_the_wire_address_is_asked_for_then_the_one_path_the_provider_appends_is_shown(
    #[case] base: &str,
    #[case] want: &str,
) {
    assert_eq!(wire_url(base), want);
}

#[rstest]
fn given_a_whole_request_url_when_it_is_completed_then_the_base_is_kept_and_no_note_is_needed() {
    let done = complete_endpoint("https://openrouter.ai/api/v1/chat/completions").unwrap();
    assert_eq!(done.endpoint, "https://openrouter.ai/api/v1");
    assert_eq!(done.note, None);
}

#[rstest]
#[case("openrouter.ai/api/v1", "https://openrouter.ai/api/v1")]
#[case("localhost:11434/v1", "http://localhost:11434/v1")]
#[case("127.0.0.1:8955/v1", "http://127.0.0.1:8955/v1")]
#[case("192.168.1.20:8000/v1", "http://192.168.1.20:8000/v1")]
fn given_an_address_without_a_scheme_when_it_is_completed_then_the_host_decides_which_one(
    #[case] typed: &str,
    #[case] want: &str,
) {
    assert_eq!(complete_endpoint(typed).unwrap().endpoint, want);
}

#[rstest]
fn given_a_house_written_without_tls_when_it_is_completed_then_the_refusal_names_the_https_address() {
    let why = complete_endpoint("http://openrouter.ai/api/v1")
        .expect_err("a key would have crossed the network in the clear");
    assert!(
        why.contains("https://openrouter.ai/api/v1"),
        "the refusal did not name the address that works: {why}"
    );
}

#[rstest]
fn given_this_machine_written_with_tls_when_it_is_completed_then_the_human_s_scheme_is_kept() {
    assert_eq!(
        complete_endpoint("https://localhost:11434/v1")
            .unwrap()
            .endpoint,
        "https://localhost:11434/v1"
    );
}

#[rstest]
#[case("https://api.openai.com/v1/responses", "https://api.openai.com/v1")]
#[case("https://api.anthropic.com/v1/messages", "https://api.anthropic.com/v1")]
fn given_another_api_s_path_when_it_is_completed_then_it_is_read_as_chat_and_the_note_says_so(
    #[case] typed: &str,
    #[case] base: &str,
) {
    let done = complete_endpoint(typed).unwrap();
    assert_eq!(done.endpoint, base);
    let note = done.note.unwrap_or_default();
    assert!(
        note.contains("openai-chat") && note.contains("/chat/completions"),
        "the note does not say what pearl speaks: {note}"
    );
}
