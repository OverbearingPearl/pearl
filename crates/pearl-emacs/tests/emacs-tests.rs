//! Stories of Emacs's keys: the motions, the kills, the mark and the undo,
//! told through the same interface a host drives.

use pearl_emacs::{Emacs, map};
use pearl_text::{Buffer, Code, Command, Encoding, Key, Outcome};
use rstest::rstest;

fn press(emacs: &mut Emacs, buffer: &mut Buffer, key: Key) -> Outcome {
    emacs.handle(key, buffer, &map())
}

#[rstest]
fn given_words_on_two_lines_when_control_k_arrives_then_the_rest_of_the_line_is_killed() {
    let mut emacs = Emacs::new();
    let mut buffer = Buffer::new("one two\nthree");
    buffer.set_caret(4);
    assert_eq!(
        press(&mut emacs, &mut buffer, Key::ch('k').ctrl()),
        Outcome::Handled
    );
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
    assert_eq!(
        press(&mut emacs, &mut buffer, Key::ch('w').ctrl()),
        Outcome::Handled
    );
    assert_eq!(buffer.text(), "one  three");
    assert_eq!(emacs.killed(), "two");
}

#[rstest]
fn given_a_letter_when_it_is_pressed_then_it_is_text_and_not_a_command() {
    let mut emacs = Emacs::new();
    let mut buffer = Buffer::new("");
    assert_eq!(
        press(&mut emacs, &mut buffer, Key::ch('a')),
        Outcome::Handled
    );
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
    assert_eq!(
        emacs.handle(Key::enter(), &mut buffer, &map),
        Outcome::Host(Command::Submit)
    );
    assert_eq!(
        buffer.text(),
        "write a parser",
        "the editor edited what it was told to hand over"
    );
}

#[rstest]
fn given_the_default_table_when_a_key_is_unbound_then_the_editor_ignores_it() {
    let mut emacs = Emacs::new();
    let mut buffer = Buffer::new("x");
    assert_eq!(
        press(&mut emacs, &mut buffer, Key::new(Code::Tab)),
        Outcome::Ignored
    );
}

#[rstest]
fn given_a_line_with_words_when_control_k_arrives_twice_then_the_ring_holds_both_lines() {
    let mut emacs = Emacs::new();
    let mut buffer = Buffer::new("one\ntwo\nthree");
    buffer.first();
    emacs.handle(Key::ch('k').ctrl(), &mut buffer, &map());
    assert_eq!(
        buffer.text(),
        "\ntwo\nthree",
        "the first C-k did not kill the line"
    );
    assert_eq!(emacs.killed(), "one");
    emacs.handle(Key::ch('k').ctrl(), &mut buffer, &map());
    assert_eq!(
        buffer.text(),
        "two\nthree",
        "the second C-k did not take the break"
    );
    assert_eq!(
        emacs.killed(),
        "one\n",
        "the ring was replaced instead of added to"
    );
    emacs.handle(Key::ch('y').ctrl(), &mut buffer, &map());
    assert_eq!(
        buffer.text(),
        "one\ntwo\nthree",
        "C-y pasted only the second kill"
    );
}

#[rstest]
fn given_a_word_when_meta_d_arrives_then_the_word_is_killed_and_meta_backspace_puts_it_back() {
    let mut emacs = Emacs::new();
    let mut buffer = Buffer::new("one two");
    buffer.first();
    emacs.handle(Key::ch('d').alt(), &mut buffer, &map());
    assert_eq!(
        buffer.text(),
        " two",
        "M-d did not kill the word under the caret"
    );
    buffer.last();
    emacs.handle(Key::new(Code::Backspace).alt(), &mut buffer, &map());
    assert_eq!(
        buffer.text(),
        " ",
        "M-Backspace did not kill the word behind"
    );
}

#[rstest]
fn given_two_letters_when_control_t_arrives_then_they_swap_and_the_caret_steps_over_both() {
    let mut emacs = Emacs::new();
    let mut buffer = Buffer::new("ab");
    buffer.first();
    buffer.right();
    emacs.handle(Key::ch('t').ctrl(), &mut buffer, &map());
    assert_eq!(buffer.text(), "ba", "C-t did not swap the two characters");
    assert_eq!(
        buffer.caret(),
        2,
        "the caret did not step past both characters"
    );
}

#[rstest]
fn given_a_line_when_control_o_arrives_then_a_break_opens_and_the_caret_stays() {
    let mut emacs = Emacs::new();
    let mut buffer = Buffer::new("ab");
    buffer.first();
    buffer.right();
    emacs.handle(Key::ch('o').ctrl(), &mut buffer, &map());
    assert_eq!(buffer.text(), "a\nb", "C-o did not open a line");
    assert_eq!(buffer.caret(), 1, "C-o moved the caret onto the new line");
}

#[rstest]
fn given_a_paragraph_when_meta_angles_arrive_then_the_ends_are_reached() {
    let mut emacs = Emacs::new();
    let mut buffer = Buffer::new("one\ntwo\nthree");
    buffer.first();
    emacs.handle(Key::ch('>').alt(), &mut buffer, &map());
    assert_eq!(buffer.caret(), 13, "M-> did not reach the end");
    emacs.handle(Key::ch('<').alt(), &mut buffer, &map());
    assert_eq!(buffer.caret(), 0, "M-< did not reach the start");
}

#[rstest]
fn given_emacs_keys_when_the_terminal_is_plain_then_control_space_is_reported() {
    let missing = pearl_emacs::unreachable(Encoding::Plain);
    let keys: Vec<String> = missing
        .iter()
        .map(|entry| format!("{:?}", entry.key))
        .collect();
    assert!(
        keys.iter().any(|key| key.contains("Char(' ')")),
        "C-Space is not reported: {keys:?}"
    );
    assert!(
        pearl_emacs::unreachable(Encoding::Enhanced).is_empty(),
        "an enhanced terminal still had keys reported"
    );
}

#[rstest]
fn given_the_emacs_help_when_it_is_read_then_it_names_the_keys_this_editor_answers() {
    let rows = pearl_emacs::help();
    let keys: Vec<&str> = rows.iter().map(|(keys, _)| *keys).collect();
    assert!(
        keys.contains(&"C-k|C-y"),
        "the kill ring is not in the help: {keys:?}"
    );
    assert!(
        keys.contains(&"C-Space"),
        "the mark is not in the help: {keys:?}"
    );
    assert!(
        rows.iter().all(|(_, said)| !said.is_empty()),
        "a row has no sentence"
    );
}
