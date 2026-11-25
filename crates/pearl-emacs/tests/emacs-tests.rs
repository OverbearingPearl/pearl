//! Stories of Emacs's keys: the motions, the kills, the mark and the undo,
//! told through the same interface a host drives.

use pearl_emacs::{Emacs, map};
use pearl_text::{Buffer, Code, Command, Key, Outcome};
use rstest::rstest;

fn press(emacs: &mut Emacs, buffer: &mut Buffer, key: Key) -> Outcome {
    emacs.handle(key, buffer, &map())
}

#[rstest]
fn given_words_on_two_lines_when_control_k_arrives_then_the_rest_of_the_line_is_killed() {
    let mut emacs = Emacs::new();
    let mut buffer = Buffer::new("one two\nthree");
    buffer.set_caret(4);
    assert_eq!(press(&mut emacs, &mut buffer, Key::ch('k').ctrl()), Outcome::Handled);
    assert_eq!(buffer.text(), "one \nthree");
    assert_eq!(emacs.killed(), "two");
}

#[rstest]
fn given_words_when_control_y_arrives_then_the_killed_words_come_back() {
    let mut emacs = Emacs::new();
    let mut buffer = Buffer::new("one two");
    buffer.set_caret(4);
    press(&mut emacs, &mut buffer, Key::ch('k').ctrl());
    press(&mut emacs, &mut buffer, Key::ch('y').ctrl());
    assert_eq!(buffer.text(), "one two");
}

#[rstest]
fn given_a_mark_planted_when_control_w_arrives_then_the_marked_words_are_killed() {
    let mut emacs = Emacs::new();
    let mut buffer = Buffer::new("one two three");
    buffer.set_caret(4);
    press(&mut emacs, &mut buffer, Key::ch(' ').ctrl());
    press(&mut emacs, &mut buffer, Key::ch('f').ctrl());
    press(&mut emacs, &mut buffer, Key::ch('f').ctrl());
    press(&mut emacs, &mut buffer, Key::ch('f').ctrl());
    assert_eq!(press(&mut emacs, &mut buffer, Key::ch('w').ctrl()), Outcome::Handled);
    assert_eq!(buffer.text(), "one  three");
    assert_eq!(emacs.killed(), "two");
}

#[rstest]
fn given_a_letter_when_it_is_pressed_then_it_is_text_and_not_a_command() {
    let mut emacs = Emacs::new();
    let mut buffer = Buffer::new("");
    assert_eq!(press(&mut emacs, &mut buffer, Key::ch('a')), Outcome::Handled);
    assert_eq!(buffer.text(), "a");
}

#[rstest]
fn given_alt_f_when_it_is_pressed_then_the_caret_moves_a_word() {
    let mut emacs = Emacs::new();
    let mut buffer = Buffer::new("one two");
    buffer.set_caret(0);
    press(&mut emacs, &mut buffer, Key::ch('f').alt());
    assert_eq!(buffer.caret(), 4);
}

#[rstest]
fn given_a_host_that_binds_enter_to_submit_when_it_is_pressed_then_the_host_is_handed_it() {
    // This is the whole reason the table is the host's: PEARL hands a task in
    // with `⏎`, and the editor must not turn that into a new line.
    let mut emacs = Emacs::new();
    let mut buffer = Buffer::new("write a parser");
    let map = map().bind(Key::enter(), Command::Submit);
    assert_eq!(emacs.handle(Key::enter(), &mut buffer, &map), Outcome::Host(Command::Submit));
    assert_eq!(buffer.text(), "write a parser", "the editor edited what it was told to hand over");
}

#[rstest]
fn given_the_default_table_when_a_key_is_unbound_then_the_editor_ignores_it() {
    let mut emacs = Emacs::new();
    let mut buffer = Buffer::new("x");
    assert_eq!(press(&mut emacs, &mut buffer, Key::new(Code::Tab)), Outcome::Ignored);
}
