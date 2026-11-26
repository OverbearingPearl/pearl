//! Stories for where the screen's space goes.
//!
//! Two panes in boxes spend four columns and four rows on furniture, and a
//! list that grows past its pane simply loses its tail -- including the task
//! the cursor is on. The space belongs to what the human is looking at.

use pearl_core::Queue;
use crossterm::event::KeyCode;
use pearl_tui::{key, screen, App, Panes};
use rstest::rstest;

/// The rendered text without whitespace: a wide character takes two cells and
/// leaves the second blank, which is invisible but breaks every substring.
fn squash(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}

fn app_with(tasks: usize) -> App {
    let queue = Queue::new();
    for i in 0..tasks {
        let _ = queue.append(format!("task number {i}"));
    }
    let mut app = App::new(queue);
    for _ in 1..tasks {
        app.handle_key(key(KeyCode::Char('j')));
    }
    app
}

#[rstest]
fn given_any_surface_when_it_is_drawn_then_no_vertical_rules_steal_columns() {
    let app = app_with(3);
    let screen = screen(&app, 60, 20);
    assert!(!screen.contains('│'), "the panes are still boxed:\n{screen}");
    assert!(!screen.contains('┌'), "the panes are still boxed:\n{screen}");
}

#[rstest]
fn given_more_tasks_than_rows_when_the_cursor_is_on_the_last_then_it_is_on_screen() {
    let app = app_with(40);
    let focused = app.queue.focus().and_then(|id| app.queue.get(id)).unwrap();
    let screen = screen(&app, 60, 20);
    assert!(screen.contains(&focused.title), "the focused task scrolled out of sight:\n{screen}");
}

#[rstest]
fn given_more_tasks_than_rows_when_the_list_is_shown_then_it_says_how_many_there_are() {
    let app = app_with(40);
    let screen = screen(&app, 60, 20);
    assert!(screen.contains("40"), "the list hid how much work there is:\n{screen}");
}

#[rstest]
fn given_a_long_report_when_it_is_shown_then_the_newest_lines_are_in_front() {
    let queue = Queue::new();
    let (id, _) = queue.append("a long answer");
    queue.start(id).unwrap();
    let body: String = (0..200).map(|i| format!("line {i}\n")).collect();
    queue.complete(id, body).unwrap();
    let app = App::new(queue);
    let screen = screen(&app, 60, 20);
    assert!(screen.contains("line 199"), "the newest line was not in front:\n{screen}");
}

#[rstest]
fn given_a_long_report_when_the_human_scrolls_back_then_earlier_lines_appear() {
    let queue = Queue::new();
    let (id, _) = queue.append("a long answer");
    queue.start(id).unwrap();
    let body: String = (0..200).map(|i| format!("line {i}\n")).collect();
    queue.complete(id, body).unwrap();
    let mut app = App::new(queue);
    for _ in 0..40 {
        app.scroll_report(-1);
    }
    let screen = screen(&app, 60, 20);
    assert!(!screen.contains("line 199"), "scrolling back did not move the report:\n{screen}");
    assert!(screen.contains("line 1"), "the earlier lines never came into view:\n{screen}");
}

#[rstest]
fn given_a_split_surface_when_the_report_is_expanded_then_it_gets_the_rows() {
    let mut app = app_with(40);
    let split = screen(&app, 60, 20);
    app.panes = Panes::Report;
    let full = screen(&app, 60, 20);
    let count = |s: &str| s.lines().filter(|l| l.contains("task number")).count();
    assert!(count(&full) < count(&split), "the report did not take the queue's rows");
}

#[rstest]
fn given_a_split_surface_when_the_queue_is_expanded_then_more_tasks_are_visible() {
    let mut app = app_with(40);
    let split = screen(&app, 60, 20);
    app.panes = Panes::Queue;
    let full = screen(&app, 60, 20);
    let count = |s: &str| s.lines().filter(|l| l.contains("task number")).count();
    assert!(count(&full) > count(&split), "the queue did not take the report's rows");
}

#[rstest]
fn given_a_split_surface_when_a_layout_key_is_pressed_then_that_window_is_taken() {
    let mut app = app_with(3);
    assert_eq!(app.panes, Panes::Split);
    app.handle_key(key(KeyCode::Char('2')));
    assert_eq!(app.panes, Panes::Report);
    app.handle_key(key(KeyCode::Char('1')));
    assert_eq!(app.panes, Panes::Queue);
    app.handle_key(key(KeyCode::Char('3')));
    assert_eq!(app.panes, Panes::Split);
}

#[rstest]
fn given_the_keys_the_surface_answers_when_help_is_opened_then_each_is_named() {
    let mut app = app_with(2);
    app.handle_key(key(KeyCode::Char('?')));
    // Wide enough that a long sentence is not wrapped through the middle of
    // the key it names.
    let screen = whole_help(&mut app, 140, 40);
    for named in [
        "the last command the model ran in its shell",
        "⇧⇩ one task",
        "close whatever is open",
        "Alt+⇧⇩ one line",
        "name a setting directly",
        "beside the work",
        "split the focused task",
        "save these settings",
        "reasoning",
        "1|2|3",
    ] {
        assert!(screen.contains(named), "a key nobody can find is a key nobody presses: {named}");
    }
}

#[rstest]
fn given_the_bottom_row_when_help_is_opened_then_every_action_on_it_has_a_row() {
    let mut app = app_with(2);
    let help: Vec<String> = app
        .help_groups()
        .into_iter()
        .flat_map(|(_, _, rows)| rows)
        .flat_map(|row| row.keys.split('|').map(String::from).collect::<Vec<_>>())
        .collect();
    let mut actions = app.action_items();
    actions.extend(app.key_items().into_iter().filter(|i| i.starts_with('v')));
    actions.extend(app.tail_items());
    for item in actions {
        let keys = item.split_whitespace().next().unwrap_or_default();
        for token in keys.split('/') {
            // The way into help is the one key its own table cannot teach;
            // the header says how to leave, and this row says how to arrive.
            if token == "?" || token.is_empty() {
                continue;
            }
            assert!(
                help.iter().any(|k| k == token),
                "the row names {token} and help has no row for it: {help:?}"
            );
        }
    }
}

#[rstest]
fn given_a_task_in_hand_when_the_bottom_row_is_read_then_it_says_how_to_put_it_down() {
    let mut app = app_with(2);
    app.handle_key(key(KeyCode::Char('t')));
    // A move half made has its own keyboard: the row stops being the queue's
    // and says the three things that finish or undo the move.
    let row = app.key_items().join(" · ");
    assert!(row.contains("p place after"), "the row does not say where it goes: {row}");
    assert!(row.contains("P place first"), "the row does not say how to lead: {row}");
    assert!(row.contains("␛ drop"), "the row does not say how to let it go: {row}");
    assert!(!row.contains("a add"), "the queue's keys are still on the row: {row}");
    // What is held is a state, drawn above the row, not a key.
    assert_eq!(app.hand_title().as_deref(), Some("task number 0"));

    let screen = screen(&app, 120, 20);
    assert!(screen.contains("in hand: task number 0"), "the screen does not say what is held: {screen:?}");
    assert!(screen.contains("p place after"), "the screen does not say where it goes");
}

#[rstest]
fn given_a_key_the_row_has_no_room_for_when_help_is_read_then_it_is_taught_anyway() {
    // The row drops these on purpose: a knob's reading is in the header, and
    // the place keys are dead without a hand. Help is the list of every key, so
    // it is the one screen a human can read to find what the row never says.
    let mut app = app_with(2);
    app.handle_key(key(KeyCode::Char('?')));
    let help = whole_help(&mut app, 140, 45);
    for named in ["m|M", "e|E", "w|W", "\\", "v", "p|P"] {
        assert!(help.contains(named), "help does not teach {named}:\n{help}");
    }
    // And the knob keys are still beside their readings in the header, so the
    // two screens agree about which key changes which number.
    let header = screen(&app_with(2), 140, 12);
    for named in ["(m/M)", "(e/E)", "(w/W)", "(\\)"] {
        assert!(header.contains(named), "the header stopped naming {named}:\n{header}");
    }
}

#[rstest]
fn given_any_width_when_the_action_row_is_read_then_help_and_quit_survive() {
    let app = app_with(2);
    for width in [30u16, 40, 60, 100, 140] {
        let screen = screen(&app, width, 20);
        assert!(screen.contains("? keys"), "at {width} there was no way to find the rest");
        assert!(screen.contains("q quit"), "at {width} there was no way to leave");
    }
}

#[rstest]
fn given_a_window_with_no_columns_when_the_row_is_drawn_then_the_surface_stays_up() {
    // A pty that has not been sized yet reports zero columns. The caret was
    // computed as `width - 1`, so the first frame panicked before drawing
    // anything; a window with nowhere to write is still a window to survive.
    let mut app = app_with(2);
    app.handle_key(key(KeyCode::Char('a')));
    let _ = screen(&app, 0, 12);
}

#[rstest]
fn given_a_narrow_window_when_a_row_is_read_then_it_drops_whole_items_and_says_so() {
    let mut app = app_with(2);
    app.footprint = Some(String::from("16M"));
    let screen = screen(&app, 46, 20);
    assert!(screen.contains('…'), "a shortened row claimed to be whole:\n{screen}");
    assert!(!screen.contains("mem 1"), "half a reading was shown:\n{screen}");
    assert!(!screen.contains("| ctx"), "a label without its value was shown:\n{screen}");
}

#[rstest]
fn given_a_short_window_when_the_surface_is_drawn_then_the_answer_keeps_its_rows() {
    let queue = Queue::new();
    let (id, _) = queue.append("work");
    queue.start(id).unwrap();
    queue.complete(id, "the answer").unwrap();
    let mut app = App::new(queue);
    app.place = Some(String::from("~/proj  sandbox:workspace-write  git:main"));
    let screen = screen(&app, 50, 7);
    assert!(screen.contains("the answer"), "the furniture ate the answer:\n{screen}");
}

#[rstest]
fn given_help_is_open_when_any_key_is_pressed_then_it_closes() {
    let mut app = app_with(2);
    app.handle_key(key(KeyCode::Char('?')));
    assert!(app.help);
    app.handle_key(key(KeyCode::Char('j')));
    assert!(!app.help, "help swallowed the next key as well");
}

#[rstest]
fn given_a_focused_task_when_a_part_is_nested_then_it_becomes_its_child() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let key = |c| KeyEvent::new(c, KeyModifiers::NONE);
    let mut app = app_with(1);
    let parent = app.queue.focus().unwrap();
    app.handle_key(key(KeyCode::Char('n')));
    for c in "inspect the parser".chars() {
        app.handle_key(key(KeyCode::Char(c)));
    }
    app.handle_key(key(KeyCode::Enter));
    let node = app.queue.get(parent).unwrap();
    assert_eq!(node.children.len(), 1, "nesting did nothing: {:?}", app.notice);
    assert_eq!(app.queue.next_queued(), Some(node.children[0]), "the part would never run");
}

#[rstest]
fn given_a_narrow_queue_when_a_row_is_read_then_the_bill_stays_and_the_title_admits_the_cut() {
    let queue = Queue::new();
    let (id, _) = queue.append("fix the parser");
    queue.start(id).unwrap();
    queue.charge(id, pearl_core::Cost::turn(1234, 340, 2400));
    let app = App::new(queue);
    let screen = screen(&app, 30, 12);
    let row = screen.lines().find(|l| l.contains('▶')).unwrap_or_default();
    // The bill is the reading; the title is the thing with a rest of it, and a
    // rest the human can ask for again is worth less than what the task cost.
    assert!(row.contains("↑1.2k"), "the bill was dropped from a narrow row: {row}");
    assert!(row.contains('…'), "the title was cut without saying so: {row}");
}

#[rstest]
fn given_a_narrowed_queue_when_the_heading_is_read_then_it_names_where_you_are() {
    let queue = Queue::new();
    let (parent, _) = queue.append("fix the parser");
    let _ = queue.add_child(parent, "read the grammar").unwrap();
    let _ = queue.add_child(parent, "write the test").unwrap();
    queue.narrow(parent).unwrap();
    let app = App::new(queue);
    let screen = screen(&app, 90, 20);
    let head = screen.lines().find(|l| l.contains("queue ")).unwrap_or_default();
    assert!(head.contains("fix the parser"), "the heading did not say whose parts these are: {head}");
    assert!(head.contains('2'), "the heading did not say how many parts: {head}");
}

#[rstest]
fn given_a_narrowed_queue_when_the_level_key_is_pressed_then_it_comes_back_out() {
    let queue = Queue::new();
    let (parent, _) = queue.append("fix the parser");
    let _ = queue.add_child(parent, "read the grammar").unwrap();
    queue.narrow(parent).unwrap();
    let mut app = App::new(queue);
    app.handle_key(key(KeyCode::Left));
    assert!(app.queue.scope().is_none(), "the level key did not come back out");
}

#[rstest]
fn given_a_plain_legend_when_it_is_read_then_it_offers_no_vim_letters() {
    let app = app_with(2);
    let (queue, report) = app.motion_hints();
    let legend = format!("{queue}  {report}");
    assert!(!legend.contains("j|k"), "vim keys in a plain legend: {legend}");
    assert!(legend.contains("PgUp|PgDn"), "plain paging is not the page keys: {legend}");
}

#[rstest]
fn given_one_key_row_when_it_is_read_then_every_action_is_named() {
    // Every action on one line, in the `·` the windows' rules use, so the two
    // places a key is met read the same way. The motions are gone from this
    // row -- they are on the windows -- and what is left needs this width.
    let app = app_with(2);
    let screen = screen(&app, 176, 12);
    let row = screen.lines().last().unwrap_or_default();
    for named in [
        "a add", "1|2|3 panes", "i now", "t take", "r rewrite", "s steer", "␠ pause",
        "c cancel", "z undo", "y copy", "/ btw", "T reason", "? keys", "q quit",
    ] {
        assert!(row.contains(named), "the row spent itself before {named}: {row}");
    }
    for moved in ["queue ", "report ", "level "] {
        assert!(!row.contains(moved), "the row still names {moved}: {row}");
    }
}

#[rstest]
fn given_a_window_when_its_rule_is_read_then_its_own_keys_sit_at_the_rules_end() {
    // The keys that move a window are written on that window's own rule, so the
    // eye that found the window has already found them: what this is on the
    // left, what moves it on the right.
    let app = app_with(2);
    let frame = screen(&app, 200, 20);
    let (queue_hint, report_hint) = app.motion_hints();
    let rule = |name: &str| {
        frame
            .lines()
            .find(|l| l.contains(name) && l.contains('─'))
            .unwrap_or_default()
            .trim_end()
            .to_string()
    };
    let queue_rule = rule(" queue ");
    let report_rule = rule(" report ");
    // The hint is padded by a space at each end and closed by two characters of
    // rule, so it ends the line instead of running into the corner.
    assert!(
        queue_rule.ends_with(&format!("{queue_hint} ──")),
        "the queue's rule does not end at its keys: {queue_rule}"
    );
    assert!(
        report_rule.ends_with(&format!("{report_hint} ──")),
        "the report's rule does not end at its keys: {report_rule}"
    );
}

#[rstest]
fn given_a_window_when_its_rule_is_read_then_air_stands_between_the_keys_and_the_rule() {
    // The hint is not a continuation of the rule: a name, then rule, then two
    // cells of air, then the keys. One cell is not enough -- the rule's glyph
    // bleeds into the cell beside it -- and without the gap the last dash
    // reads as part of the first key.
    let app = app_with(2);
    let frame = screen(&app, 200, 20);
    for name in [" queue ", " report "] {
        let rule = frame
            .lines()
            .find(|l| l.contains(name) && l.contains('─'))
            .unwrap_or_default()
            .to_string();
        let prefix = if rule.contains("Alt+⇧⇩") { "Alt+⇧⇩" } else { "⇧⇩" };
        let bytes = rule.find(prefix).expect("the rule names no keys");
        let at: Vec<char> = rule[..bytes].chars().collect();
        let cell = |back: usize| at.get(at.len() - back).copied().unwrap_or_default();
        assert_eq!(cell(1), ' ', "the keys touch the rule: {rule}");
        assert_eq!(cell(2), ' ', "the gap is one cell, which the rule's glyph eats: {rule}");
        assert_eq!(cell(3), '─', "the rule does not run up to the gap: {rule}");
    }
}

#[rstest]
fn given_the_arrows_when_the_row_and_the_rules_are_read_then_they_go_up_then_down_and_left_then_right() {
    // The pair is written the way an arrow cluster is printed -- up above
    // down, left beside right -- so a hand that knows the keys finds the glyph
    // where it already looks. The glyphs are also drawn without the bar the
    // named pairs need: a pair of glyphs is one sign, not two names.
    let app = app_with(2);
    let (queue_hint, report_hint) = app.motion_hints();
    let row = screen(&app, 200, 20).lines().last().unwrap_or_default().to_string();
    for legend in [&queue_hint, &report_hint, &row] {
        assert!(!legend.contains("⇧|⇩"), "the arrows are written up-then-down: {legend}");
        assert!(!legend.contains("⇦|⇨"), "the level is written left-then-right: {legend}");
    }
    assert!(queue_hint.contains("⇧⇩ task"), "the queue lost its up-then-down pair: {queue_hint}");
    assert!(report_hint.contains("Alt+⇧⇩ line"), "the report lost its up-then-down pair: {report_hint}");
    assert!(queue_hint.contains("⇦⇨ level"), "the level lost its left-then-right pair: {queue_hint}");
}

#[rstest]
fn given_any_width_when_the_frame_is_read_then_the_header_and_the_row_start_one_column_in() {
    // Both rows of furniture sit inside the work rather than on the edge:
    // the same margin at the top and the bottom is what makes them a frame.
    let app = app_with(2);
    for width in [40u16, 80, 120, 200] {
        let frame = screen(&app, width, 12);
        let lines: Vec<&str> = frame.lines().collect();
        for line in [lines.first().copied().unwrap_or_default(), lines.last().copied().unwrap_or_default()] {
            assert!(line.starts_with(' '), "at {width} a row starts on the edge: {line:?}");
            assert!(!line.starts_with("  "), "at {width} a row starts two columns in: {line:?}");
        }
    }
}

#[rstest]
fn given_a_window_too_narrow_for_its_keys_when_the_rule_is_read_then_the_hint_is_dropped_whole() {
    // Half a hint is a key nobody presses, so a rule that cannot hold the
    // whole line keeps its name and lets help carry the keys.
    let app = app_with(2);
    let frame = screen(&app, 60, 20);
    let queue_rule = frame.lines().find(|l| l.contains(" queue ")).unwrap_or_default().to_string();
    assert!(queue_rule.contains(" queue "), "the rule lost the window's name:
{frame}");
    assert!(
        !queue_rule.contains("task"),
        "the rule cut a hint in the middle instead of dropping it: {queue_rule}"
    );
}

#[rstest]
fn given_an_open_prompt_when_the_bottom_row_is_read_then_it_is_the_prompt_not_the_legend() {
    let mut app = app_with(2);
    app.handle_key(key(KeyCode::Char('a')));
    let screen = screen(&app, 70, 20);
    assert!(screen.contains("append:"), "the legend ate the input:\n{screen}");
    assert!(!screen.contains("? keys"), "the legend is still competing with the prompt");
}

/// Every line the key screen can show, gathered by scrolling it. Help is
/// taller than a terminal now that each key carries its sentence, so a story
/// about what it names has to read all of it rather than the first page.
fn whole_help(app: &mut App, w: u16, h: u16) -> String {
    let mut all = String::new();
    let mut last = usize::MAX;
    while app.help_at != last {
        last = app.help_at;
        all.push_str(&screen(app, w, h));
        app.handle_key(key(KeyCode::PageDown));
    }
    all.push_str(&screen(app, w, h));
    all
}

#[rstest]
fn given_the_look_group_when_help_is_read_then_a_style_costs_three_lines_and_the_rest_two() {
    let app = app_with(2);
    let look = app
        .help_groups()
        .into_iter()
        .find(|(en, _, _)| *en == "look")
        .map(|(_, _, rows)| rows)
        .expect("help has no look group");
    // Three facts, three lines: the queue, the report, the level. A list per
    // motion kind said "page" twice and spent the screen on headings. Then the
    // three keys the row cannot carry: the panes key moves the whole screen,
    // so no window's rule owns it; `v` reads a request in full. The draft's
    // keys are not here: they live in the editor's own crate and its own help;
    // `y` is a row key now.
    assert_eq!(look.len(), 6, "the look group is {} lines, not six", look.len());
    assert!(
        !look[2].en.contains("1|2|3"),
        "the level line still carries the panes keys: {}",
        look[2].en
    );
    assert_eq!(look[3].keys, "1|2|3", "the panes keys are not the fourth look line");
    assert_eq!(look[4].keys, "v", "the read key is not the last look line");
}

#[rstest]
fn given_help_in_english_when_tab_is_pressed_then_the_screen_speaks_chinese() {
    let mut app = app_with(2);
    app.handle_key(key(KeyCode::Char('?')));
    let en = screen(&app, 100, 40);
    assert!(en.contains("Tab Eng|中"), "the header does not name the toggle:\n{en}");
    app.handle_key(key(KeyCode::Tab));
    // A wide character occupies two cells and the buffer keeps the second
    // empty, so the rendered text reads with gaps; the words are what matter.
    let zh = squash(&screen(&app, 100, 60));
    assert!(app.help, "Tab closed the screen instead of translating it");
    assert!(zh.contains("按键"), "the header stayed English:\n{zh}");
    assert!(zh.contains("队列"), "the rows stayed English:\n{zh}");
    assert!(!zh.contains("add a task to the end"), "an English sentence survived the switch:\n{zh}");
    app.handle_key(key(KeyCode::Tab));
    assert!(screen(&app, 100, 40).contains("add a task to the end"), "Tab did not translate back");
}

#[rstest]
fn given_the_help_table_when_it_is_read_then_no_row_exists_in_one_language_only() {
    let app = app_with(2);
    let cjk = |s: &str| s.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c));
    for (en, zh, rows) in app.help_groups() {
        assert!(!en.is_empty() && !zh.is_empty(), "a group has no name in one language");
        assert!(cjk(zh), "the group {en} is not translated: {zh}");
        for row in rows {
            assert!(
                !row.en.is_empty() && !row.zh.is_empty(),
                "{} is not said in both languages",
                row.keys
            );
            // A Chinese row that is the English row with the letters left in
            // is a row nobody wrote; the reader is getting a caption, not an
            // explanation.
            assert!(cjk(&row.zh), "{} is not translated: {}", row.keys, row.zh);
            assert_ne!(row.en, row.zh, "{} says the same thing twice", row.keys);
        }
    }
}

#[rstest]
fn given_help_when_it_is_read_then_the_keys_are_grouped_in_the_rows_order() {
    let mut app = app_with(2);
    app.handle_key(key(KeyCode::Char('?')));
    let screen = whole_help(&mut app, 60, 40);
    for group in ["start", "look", "work", "safety", "ask", "set"] {
        assert!(screen.contains(group), "help is an unlabelled pile: missing {group}");
    }
    assert!(
        !screen.contains("readings"),
        "help repeats the header's readings on a second list:\n{screen}"
    );
    let row = app.key_items();
    let start = screen.find("add a task to the end").unwrap_or(usize::MAX);
    let look = screen.find("⇧⇩ one task").unwrap_or(0);
    let work = screen.find("run a task right now").unwrap_or(0);
    assert!(start < look && look < work, "help orders the keys differently from the row");
    assert_eq!(row.first().map(String::as_str), Some("a add"));
}

#[rstest]
fn given_the_header_names_the_knobs_when_the_key_row_is_read_then_they_are_not_repeated() {
    let app = app_with(2);
    assert!(
        !app.key_items().iter().any(|i| i.contains("M/E/W")),
        "the row repeats what the header already says beside each reading"
    );
}

#[rstest]
fn given_a_window_too_short_for_help_when_it_is_opened_then_it_says_how_much_it_hides() {
    let mut app = app_with(2);
    app.handle_key(key(KeyCode::Char('?')));
    let screen = screen(&app, 46, 12);
    assert!(screen.contains("⇧⇩ more"), "help cut groups off silently:\n{screen}");
    // And the rest is reachable: a screen that hides work it will not give
    // back is worse than one that never offered it.
    let rest = whole_help(&mut app, 46, 12);
    assert!(rest.contains("name a setting"), "the hidden groups cannot be scrolled to:\n{rest}");
}

#[rstest]
fn given_the_help_screen_when_it_is_read_then_the_main_legends_tail_is_not_drawn_on_it() {
    // The help screen has a header that says a key closes it and a footer that
    // says which slice of the table is showing. `? keys  q quit` is the main
    // screen's tail, and carried here it is a third voice saying the second
    // one's sentence on a line the table needed.
    let mut app = app_with(2);
    app.handle_key(key(KeyCode::Char('?')));
    // Tall enough that the whole table is on screen: on a short window the
    // table scrolls and the tail is cut off, which would pass this story
    // without the line ever being absent.
    let drawn = screen(&app, 100, 60);
    assert!(drawn.contains("keys"), "the help screen did not open:\n{drawn}");
    assert!(
        !drawn.contains("? keys  q quit"),
        "the main screen's legend is in help:\n{drawn}"
    );
}

#[rstest]
fn given_every_screen_when_it_is_drawn_then_no_cell_carries_a_colour_of_ours() {
    // PEARL runs in a terminal it cannot see the palette of. `Cyan` on a theme
    // whose background is cyan, a red that means "link" on another: a colour we
    // pick is a guess. Weight, dimness and reverse are not guesses -- the
    // terminal makes them from its own two colours -- so nothing drawn may name
    // one of its own.
    use crossterm::event::{KeyModifiers, MouseButton};
    use ratatui::style::Color;
    let queue = Queue::new();
    let (running, _) = queue.append("running");
    queue.start(running).unwrap();
    queue.notice(running, "sandbox: a write was refused (Operation not permitted)").unwrap();
    let (done, _) = queue.append("done");
    queue.start(done).unwrap();
    queue.complete(done, "# heading\n\ninline `code` and **strong**").unwrap();
    let (failed, _) = queue.append("failed");
    queue.start(failed).unwrap();
    queue.fail(failed, "it broke").unwrap();
    let mut app = App::new(queue);
    for keyed in [
        KeyCode::Char('j'),
        KeyCode::Char('P'),
        KeyCode::Char('/'),
        KeyCode::Char('T'),
        KeyCode::Char('?'),
        KeyCode::Char('m'),
        KeyCode::Char('M'),
        KeyCode::Esc,
    ] {
        app.handle_key(key(keyed));
        for (fg, bg) in pearl_tui::cell_colours(&app, 100, 30) {
            assert_eq!(fg, Color::Reset, "something was painted {fg:?} after {keyed:?}");
            assert_eq!(bg, Color::Reset, "something was painted {bg:?} after {keyed:?}");
        }
    }
    let _ = (MouseButton::Left, KeyModifiers::NONE);
}

#[rstest]
fn given_words_about_the_files_a_task_touched_when_they_are_shown_then_the_screen_reads_them_under_files(
) {
    let mut app = app_with(1);
    app.read_text = Some(String::from("what this task touched\n\nop 0  edit src/lib.rs"));
    app.read = true;

    let text = screen(&app, 120, 20);

    assert!(text.contains("files"), "{text}");
    assert!(text.contains("op 0"), "{text}");
    assert!(text.contains("src/lib.rs"), "{text}");
}

/// Given a queue holding one task in `state`, focused.
fn row_of(state: &str) -> String {
    use pearl_core::Queue;
    let mut q = Queue::new();
    let (id, _) = q.append("write the parser");
    match state {
        "queued" => {}
        "held" => {
            q.hold(id, "").unwrap();
        }
        "running" => {
            q.start(id).unwrap();
        }
        "awaiting" => {
            q.start(id).unwrap();
            q.ask(id, "which file?");
        }
        "paused" => {
            q.start(id).unwrap();
            q.hold(id, "").unwrap();
        }
        "done" => {
            q.start(id).unwrap();
            q.complete(id, "done").unwrap();
        }
        "failed" => {
            q.start(id).unwrap();
            q.fail(id, "broke").unwrap();
        }
        other => panic!("{other} is not a state"),
    }
    pearl_tui::App::new(q).key_items().join("  ")
}

#[rstest]
#[case::queued("queued", &["t take", "r rewrite", "s steer", "␠ pause", "c cancel"], &["R retry", "c drop", "s answer"])]
#[case::held("held", &["t take", "r rewrite", "s steer", "␠ run", "c cancel"], &["␠ pause"])]
#[case::running("running", &["r rewrite", "s steer", "␠ pause", "c cancel"], &["t take", "R retry"])]
#[case::awaiting("awaiting", &["s answer", "r rewrite", "c cancel"], &["t take", "␠ pause", "R retry"])]
#[case::paused("paused", &["t take", "r rewrite", "s steer", "␠ run", "c cancel"], &["␠ pause"])]
#[case::done("done", &["t take", "R retry", "c drop"], &["r rewrite", "s steer", "␠ pause"])]
#[case::failed("failed", &["t take", "R retry", "c drop"], &["r rewrite", "s steer", "␠ pause"])]
fn given_a_task_in_a_state_when_the_row_is_read_then_it_names_only_what_that_state_takes(
    #[case] state: &str,
    #[case] offered: &[&str],
    #[case] withheld: &[&str],
) {
    // The row is the keyboard of the task in front of the cursor: a key the
    // queue would refuse is not offered, and a key that changes work is called
    // what it does here. One story per state, because a row that is right for
    // a queued task and wrong for a finished one is the defect this names.
    let row = row_of(state);
    for label in offered {
        assert!(row.contains(label), "a {state} task lost {label}: {row}");
    }
    for label in withheld {
        assert!(!row.contains(label), "a {state} task was offered {label}: {row}");
    }
}

#[rstest]
fn given_a_finished_task_when_the_frame_is_drawn_then_the_last_line_offers_moving_retry_and_drop() {
    use pearl_core::Queue;
    let mut q = Queue::new();
    let (id, _) = q.append("write the parser");
    q.start(id).unwrap();
    q.complete(id, "done").unwrap();
    let app = pearl_tui::App::new(q);

    // The row is built once and drawn once: this reads the drawn cells, so a
    // row that is right in `key_items` and missing from the frame is caught.
    let screen = screen(&app, 140, 12);
    let row = screen.lines().last().unwrap_or_default();
    assert!(row.contains("R retry"), "a finished row cannot be retried: {row}");
    assert!(row.contains("c drop"), "a finished row cannot be dropped: {row}");
    // A finished row can still be moved and dropped: the list is the human's
    // to rearrange. What it cannot take is words for work that is over.
    for gone in ["r rewrite", "s steer", "␠ pause"] {
        assert!(!row.contains(gone), "a finished row still offers {gone}: {row}");
    }
}

#[rstest]
fn given_two_topics_at_the_root_when_the_queue_is_read_then_each_says_how_far_it_has_got() {
    let mut surface = App::new(Queue::new());
    let (parser, _) = surface.queue.topic("parser").unwrap();
    let (a, _) = surface.queue.add_child(parser, "lexer").unwrap();
    surface.queue.add_child(parser, "grammar").unwrap();
    surface.queue.start(a).unwrap();
    surface.queue.complete(a, "done").unwrap();

    let lines = pearl_tui::list_lines(&surface.queue, 80);

    assert!(lines.iter().any(|l| l.contains("▸ parser")), "{lines:?}");
    assert!(lines.iter().any(|l| l.contains("1/2")), "no progress: {lines:?}");
}

#[rstest]
fn given_a_topic_whose_part_finished_when_the_list_is_read_then_the_heading_says_how_far_it_got() {
    let queue = pearl_core::Queue::new();
    let (topic, _) = queue.topic("parser").unwrap();
    let (part, _) = queue.add_child(topic, "write the lexer").unwrap();
    let (idle, _) = queue.add_child(topic, "write the parser").unwrap();
    queue.start(part).unwrap();
    queue.complete(part, "done").unwrap();

    let app = App::new(queue);
    let screen = pearl_tui::screen(&app, 140, 20);

    // The heading is what the root level shows: the parts are inside it, and
    // the count is the only thing that tells the human how far it got.
    assert!(screen.contains("▸ parser  1/2"), "no count on the heading:\n{screen}");
    assert!(screen.contains("▶ · ▸ parser"), "{screen}");
    let _ = idle;
}

#[rstest]
fn given_a_topic_whose_last_part_settled_when_the_list_is_read_then_the_heading_is_done() {
    let queue = Queue::new();
    let (topic, _) = queue.topic("parser").unwrap();
    let (part, _) = queue.add_child(topic, "write the lexer").unwrap();
    queue.start(part).unwrap();
    queue.complete(part, "done").unwrap();

    let app = App::new(queue);
    let screen = screen(&app, 140, 20);

    assert!(screen.contains("✓ ▸ parser  1/1"), "the line of work never closed:\n{screen}");
}

#[rstest]
fn given_the_start_group_when_help_is_read_then_both_ways_into_a_line_of_work_are_named() {
    let queue = Queue::new();
    let mut app = App::new(queue);
    app.handle_key(key(KeyCode::Char('?')));
    let screen = screen(&app, 200, 60);

    // Help prints the keys column, not the row's short label: the sentence is
    // what names the job, and it must be the one the key does.
    let start = screen.find(" start").expect("no start group");
    let line = screen[start..].lines().nth(2).unwrap_or_default();
    assert!(line.trim_start().starts_with('A'), "`A` is missing from help: {line}");
    assert!(line.contains("name a new heading"), "{line}");
    assert!(line.contains("in your hand"), "{line}");
}

#[rstest]
fn given_a_topic_with_a_part_when_the_queue_is_read_then_the_part_steps_in_under_it() {
    let q = Queue::new();
    let (topic, _) = q.topic("parser").unwrap();
    q.add_child(topic, "write the lexer").unwrap();

    let app = pearl_tui::App::new(q);
    let screen = screen(&app, 140, 12);
    let rows: Vec<&str> = screen.lines().collect();
    let top = rows.iter().position(|r| r.contains("▸ parser")).expect("no heading");
    let below = rows[top + 1];

    // The badge stays at one column and the title steps in: `badge, space,
    // two columns of step`.
    assert!(
        below.starts_with("  ·   write the lexer") || below.starts_with("▶ ·   write the lexer"),
        "the part is not stepped in under its heading: {below}"
    );
}
