//! Stories of the buffer: where the words are and where the caret stands.
//!
//! These are unit stories in the crate the words live in, because a caret is
//! a fact about the text and not about a screen: the surface's stories about
//! rows and hints are in `pearl-tui`.

use pearl_text::{Buffer, Code, Command, Encoding, Key, KeyMap, Outcome};
use rstest::rstest;

#[rstest]
fn given_a_buffer_when_a_character_is_typed_then_it_lands_at_the_caret() {
    let mut buffer = Buffer::new("hello world");
    buffer.set_caret(5);
    buffer.insert(",");
    assert_eq!(buffer.text(), "hello, world");
    assert_eq!(
        buffer.caret(),
        6,
        "the caret did not follow the character it typed"
    );
}

#[rstest]
fn given_chinese_words_when_the_caret_moves_then_it_counts_characters_not_bytes() {
    let mut buffer = Buffer::new("任务队列");
    buffer.set_caret(2);
    assert_eq!(buffer.caret(), 2);
    buffer.insert("的");
    assert_eq!(buffer.text(), "任务的队列");
    assert_eq!(buffer.caret(), 3);
    buffer.backspace();
    assert_eq!(
        buffer.text(),
        "任务队列",
        "a backspace ate half a character"
    );
    assert_eq!(buffer.caret(), 2);
}

#[rstest]
fn given_a_selection_when_text_is_typed_then_the_selection_is_replaced() {
    let mut buffer = Buffer::new("one two three");
    buffer.set_caret(4);
    buffer.toggle_mark();
    buffer.right();
    buffer.right();
    buffer.right();
    assert_eq!(
        buffer.selection(),
        Some((4, 7)),
        "the motion did not extend the mark"
    );
    buffer.insert("TWO");
    assert_eq!(buffer.text(), "one TWO three");
}

#[rstest]
fn given_a_buffer_with_two_lines_when_the_caret_moves_up_then_it_keeps_its_column() {
    let mut buffer = Buffer::new("first line\nsecond");
    buffer.set_caret(13); // "second": its third character
    buffer.up();
    assert_eq!(buffer.caret(), 2, "the caret lost its column going up");
}

#[rstest]
fn given_an_edit_when_it_is_undone_then_the_words_and_the_caret_come_back() {
    let mut buffer = Buffer::new("one");
    buffer.insert(" and two");
    buffer.undo();
    assert_eq!(buffer.text(), "one");
    assert_eq!(
        buffer.caret(),
        3,
        "undo left the caret where the edit had put it"
    );
}

#[rstest]
fn given_a_line_when_it_is_killed_then_the_words_come_back_and_the_line_is_gone() {
    let mut buffer = Buffer::new("keep this\ntake this\nand this");
    buffer.set_caret(10);
    let killed = buffer.kill_to_end();
    assert_eq!(killed, "take this");
    assert_eq!(buffer.text(), "keep this\n\nand this");
}

#[rstest]
fn given_a_table_when_a_key_is_bound_twice_then_the_later_binding_wins() {
    let map = KeyMap::new()
        .bind(Key::enter(), Command::Break)
        .bind(Key::enter(), Command::Submit);
    assert_eq!(map.on(Key::enter()), Some(Command::Submit));
}

#[rstest]
fn given_a_plain_buffer_when_an_arrow_arrives_then_the_caret_moves_one_character() {
    let mut map = pearl_text::plain();
    let mut buffer = Buffer::new("ab");
    let mut killed = String::new();
    let outcome = map
        .on(Key::new(Code::Left))
        .map(|command| pearl_text::execute(command, &mut buffer, &mut killed).expect("a command"));
    assert_eq!(outcome, Some(Outcome::Handled));
    assert_eq!(buffer.caret(), 1);
    map = map.bind(Key::new(Code::Left), Command::Cancel);
    assert_eq!(map.on(Key::new(Code::Left)), Some(Command::Cancel));
}

#[rstest]
fn given_a_table_with_control_j_when_it_is_read_on_a_plain_terminal_then_the_key_is_reported() {
    let map = KeyMap::new().bind(Key::ch('j').ctrl(), Command::Break);
    let missing = map.unreachable(Encoding::Plain);
    assert_eq!(missing.len(), 1, "C-j was not reported: {missing:?}");
    assert_eq!(missing[0].key, Key::ch('j').ctrl());
    assert!(
        missing[0].why.contains("⏎"),
        "the reason does not say what arrives: {}",
        missing[0].why
    );
    assert!(
        map.unreachable(Encoding::Enhanced).is_empty(),
        "a terminal that can tell C-j apart still had it reported"
    );
}

#[rstest]
fn given_a_table_with_meta_and_shifted_keys_when_it_is_read_then_only_the_unreachable_ones_are_reported()
 {
    let map = KeyMap::new()
        .bind(Key::ch('f').alt(), Command::Right)
        .bind(Key::enter().shift(), Command::Break)
        .bind(Key::tab(), Command::Cancel);
    let missing = map.unreachable(Encoding::Plain);
    assert!(
        missing.is_empty(),
        "reachable keys were reported: {missing:?}"
    );
}

#[rstest]
fn given_the_shared_help_when_it_is_read_then_it_names_the_keys_every_style_answers() {
    let rows = pearl_text::help();
    let keys: Vec<&str> = rows.iter().map(|(keys, _)| *keys).collect();
    assert!(
        keys.contains(&"⏎"),
        "handing the words in is not in the help: {keys:?}"
    );
    assert!(
        keys.contains(&"Tab|␛"),
        "leaving the draft is not in the help: {keys:?}"
    );
    assert!(
        rows.iter().all(|(_, said)| !said.is_empty()),
        "a row has no sentence"
    );
}
