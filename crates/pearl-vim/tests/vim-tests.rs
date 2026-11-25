//! Stories of Vim's modes: what a key means depends on the mode, and the
//! operators take the line or the marked words.

use pearl_text::{Buffer, Command, Key, Outcome};
use pearl_vim::{Mode, Vim};
use rstest::rstest;

fn press(vim: &mut Vim, buffer: &mut Buffer, key: Key) -> Outcome {
    let map = vim.map();
    vim.handle(key, buffer, &map)
}

#[rstest]
fn given_a_draft_being_typed_when_escape_arrives_then_the_keys_are_back_in_normal_mode() {
    let mut vim = Vim::typing();
    let mut buffer = Buffer::new("one two");
    assert_eq!(press(&mut vim, &mut buffer, Key::escape()), Outcome::Handled);
    assert_eq!(vim.mode(), Mode::Normal);
    press(&mut vim, &mut buffer, Key::ch('0'));
    assert_eq!(buffer.caret(), 0, "0 did not move to the start of the line");
}

#[rstest]
fn given_normal_mode_when_dd_arrives_then_the_line_is_gone_and_in_the_register() {
    let mut vim = Vim::typing();
    let mut buffer = Buffer::new("first\nsecond\nthird");
    press(&mut vim, &mut buffer, Key::escape());
    buffer.set_caret(8); // inside "second"
    press(&mut vim, &mut buffer, Key::ch('d'));
    assert_eq!(vim.pending(), Some('d'), "d did not wait for its target");
    press(&mut vim, &mut buffer, Key::ch('d'));
    assert_eq!(buffer.text(), "first\nthird");
    assert_eq!(vim.register(), "second\n");
}

#[rstest]
fn given_marked_words_when_y_arrives_then_they_are_copied_and_the_mode_is_normal() {
    let mut vim = Vim::typing();
    let mut buffer = Buffer::new("one two three");
    press(&mut vim, &mut buffer, Key::escape());
    buffer.set_caret(4);
    press(&mut vim, &mut buffer, Key::ch('v'));
    assert_eq!(vim.mode(), Mode::Visual);
    press(&mut vim, &mut buffer, Key::ch('w'));
    press(&mut vim, &mut buffer, Key::ch('y'));
    assert_eq!(vim.register(), "two ");
    assert_eq!(buffer.text(), "one two three", "y took the words away");
    assert_eq!(vim.mode(), Mode::Normal);
}

#[rstest]
fn given_normal_mode_when_o_arrives_then_a_line_opens_below_and_insert_is_back() {
    let mut vim = Vim::typing();
    let mut buffer = Buffer::new("one");
    press(&mut vim, &mut buffer, Key::escape());
    press(&mut vim, &mut buffer, Key::ch('o'));
    assert_eq!(buffer.text(), "one\n");
    assert_eq!(vim.mode(), Mode::Insert);
}

#[rstest]
fn given_gg_when_it_arrives_then_the_caret_is_at_the_first_character() {
    let mut vim = Vim::typing();
    let mut buffer = Buffer::new("one\ntwo");
    press(&mut vim, &mut buffer, Key::escape());
    press(&mut vim, &mut buffer, Key::ch('g'));
    assert_eq!(vim.pending(), Some('g'));
    press(&mut vim, &mut buffer, Key::ch('g'));
    assert_eq!(buffer.caret(), 0);
}

#[rstest]
fn given_a_host_that_binds_enter_to_submit_when_normal_mode_reads_it_then_the_host_is_handed_it() {
    let mut vim = Vim::typing();
    let mut buffer = Buffer::new("write a parser");
    press(&mut vim, &mut buffer, Key::escape());
    let map = vim.map().bind(Key::enter(), Command::Submit);
    assert_eq!(
        vim.handle(Key::enter(), &mut buffer, &map),
        Outcome::Host(Command::Submit)
    );
    assert_eq!(buffer.text(), "write a parser");
}

#[rstest]
fn given_a_mode_when_its_table_is_read_then_it_names_that_modes_keys() {
    let typing = Vim::typing();
    assert_eq!(typing.map().on(Key::ch('h')), None, "Insert mode treats h as a letter");
    let normal = Vim::starting_in(Mode::Normal);
    assert_eq!(normal.map().on(Key::ch('h')), Some(Command::Left));
}
