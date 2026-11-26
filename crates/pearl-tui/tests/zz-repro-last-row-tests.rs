//! TEMPORARY repro: which keys cost the queue a row, and where the row goes.

use crossterm::event::KeyCode;
use pearl_core::{Keys, Queue, Settings, TaskId};
use pearl_tui::{App, screen};

/// 17 rows whose titles are ASCII, so a story can look for one row and mean it.
fn the_list() -> (Queue, Vec<TaskId>) {
    let q = Queue::new();
    let mut at = Vec::new();
    let finish = |id: TaskId| {
        q.start(id).unwrap();
        q.complete(id, "said something").unwrap();
    };
    let topic = |title: &str| {
        let (id, _) = q.append(title);
        q.promote(id).unwrap();
        id
    };
    let a = topic("T-A");
    finish(a);
    at.push(a);
    let (b, _) = q.append("R02");
    finish(b);
    at.push(b);
    let (c, _) = q.append("R03");
    finish(c);
    at.push(c);
    let d = topic("T-D");
    let (d1, _) = q.add_child(d, "R05").unwrap();
    finish(d1);
    at.push(d);
    at.push(d1);
    let e = topic("T-E");
    let (e1, _) = q.add_child(e, "R08").unwrap();
    finish(e1);
    at.push(e);
    at.push(e1);
    let f = topic("T-F");
    let (f1, _) = q.add_child(f, "R11").unwrap();
    finish(f1);
    at.push(f);
    at.push(f1);
    let g = topic("T-G");
    let (g1, _) = q.add_child(g, "R14").unwrap();
    finish(g1);
    let (g2, _) = q.add_child(g, "R15").unwrap();
    finish(g2);
    at.push(g);
    at.push(g1);
    at.push(g2);
    let (h, _) = q.append("R17");
    finish(h);
    at.push(h);
    let (i, _) = q.append("R18-focused");
    finish(i);
    at.push(i);
    let j = topic("T-J");
    let (j1, _) = q.add_child(j, "R21").unwrap();
    finish(j1);
    at.push(j);
    at.push(j1);
    let k = topic("T-LAST");
    at.push(k);
    (q, at)
}

fn rows(out: &str) -> Vec<String> {
    out.lines()
        .filter(|l| {
            let t = l.trim_start();
            t.starts_with('✓') || t.starts_with('·') || t.starts_with('▶') || t.starts_with('▸')
        })
        .map(|l| l.trim_end().to_string())
        .collect()
}

fn footer(out: &str) -> String {
    out.lines()
        .find(|l| l.contains("queue ") && l.contains(" of "))
        .map(|l| l.split('─').next().unwrap_or("").trim().to_string())
        .unwrap_or_else(|| String::from("<none>"))
}

fn settings(style: Keys) -> Settings {
    let s = Settings::new("m");
    s.apply(&format!("keys {}", style.as_str())).unwrap();
    s
}

fn report(label: &str, app: &App, height: u16) {
    let out = screen(app, 200, height);
    let drawn = rows(&out);
    println!(
        "{label:<22} hand={:<7} view={:<3} drawn={:<3} footer={:<18} last-row={}",
        format!("{:?}", app.hand.is_some()),
        app.queue.view().len(),
        drawn.len(),
        footer(&out),
        if out.contains("T-LAST") { "shown" } else { "GONE" },
    );
}

#[test]
fn repro_which_keys_cost_a_row() {
    let (q, at) = the_list();
    assert_eq!(q.view().len(), 17, "the list is not the shape intended");
    let focused = at[at.len() - 3]; // R18-focused
    q.focus_on(focused).unwrap();

    // A height at which every row is drawn at rest.
    let mut height = 0;
    for h in 20..70u16 {
        let out = screen(&App::new(q.clone()).with_settings(settings(Keys::Emacs)), 200, h);
        if out.contains("T-LAST") && rows(&out).len() == 17 {
            height = h;
            break;
        }
    }
    println!("height where all 17 rows are drawn: {height}\n");

    for style in [Keys::Emacs, Keys::Vim] {
        println!("--- {style:?} ---");
        let mut app = App::new(q.clone()).with_settings(settings(style));
        app.queue.focus_on(focused).unwrap();
        report("at rest", &app, height);
        for key in ['t', 'k', 'c', '␠', 'a', 'A', 'i', 'r', 'S', 'z', 'y', '?', 'f', 'v'] {
            app.handle_key(pearl_tui::key(KeyCode::Char(key)));
            report(&format!("after '{key}'"), &app, height);
            // Close anything modal that opened, so the next key is a queue key.
            app.help = false;
            app.read = false;
            app.mode = pearl_tui::Mode::Normal;
        }
        println!();
    }
}

#[test]
fn repro_put_back_brings_the_row_back() {
    let (q, at) = the_list();
    let focused = at[at.len() - 3];
    q.focus_on(focused).unwrap();
    let mut height = 0;
    for h in 20..70u16 {
        let out = screen(&App::new(q.clone()).with_settings(settings(Keys::Emacs)), 200, h);
        if out.contains("T-LAST") && rows(&out).len() == 17 {
            height = h;
            break;
        }
    }
    let mut app = App::new(q.clone()).with_settings(settings(Keys::Emacs));

    // `t`: the focused row goes into the hand.
    app.handle_key(pearl_tui::key(KeyCode::Char('t')));
    report("after 't' (taken)", &app, height);
    // `k`: nothing is bound to it in emacs.
    app.handle_key(pearl_tui::key(KeyCode::Char('k')));
    report("after 'k'", &app, height);
    // Escape puts it back.
    app.handle_key(pearl_tui::key(KeyCode::Esc));
    report("after Esc (put back)", &app, height);
}

#[test]
fn repro_a_leaf_in_the_hand_costs_the_same_row() {
    let (q, at) = the_list();
    let leaf = at[13]; // R18-focused, a leaf
    let mut height = 0;
    for h in 20..70u16 {
        let out = screen(&App::new(q.clone()).with_settings(settings(Keys::Emacs)), 200, h);
        if out.contains("T-LAST") && rows(&out).len() == 17 {
            height = h;
            break;
        }
    }
    let mut app = App::new(q.clone()).with_settings(settings(Keys::Emacs));
    app.queue.focus_on(leaf).unwrap();
    report("at rest", &app, height);
    app.handle_key(pearl_tui::key(KeyCode::Char('t')));
    report("after 't' (leaf)", &app, height);
    println!("the taken row is still a node: {}", app.queue.get(leaf).is_some());
    app.handle_key(pearl_tui::key(KeyCode::Esc));
    report("after Esc", &app, height);
    // And with no notice at all: clear it by hand, as nothing else would.
    app.notice = None;
    report("notice cleared", &app, height);
}
