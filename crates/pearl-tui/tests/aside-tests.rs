//! Stories for the aside on the surface: a side question shown, never queued.

use crossterm::event::KeyCode;
use pearl_core::{AsideState, Queue};
use pearl_tui::{key, App};
use rstest::{fixture, rstest};

/// Given a surface with one queued task focused.
#[fixture]
fn surface() -> App {
    let mut q = Queue::new();
    q.append("the real work");
    App::new(q)
}

fn type_text(app: &mut App, text: &str) {
    for c in text.chars() {
        app.handle_key(key(KeyCode::Char(c)));
    }
    app.handle_key(key(KeyCode::Enter));
}

#[rstest]
fn given_any_style_when_the_aside_key_is_pressed_then_the_prompt_asks(mut surface: App) {
    for style in [pearl_core::Keys::Plain, pearl_core::Keys::Vim, pearl_core::Keys::Emacs] {
        surface = App::new(surface.queue.clone()).with_settings({
            let s = pearl_core::Settings::new("m");
            s.apply("keys vim").ok();
            s
        });
        surface.handle_key(key(KeyCode::Char('/')));
        assert!(
            surface.prompt_line().starts_with("btw"),
            "{style:?}: / did not open the aside: {}",
            surface.prompt_line()
        );
        surface.handle_key(key(KeyCode::Esc));
    }
}

#[rstest]
fn given_normal_when_btw_typed_then_an_aside_asks_and_no_node_appears(mut surface: App) {
    let nodes_before = surface.queue.len();
    surface.handle_key(key(KeyCode::Char('/')));
    type_text(&mut surface, "why does it borrow?");
    let aside = surface.aside.as_ref().expect("an aside is held");
    assert_eq!(aside.question, "why does it borrow?");
    assert_eq!(aside.state, AsideState::Asking);
    // The queue gained nothing: the aside is not work.
    assert_eq!(surface.queue.len(), nodes_before);
    assert!(surface.aside_line().unwrap().contains("(asking...)"));
}

#[rstest]
fn given_an_asking_aside_when_answered_then_the_line_shows_the_answer(mut surface: App) {
    surface.handle_key(key(KeyCode::Char('/')));
    type_text(&mut surface, "why?");
    surface.aside.as_mut().unwrap().answer("because it moved").unwrap();
    let line = surface.aside_line().unwrap();
    assert!(line.contains("why?"));
    assert!(line.contains("because it moved"));
}

#[rstest]
fn given_an_aside_when_escape_then_it_is_dismissed(mut surface: App) {
    surface.handle_key(key(KeyCode::Char('/')));
    type_text(&mut surface, "why?");
    assert!(surface.aside.is_some());
    surface.handle_key(key(KeyCode::Esc));
    assert!(surface.aside.is_none());
    assert!(surface.aside_line().is_none());
}
