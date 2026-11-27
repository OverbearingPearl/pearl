//! TEMPORARY repro: can a topic be folded shut in the queue pane?

use crossterm::event::KeyCode;
use pearl_core::{Keys, Queue, Settings, TaskId};
use pearl_tui::{App, screen};

/// Four topics, each with two done parts: 12 rows, so folding would be visible.
fn the_list() -> (Queue, Vec<TaskId>) {
    let q = Queue::new();
    let mut topics = Vec::new();
    for name in ["alpha", "beta", "gamma", "delta"] {
        let (t, _) = q.append(name);
        q.promote(t).unwrap();
        for part in ["one", "two"] {
            let (p, _) = q.add_child(t, part).unwrap();
            q.start(p).unwrap();
            q.complete(p, "done").unwrap();
        }
        topics.push(t);
    }
    (q, topics)
}

fn settings() -> Settings {
    let s = Settings::new("m");
    s.apply(&format!("keys {}", Keys::Emacs.as_str())).unwrap();
    s
}

fn titles(app: &App) -> Vec<String> {
    app.queue
        .view()
        .iter()
        .map(|n| n.title.clone())
        .collect()
}

#[test]
fn repro_no_key_folds_a_topic_shut() {
    let (q, topics) = the_list();
    assert_eq!(q.view().len(), 12, "4 topics x (1 + 2 parts)");
    let alpha = topics[0];

    // Every key the row knows, plus the obvious suspects, tried on the topic.
    let suspects = [
        KeyCode::Enter,
        KeyCode::Char(' '),
        KeyCode::Char('z'),
        KeyCode::Char('Z'),
        KeyCode::Tab,
        KeyCode::Char('-'),
        KeyCode::Char('+'),
        KeyCode::Char('h'),
        KeyCode::Char('l'),
        KeyCode::Char('o'),
        KeyCode::Char('F'),
        KeyCode::Char('C'),
        KeyCode::Left,
        KeyCode::Right,
    ];
    for code in suspects {
        let mut app = App::new(q.clone()).with_settings(settings());
        app.queue.focus_on(alpha).unwrap();
        let before = titles(&app).len();
        let scope_before = app.queue.scope();
        app.handle_key(pearl_tui::key(code));
        let after = titles(&app).len();
        println!(
            "{code:?}: rows {before} -> {after}   scope {scope_before:?} -> {:?}   notice {:?}",
            app.queue.scope(),
            app.notice
        );
    }
    // Four topics shut would be 4 rows; five shut plus one open, 8. No key
    // reaches either: every attempt either leaves 12 rows or narrows to 2.
    for code in suspects {
        let mut one = App::new(q.clone()).with_settings(settings());
        one.queue.focus_on(alpha).unwrap();
        one.handle_key(pearl_tui::key(code));
        let rows = one.queue.view().len();
        assert!(
            rows == 12 || rows == 2,
            "{code:?} left {rows} rows: something folds a topic shut"
        );
    }
}

#[test]
fn repro_the_only_way_to_hide_a_row_is_to_drill_into_another() {
    let (q, topics) = the_list();
    let mut app = App::new(q.clone()).with_settings(settings());
    app.queue.focus_on(topics[0]).unwrap();
    println!("at rest: {:?}", titles(&app));

    // `⏎` on a topic narrows to that topic's children.
    app.handle_key(pearl_tui::key(KeyCode::Enter));
    println!("after ⏎ on alpha: {:?}", titles(&app));
    println!("scope: {:?}", app.queue.scope());
    let out = screen(&app, 100, 20);
    println!(
        "footer: {:?}",
        out.lines().find(|l| l.contains("queue ")).map(str::trim)
    );
    assert_eq!(
        app.queue.view().len(),
        2,
        "narrowing to one topic did not shrink the list to its parts"
    );
    // The other three topics are gone from the screen entirely -- this is the
    // only fold there is, and it hides three topics to show one.
    let rows: Vec<&str> = out
        .lines()
        .filter(|l| l.trim_start().starts_with(['\u{2713}', '\u{b7}', '\u{25b6}', '\u{25b8}']))
        .collect();
    println!("rows drawn: {rows:?}");
    assert!(
        out.contains("in \"alpha\""),
        "the rule does not say which level is open"
    );
}

#[test]
fn repro_the_model_s_digest_does_collapse() {
    // The digest is the other surface of the same list, and it has a collapse
    // rule: only the line of work the task in flight stands in is expanded.
    let (q, topics) = the_list();
    let (beta_part, _) = q.add_child(topics[1], "a part in flight").unwrap();

    let digest = q.digest(Some(beta_part));
    println!("--- digest with a task in flight under beta ---\n{digest}\n");
    let lines: Vec<&str> = digest.lines().collect();
    let alpha = lines.iter().find(|l| l.contains("alpha")).unwrap();
    assert!(
        alpha.contains('('),
        "a topic off the chain is not collapsed to one line: {alpha:?}"
    );
    assert!(
        lines.iter().any(|l| l.contains("beta")),
        "the chain's own root is missing"
    );
    // `walk` names at most eight parts and counts the rest.
    println!("lines that mention a part title: {}", lines.iter().filter(|l| l.contains("one") || l.contains("two")).count());
}

#[test]
fn repro_eight_is_the_digest_s_named_limit() {
    let q = Queue::new();
    let (t, _) = q.append("wide");
    q.promote(t).unwrap();
    let mut parts = Vec::new();
    for i in 0..12 {
        let (p, _) = q.add_child(t, format!("part {i}")).unwrap();
        q.start(p).unwrap();
        q.complete(p, "done").unwrap();
        parts.push(p);
    }
    let in_flight = parts[0];
    // Re-open the first part so the topic is on the chain.
    let digest = q.digest(Some(in_flight));
    println!("--- 12 parts, one in flight ---\n{digest}\n");
    println!("the queue pane would draw {} rows", q.view().len());
}

#[test]
fn repro_enter_on_a_leaf_opens_an_empty_level() {
    // `narrow` accepts any node and sets `focus` to its first child -- which a
    // leaf does not have. The list then has nothing to draw.
    let (q, _) = the_list();
    let leaf = q.get(q.view()[1].id).unwrap().id; // "one"
    let mut app = App::new(q.clone()).with_settings(settings());
    app.queue.focus_on(leaf).unwrap();
    app.handle_key(pearl_tui::key(KeyCode::Enter));
    println!(
        "after ⏎ on a leaf: scope={:?} focus={:?} rows={}",
        app.queue.scope(),
        app.queue.focus(),
        app.queue.view().len()
    );
    let out = screen(&app, 100, 20);
    println!(
        "footer: {:?}",
        out.lines().find(|l| l.contains("queue ")).map(str::trim)
    );
    println!(
        "is there a way back on the row? {:?}",
        app.key_items().join(" · ")
    );
}

#[test]
fn repro_what_the_users_list_would_look_like_folded() {
    // The list from the report: 11 roots, 5 of them topics with parts, 6 parts.
    let q = Queue::new();
    let mut roots = 0;
    let mut parts = 0;
    for (name, n) in [
        ("topicA", 0),
        ("功能", 0),
        ("windows", 0),
        ("翻页", 1),
        ("表格", 1),
        ("剪贴板", 1),
        ("topic无法drop", 2),
        ("drop掉task", 0),
        ("当前task", 0),
        ("zz", 1),
        ("token", 0),
    ] {
        roots += 1;
        if n == 0 {
            let (id, _) = q.append(name);
            q.start(id).unwrap();
            q.complete(id, "said").unwrap();
            continue;
        }
        let (t, _) = q.append(name);
        q.promote(t).unwrap();
        q.start(t).unwrap_or_default();
        for _ in 0..n {
            parts += 1;
            let (p, _) = q.add_child(t, "part").unwrap();
            q.start(p).unwrap();
            q.complete(p, "said").unwrap();
        }
    }
    println!(
        "roots={roots} parts={parts} rows now={} rows if every topic folded={roots}",
        q.view().len()
    );
}
