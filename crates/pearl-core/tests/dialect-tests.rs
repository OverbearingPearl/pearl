//! Stories for what a house can say about thinking.
//!
//! One ladder for the human, a dialect per house on the wire. A rung the
//! endpoint cannot express is not offered: a setting that is silently ignored
//! is worse than one that is refused, because the human believes it took.

use pearl_core::{Dialect, Effort, Settings};
use rstest::rstest;

#[rstest]
fn given_the_ladder_when_it_is_read_then_it_names_every_intent_a_house_may_have() {
    for (word, want) in [
        ("off", Effort::Off),
        ("minimal", Effort::Minimal),
        ("low", Effort::Low),
        ("medium", Effort::Medium),
        ("high", Effort::High),
        ("max", Effort::Max),
    ] {
        assert_eq!(
            Effort::parse(word),
            Some(want),
            "{word} is not on the ladder"
        );
        assert_eq!(want.as_str(), word);
    }
}

#[rstest]
fn given_a_house_when_its_dialect_is_looked_up_then_it_speaks_its_own_fields() {
    assert_eq!(Dialect::for_house("deepseek"), Dialect::Silent);
    assert_eq!(Dialect::for_house("dashscope"), Dialect::Qwen);
    assert_eq!(Dialect::for_house("bigmodel"), Dialect::Glm);
    assert_eq!(Dialect::for_house("anthropic"), Dialect::Anthropic);
    assert_eq!(Dialect::for_house("openai"), Dialect::OpenAi);
    assert_eq!(Dialect::for_house("localhost"), Dialect::OpenAi);
}

#[rstest]
fn given_deepseek_when_its_rungs_are_read_then_there_are_none_because_the_model_decides() {
    assert!(Dialect::Silent.rungs().is_empty());
    assert!(Dialect::Silent.describe().contains("the model decides"));
}

#[rstest]
fn given_an_openai_dialect_when_max_is_asked_for_then_it_is_refused_rather_than_ignored() {
    let s = Settings::new("gpt-4o");
    let said = s.apply("effort max").unwrap_err();
    assert!(
        said.contains("max"),
        "the refusal does not name the rung: {said}"
    );
    assert!(s.effort().is_none(), "a refused rung was taken anyway");
}

#[rstest]
fn given_deepseek_when_the_effort_key_is_pressed_then_it_stays_auto_and_says_why() {
    let s = Settings::new("deepseek-reasoner");
    for _ in 0..4 {
        s.cycle_effort();
        assert!(
            s.effort().is_none(),
            "a house with no effort knob was given one"
        );
    }
}

#[rstest]
fn given_a_qwen_dialect_when_the_effort_key_walks_then_it_reaches_the_rungs_qwen_has() {
    let s = Settings::new("qwen3");
    s.apply("add https://dashscope.aliyuncs.com/compatible-mode/v1 qwen3")
        .unwrap();
    s.apply("choose 2").unwrap();
    let mut seen = vec![];
    for _ in 0..pearl_core::settings::Effort::ladder().len() {
        s.cycle_effort();
        seen.push(s.effort().map(|e| e.as_str()));
    }
    // A budget house takes a number for every rung, so the key walks the whole
    // ladder here: the same seven words as everywhere else.
    for rung in pearl_core::settings::Effort::ladder() {
        assert!(
            seen.contains(&Some(rung.as_str())),
            "qwen cannot reach {}: {seen:?}",
            rung.as_str()
        );
    }
}

#[rstest]
fn given_a_bailian_deployment_when_its_endpoint_is_chosen_then_thinking_speaks_qwen() {
    // The deployment's own id is the first label, but the thinking fields are
    // the gateway's: a person who points a row at a dedicated Bailian address
    // must not have to type `dialect qwen` to be understood.
    let s = Settings::new("qwen3.8-max");
    s.apply(
        "add https://llm-l5plvn5p70xoh8n5.cn-beijing.maas.aliyuncs.com/compatible-mode/v1 qwen3.8-max",
    )
    .unwrap();
    s.apply("choose 2").unwrap();
    assert_eq!(s.dialect(), Dialect::Qwen);
    s.apply("effort high")
        .expect("a qwen gateway was not offered a thinking budget");
}

#[rstest]
fn given_a_dialect_set_by_hand_when_it_is_read_then_it_overrides_the_house() {
    let s = Settings::new("m");
    s.apply("dialect qwen").unwrap();
    assert_eq!(s.dialect(), Dialect::Qwen);
    s.apply("effort max").unwrap();
    assert_eq!(s.effort(), Some(Effort::Max));
}

#[rstest]
fn given_a_house_that_refused_a_rung_when_it_is_asked_for_again_then_the_refusal_is_the_reason() {
    let s = Settings::new("m");
    s.apply("dialect openai").unwrap();
    s.refuse(Effort::High);

    let said = s
        .apply("effort high")
        .expect_err("a rung this house refused went into force");
    assert!(
        said.contains("refused high"),
        "the reason did not name the rung: {said}"
    );
    // The address is the one that answered, not the dialect the human stated:
    // `dialect openai` says what to speak, and `-` is the server that said no.
    assert!(
        said.contains("the launch endpoint refused high"),
        "the reason did not name the address: {said}"
    );
    assert_eq!(s.effort(), None, "the rung went into force anyway");
    // Marked, not removed: the ladder still lists it, so the surface can say
    // why the word the human wants is not the one in force.
    assert!(
        s.dialect().rungs().contains(&Effort::High),
        "the rung disappeared from the ladder"
    );
}

#[rstest]
fn given_a_rung_this_house_refused_when_effort_steps_then_it_is_stepped_over() {
    let s = Settings::new("m");
    s.apply("dialect openai").unwrap();
    s.refuse(Effort::Medium);

    let mut walked = Vec::new();
    for _ in 0..6 {
        s.cycle_effort();
        walked.push(s.effort().map(Effort::as_str).unwrap_or("auto"));
    }
    assert!(
        !walked.contains(&"medium"),
        "the key walked onto a refused rung: {walked:?}"
    );
    assert!(
        walked.contains(&"high"),
        "the key never reached a rung the house takes: {walked:?}"
    );
}

#[rstest]
fn given_two_houses_and_one_refusal_when_the_other_is_chosen_then_its_rungs_are_whole() {
    let s = Settings::new("m");
    s.apply("dialect openai").unwrap();
    s.apply("add https://api.qwen.com/v1 qwen3").unwrap();
    // The launch endpoint is the one that said no.
    assert_eq!(s.address(), "-");
    s.refuse(Effort::High);
    assert_eq!(
        s.refused(),
        vec![Effort::High],
        "the refusal was not filed at all"
    );

    s.apply("choose 2").unwrap();
    assert_eq!(s.address(), "https://api.qwen.com/v1");
    assert!(
        s.refused().is_empty(),
        "a refusal filed against one address leaked to another"
    );
    assert_eq!(
        s.apply("effort high").unwrap(),
        "effort high",
        "qwen takes a word deepseek refused"
    );

    s.apply("choose 1").unwrap();
    assert_eq!(
        s.refused(),
        vec![Effort::High],
        "the refusal was lost on the way back"
    );
}

#[rstest]
fn given_a_refusal_when_the_settings_are_saved_and_read_back_then_it_survives() {
    let dir = std::env::temp_dir().join(format!("pearl-refused-{}", std::process::id()));
    let _ = std::fs::remove_file(&dir);
    let s = Settings::new("m");
    s.apply("dialect openai").unwrap();
    s.refuse(Effort::Medium);
    s.save(&dir).unwrap();

    let text = std::fs::read_to_string(&dir).unwrap();
    assert!(
        text.contains("refused - medium"),
        "the file does not carry the refusal:\n{text}"
    );

    let read = Settings::new("m");
    read.load(&dir).unwrap();
    assert_eq!(
        read.refused(),
        vec![Effort::Medium],
        "the refusal did not come back"
    );
    let _ = std::fs::remove_file(&dir);
}

#[rstest]
fn given_refusals_when_they_are_cleared_then_the_rungs_come_back_into_force() {
    let s = Settings::new("m");
    s.apply("dialect openai").unwrap();
    s.refuse(Effort::High);
    assert!(s.apply("effort high").is_err());

    s.apply("refused clear").unwrap();
    assert_eq!(s.apply("effort high").unwrap(), "effort high");
}

#[rstest]
fn given_a_rung_the_endpoint_cannot_be_told_when_it_is_chosen_then_the_answer_says_what_it_can_hear()
 {
    // The refusal is read by someone who never saw a wire: it names the
    // setting the endpoint lacks, not the name we gave the spelling.
    let s = Settings::new("deepseek-chat");
    let said = s.apply("effort high").unwrap_err();
    assert!(
        said.contains("no thinking setting"),
        "the answer does not say why: {said}"
    );
    assert!(
        !said.contains("dialect"),
        "the answer talks about our own word for it: {said}"
    );
}
