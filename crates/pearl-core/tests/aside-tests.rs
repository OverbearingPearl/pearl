//! Stories for the aside: a side question with a short, strict life.

use pearl_core::{Aside, AsideError, AsideState};
use rstest::rstest;

#[rstest]
fn given_a_fresh_aside_when_answered_then_settled_with_the_text() {
    let mut aside = Aside::new("why does the borrow checker complain?");
    assert_eq!(aside.state, AsideState::Asking);
    assert!(!aside.is_settled());
    aside.answer("because the value moved").unwrap();
    assert_eq!(aside.state, AsideState::Answered);
    assert_eq!(aside.answer.as_deref(), Some("because the value moved"));
    assert!(aside.is_settled());
}

#[rstest]
fn given_an_answered_aside_when_answered_again_then_rejected() {
    let mut aside = Aside::new("q");
    aside.answer("first").unwrap();
    let err = aside.answer("second").unwrap_err();
    assert_eq!(err, AsideError::NotAsking(AsideState::Answered));
    assert_eq!(aside.answer.as_deref(), Some("first"));
}

#[rstest]
fn given_a_fresh_aside_when_failed_then_settled_with_the_reason() {
    let mut aside = Aside::new("q");
    aside.fail("provider unreachable").unwrap();
    assert_eq!(aside.state, AsideState::Failed);
    assert!(aside.is_settled());
    assert_eq!(aside.answer.as_deref(), Some("provider unreachable"));
}
