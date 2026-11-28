//! view --- single-focus flat list: badges for awareness, one cursor for focus.
//!
//! The tree executes in parallel underneath, but the human sees one flat list
//! at the current level. Status is a badge you scan; the cursor is the one spot
//! your attention rests, and the pane below it is what that task reported.

use crate::app::{App, Panes};
use crate::style;
use pearl_core::{Bill, NodeState, Queue};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use unicode_width::UnicodeWidthChar;

/// A status badge: awareness at a glance, no attention demanded.
pub fn badge(state: NodeState) -> &'static str {
    match state {
        NodeState::Queued => "·",
        NodeState::Running => "▶",
        NodeState::AwaitingInput => "?",
        NodeState::Paused => "‖",
        NodeState::Done => "✓",
        NodeState::Failed => "✗",
        NodeState::Cancelled => "⊘",
    }
}

/// How a badge is drawn when the only palette we may trust is the human's:
/// the glyph carries the state, and the weight says how much of a look it asks
/// for. The three styles are all the surface owns (see `style`), so a state
/// that needed a colour would be a state that needed a better glyph.
fn badge_style(state: NodeState) -> Style {
    match state {
        NodeState::Running => style::strong(),
        NodeState::AwaitingInput => style::strong().add_modifier(Modifier::UNDERLINED),
        NodeState::Failed => style::marked(),
        NodeState::Done | NodeState::Paused => style::dim(),
        _ => Style::default(),
    }
}

/// Hard-wrap `text` at `cols` display columns, keeping every line.
///
/// Columns, not characters: a title written in Chinese draws two cells per
/// character, so a line wrapped by counting characters runs past the glass and
/// is clipped by the terminal without a mark -- the same mistake ADR 21 names
/// for `fit`, and the one that leaves half a wide character and a neighbour's
/// numbers on the frame, because the console's model of the screen stops
/// agreeing with the cells that are actually lit.
pub fn wrap(text: &str, cols: usize) -> Vec<String> {
    let glass = cols.max(1);
    let mut lines: Vec<String> = Vec::new();
    for line in text.lines() {
        if line.is_empty() {
            lines.push(String::new());
            continue;
        }
        let mut current = String::new();
        let mut width = 0;
        for c in line.chars() {
            let cell = UnicodeWidthChar::width(c).unwrap_or(0);
            if width + cell > glass && !current.is_empty() {
                lines.push(std::mem::take(&mut current));
                width = 0;
            }
            current.push(c);
            width += cell;
        }
        lines.push(current);
    }
    lines
}

/// One round's work and its bill, fitted to the window.
///
/// The work yields first -- `…` says it did -- because the bill is the reason
/// the line is printed at all, and a round whose note ate the numbers says it
/// was expensive without saying how.
fn bill_line(number: usize, paid: &Bill, width: usize) -> String {
    let lead = format!("round {number}  ");
    let cost = paid.bill();
    // The bill stands in the same column the queue's readings stand in: one
    // margin in from the right, on every round. Numbers that start wherever
    // the note happened to end cannot be compared down a list of rounds, and
    // comparing rounds is what the column is for.
    let room = width.saturating_sub(2 + crate::fit::width_of(&lead) + crate::fit::width_of(&cost));
    let note = crate::fit::middle(&paid.note, room);
    let gap = room.saturating_sub(crate::fit::width_of(&note));
    format!("{lead}{note}{}{cost}", " ".repeat(gap))
}

/// Render the queue's current view into a frame: the list above, the report of
/// the focused task below. Reading is not a mode you enter.
/// How many rows each window gets. One function, because a page key that
/// jumps a different number of rows than the window shows is a page key that
/// lies about what a page is.
pub fn pane_rows(app: &App, height: u16) -> (u16, u16) {
    let view = app.queue.view();
    // A hole is a row the list draws, so the list is sized with it: a pane that
    // counted only the queue's own rows would clip the hole it draws whenever
    // the list filled the pane exactly -- and the hole is the way back.
    let hole = app
        .queue
        .lifted()
        .is_some_and(|(_, parent, _)| hole_has_a_row(&app.queue, parent, &view));
    let needed = u16::try_from(view.len())
        .unwrap_or(u16::MAX)
        .saturating_add(1)
        .saturating_add(u16::from(hole));
    let half = (height / 2).max(2);
    let rows = match app.panes {
        Panes::Split => needed.clamp(2, half),
        Panes::Queue => height.saturating_sub(2).max(2),
        Panes::Report => 2,
    };
    (rows, height.saturating_sub(rows))
}

pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    // A rule under a title, not a box: the sides of the screen are for text.
    // The panes give up the last column, as the header and the row do, because
    // a character written there arms the pending wrap. The left edge keeps its
    // column: a pane's title and its rows already begin one cell in, so the
    // rule's name lines up with the rows it heads.
    let area = Rect {
        width: area.width.saturating_sub(1),
        ..area
    };
    let (rows, _) = pane_rows(app, area.height);
    let parts = Layout::vertical([Constraint::Length(rows), Constraint::Min(0)]).split(area);
    render_list(app, frame, parts[0]);
    render_report(app, frame, parts[1]);
}

/// A pane's topic: a title and a rule beneath it, spanning the full width.
fn topic(title: String) -> Block<'static> {
    headed(title, None)
}

/// A topic whose rule also carries the keys that move the window it heads.
///
/// The hint sits at the rule's right end, opposite the name of what the window
/// shows, so the two read as one line: what this is on the left, what moves it
/// on the right. A window too narrow for both keeps the name and drops the
/// hint whole -- a caption cut mid-key teaches half a key, and `?` still has
/// the sentence. The row paid for these keys out of the width its operations
/// needed; the rule has the width anyway, because it is drawn either way.
///
/// Both ends breathe: a space between the name and its rule, two cells between
/// the rule and the hint, and two characters of rule left at the far corner,
/// because a hint that runs into the edge reads as a line that was severed
/// rather than one that ends. The gap is two cells rather than one: a rule's
/// own glyph bleeds into the cell beside it in most monospace fonts, and a
/// single space between `─` and a key is a space nobody can see.
fn headed(title: String, hint: Option<String>) -> Block<'static> {
    let mut block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(style::dim())
        .title_bottom(Line::from(title).style(style::dim()));
    if let Some(hint) = hint {
        block = block.title_bottom(Line::from(hint).right_aligned().style(style::dim()));
    }
    block
}

/// The hint, dressed for the rule: padded by a space at each end and closed by
/// a short stretch of rule so the line ends rather than runs out. Kept only if
/// the rule can hold it beside its name with a stretch of rule between them.
/// Whole or nothing: half a hint is a key nobody presses.
fn hint_that_fits(area: Rect, title: &str, hint: String) -> Option<String> {
    let dressed = format!("  {hint} ──");
    let need = crate::fit::width_of(title) + crate::fit::width_of(&dressed) + 3;
    (need <= area.width as usize).then_some(dressed)
}

/// A laid-out report and the readings it was laid out from. Kept between
/// frames because the screen redraws ten times a second whether or not a word
/// changed, and laying out an answer costs about a hundred times the answer's
/// own size in small allocations -- for a forty-kilobyte answer, three and a
/// half thousand of them, ten times a second, on a screen nobody is typing on.
#[derive(Debug, Clone)]
pub struct Laid {
    /// Everything the rows were made of, in one number: the node's words and
    /// its state, the width they were fitted to, and whether thinking is shown.
    digest: u64,
    lines: std::sync::Arc<Vec<Line<'static>>>,
    /// The row the answer begins on, once the task has settled: the window
    /// rests there rather than at the bottom of a long answer.
    answer: Option<usize>,
}

/// Fold one reading into a digest. FNV-1a: no allocation, and a collision
/// costs a stale row until the next change rather than a wrong one forever.
fn fold(hash: u64, bytes: &[u8]) -> u64 {
    let mut h = hash;
    for byte in bytes {
        h ^= u64::from(*byte);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// The report as the pane draws it: the whole document, in order, whether or
/// not it fits. The pane shows the window its offset names, and a story can
/// read what is beyond the glass without a terminal to scroll.
///
/// The rows are the same rows on a screen that changed nothing, so they are
/// laid out once and handed back until something they were made of moves.
pub fn report_lines(app: &App, width: usize) -> std::sync::Arc<Vec<Line<'static>>> {
    let queue = &app.queue;
    // The node is read through the lock rather than copied out of it: a row
    // that has run twenty rounds carries every note, the answer and the
    // thinking, and the window asks for it ten times a second to draw the same
    // words.
    queue.with_task(queue.focus().unwrap_or_default(), |node| {
        let mut digest = 0xcbf2_9ce4_8422_2325;
        digest = fold(digest, &width.to_le_bytes());
        digest = fold(digest, &[u8::from(app.show_thinking)]);
        if let Some(n) = node {
            digest = fold(digest, n.title.as_bytes());
            digest = fold(digest, badge(n.state).as_bytes());
            for words in [
                n.result.as_deref(),
                n.error.as_deref(),
                n.activity.as_deref(),
                n.reasoning.as_deref(),
            ] {
                digest = fold(digest, words.unwrap_or("").as_bytes());
            }
            for bill in &n.bills {
                digest = fold(digest, bill.note.as_bytes());
            }
            for notice in &n.notices {
                digest = fold(digest, notice.as_bytes());
            }
        }
        let laid = app.report.borrow();
        if let Some(laid) = laid.as_ref() {
            if laid.digest == digest {
                return std::sync::Arc::clone(&laid.lines);
            }
        }
        let (lines, answer) = lay_out(app, node, width);
        let lines = std::sync::Arc::new(lines);
        drop(laid);
        *app.report.borrow_mut() = Some(Laid {
            digest,
            lines: std::sync::Arc::clone(&lines),
            answer,
        });
        lines
    })
}

/// The rows themselves, from the node as it stood when they were asked for.
fn lay_out(
    app: &App,
    node: Option<&pearl_core::TaskNode>,
    width: usize,
) -> (Vec<Line<'static>>, Option<usize>) {
    let thought = node.and_then(|n| n.reasoning.clone());
    let answer = match node {
        Some(n) => n
            .result
            .clone()
            .or_else(|| n.error.clone())
            .or_else(|| n.activity.clone())
            .unwrap_or_else(|| String::from("no report yet")),
        None => String::from("no report yet"),
    };
    // Thinking reads before the answer and dimmer than it: it is how the
    // answer was reached, not the answer, and the eye lands on the answer.
    let dim = style::dim();
    let mut lines: Vec<Line<'static>> = Vec::new();
    // Each round's own bill, before the answer it paid for. A task's total
    // tells you what it cost; the bills tell you which round was expensive.
    if let Some(n) = node {
        for (i, bill) in n.bills.iter().enumerate() {
            lines.push(Line::styled(bill_line(i + 1, bill, width), dim));
        }
        if !n.bills.is_empty() {
            lines.push(Line::default());
        }
    }
    // What the environment did to this task: a write the sandbox refused, a
    // history folded to fit the window. Neither is the model's words, and an
    // answer shaped by them is not the whole record without them.
    if let Some(n) = node {
        if !n.notices.is_empty() {
            lines.push(Line::styled(String::from("noted"), dim));
            for notice in &n.notices {
                // The bullet hangs two cells in, the way the round bills do, so
                // the eye that scans the left edge finds the topic once.
                let said = format!("· {notice}");
                lines.extend(wrap(&said, width).into_iter().map(|l| Line::styled(l, dim)));
            }
            lines.push(Line::default());
        }
    }
    let mut answer_at = lines.len();
    if let (Some(t), true) = (&thought, app.show_thinking) {
        lines.push(Line::styled(String::from("thinking"), dim));
        // Thinking is drawn plainly: it is how the answer was reached, and the
        // marks in it are the model thinking aloud rather than a document.
        lines.extend(wrap(t, width).into_iter().map(|l| Line::styled(l, dim)));
        lines.push(Line::default());
        // The header belongs to the answer: a window that started under it
        // would open on the first sentence without saying what it is.
        answer_at = lines.len();
        lines.push(Line::styled(String::from("answer"), dim));
    }
    // The answer is the terminal's own ink: it is what the human came to
    // read, and a colour of its own made it dimmer than the chrome around it.
    let failed = node.is_some_and(|n| n.state == NodeState::Failed);
    // A failed answer is not painted in a colour of ours either: the `✗` and
    // the words carry the failure, and a red on a theme whose errors are orange
    // is a colour that means something else.
    let ink = if failed {
        style::strong()
    } else {
        Style::default()
    };
    // The answer is drawn through its own marks: a topic, a list and a code
    // block arrive as the model wrote them, and the human reads a document
    // rather than the syntax of one. Every row still comes back no wider than
    // the pane, because a line the terminal clips at a cell it did not choose
    // is how a wide character loses its second half.
    lines.extend(crate::marks::lines(&answer, width, ink));
    // A running task has no answer to rest on: its activity line is the news,
    // and the window keeps following the bottom until the task settles.
    let settled = node.is_some_and(|n| n.is_settled());
    (lines, settled.then_some(answer_at))
}

/// Where the report's window starts, in document rows. One function, because
/// a window that starts anywhere but here is a window the selection and the
/// drawing would disagree about.
pub fn report_offset(app: &App, height: usize, len: usize) -> usize {
    // A window is exactly `height` rows tall and slides over the document, so
    // it can only sit where a whole screen still fits. Past the first screen
    // there is no place further back: clamped as a distance from the end of
    // the text, the window shrank as it went and finished in blank pages.
    let room = len.saturating_sub(height);
    // A settled task is read from its answer. The rounds and the notes are how
    // the answer was reached, and a window that opens on the last line of a
    // long answer opens in the middle of it. Following the task while it runs
    // still means the bottom, because there is no answer yet and the newest
    // line is the news.
    if app.report_held() == 0 && !app.report_moved {
        if let Some(answer) = report_answer(app) {
            return answer.min(room);
        }
    }
    len.saturating_sub(app.report_held().min(room))
        .saturating_sub(height)
}

/// Where a settled task's answer begins in the report's rows, when the pane is
/// following and has one: the resting place of the window after a task ends.
pub fn report_answer(app: &App) -> Option<usize> {
    app.report.borrow().as_ref().and_then(|laid| laid.answer)
}

fn render_report(app: &App, frame: &mut Frame, area: Rect) {
    // Only the title needs the node, and a title is one word: the node itself
    // is read through the queue rather than copied into every frame.
    let thought = app
        .queue
        .focus()
        .map(|id| {
            app.queue
                .with_task(id, |node| node.and_then(|n| n.reasoning.clone()))
        })
        .unwrap_or_default();
    // The topic's rule is the same whatever the title says, so the width the
    // text wraps at is known before the title is: the count of lines decides
    // how far back "the oldest line" is, and only the drawing can count them.
    let inner = topic(String::from(" report ")).inner(area);
    let lines = report_lines(app, inner.width as usize);
    app.report_len.set(lines.len());
    // A page is the pane's own height, so a page key moves the window by
    // exactly one screenful. Measured here because the drawing is the only
    // place that knows how many rows the pane got.
    let page = (inner.height as usize).max(1);
    app.report_page.set(page);
    let start = report_offset(app, page, lines.len());
    let end = (start + page).min(lines.len());
    let back = lines.len().saturating_sub(end);
    let mut title = String::from(" report ");
    if let Some(t) = &thought {
        title = if app.show_thinking {
            String::from(" report (thinking open) ")
        } else {
            format!(" report (+thinking, {} lines) ", t.lines().count())
        };
    }
    if back > 0 {
        title = format!("{title}(+{back} newer) ");
    }
    let (_, motions) = app.motion_hints();
    let block = headed(title.clone(), hint_that_fits(area, &title, motions));
    let shown: Vec<Line> = lines[start..end].to_vec();
    frame.render_widget(Paragraph::new(shown).block(block), area);
}

/// The queue as the pane draws it: one row per task, in order, whether or not
/// the pane is tall enough. The cursor glyph follows the focus, so a copy of
/// the rows reads the way the screen did.
pub fn list_lines(queue: &Queue, width: usize) -> Vec<String> {
    list_rows(queue, width)
        .into_iter()
        .map(|(_, text)| text)
        .collect()
}

/// Whether the hand's hole has a row to be drawn in.
///
/// The hole belongs to the level the row came from, and that level is on the
/// screen when its own rows are: the level on screen itself, or, at the root,
/// any topic -- the list draws a topic's parts under it, so a part lifted out
/// of one keeps its hole where the hand found it. A root row lifted while the
/// human looks inside a topic has no row of its level on the screen, and a hole
/// at the bottom of someone else's topic would say it came from there.
fn hole_has_a_row(
    queue: &Queue,
    parent: Option<pearl_core::TaskId>,
    view: &[pearl_core::TaskNode],
) -> bool {
    parent == queue.scope() || parent.is_some_and(|p| view.iter().any(|n| n.id == p))
}

/// The rows the list draws, and the node each is for. A lifted row leaves a
/// placeholder behind: the hole and its title stay on the screen, but no
/// cursor can stand in it, because the row is in the hand.
pub fn list_rows(queue: &Queue, width: usize) -> Vec<(Option<pearl_core::TaskId>, String)> {
    let focus = queue.focus();
    let mut rows: Vec<(Option<pearl_core::TaskId>, String)> = queue
        .view()
        .iter()
        .map(|node| (Some(node.id), row_text(queue, node, width, focus)))
        .collect();
    let Some((id, parent, index)) = queue.lifted() else {
        return rows;
    };
    let Some(title) = queue
        .get(id)
        .map(|n| n.title.lines().next().unwrap_or_default().to_string())
    else {
        return rows;
    };
    // The level's own rows in drawing order, so the placeholder lands where
    // the row will go back rather than after its neighbours' children.
    let view = queue.view();
    if !hole_has_a_row(queue, parent, &view) {
        return rows;
    }
    let at = view
        .iter()
        .enumerate()
        .filter(|(_, n)| n.parent == parent)
        .nth(index)
        .map(|(position, _)| position)
        .unwrap_or_else(|| {
            // The node was the level's only row, so nothing of that level is
            // left to stand after: the hole goes under the row that held it --
            // a topic's own row, or the end of the roots.
            view.iter()
                .rposition(|n| n.parent == parent)
                .or_else(|| parent.and_then(|p| view.iter().position(|n| n.id == p)))
                .map_or(rows.len(), |position| position + 1)
        });
    // The hole is drawn where the row stood, so it steps in under its topic the
    // way the row does: an indent of its own is the only thing that says "this
    // one lives inside that one" now that the row itself is off the list.
    let indent = "  ".repeat(depth_of(queue, parent));
    let line = format!(
        "{indent}  · {} (in hand)",
        crate::fit::head(&title, width.saturating_sub(16 + indent.len()))
    );
    rows.insert(at.min(rows.len()), (None, line));
    rows
}

/// How far in a row standing on this level stands: the number of topics over
/// it. The list indents by this, and the hand's hole indents by the same, so
/// the hole of a part sits level with the parts around it.
fn depth_of(queue: &Queue, at: Option<pearl_core::TaskId>) -> usize {
    let mut depth = 0;
    let mut at = at;
    while let Some(parent) = at {
        depth += 1;
        at = queue.get(parent).and_then(|n| n.parent);
    }
    depth
}

/// One row, as a string: the marks, the badge, the title, and the readings
/// that do not yield to it.
fn row_text(
    queue: &Queue,
    node: &pearl_core::TaskNode,
    width: usize,
    focus: Option<pearl_core::TaskId>,
) -> String {
    let cursor = if Some(node.id) == focus { "▶ " } else { "  " };
    // A part steps in under its topic behind a `↳` and at its own depth, so
    // the whole row -- badge, words and bill -- moves right together. An
    // indent on the words alone left the badge straight under the topic's,
    // which reads as a sibling.
    let depth = depth_of(queue, node.parent);
    let step = if depth == 0 { "" } else { "↳ " };
    let indent = "  ".repeat(depth);
    let head_of_row = format!("{indent}{cursor}{} {step}", badge(node.state));
    // The readings stand at the right, one margin in from the edge: a column
    // of bills that floats after the words is read as part of the sentence,
    // and a column that touches the edge is read as the edge. The margin is
    // the same one the header keeps, so the two rows line up.
    let room = width.saturating_sub(2 + crate::fit::width_of(&head_of_row));
    // The bill and the child count are readings: a row that drops them
    // to make room for a title has hidden what the task cost. So the
    // title yields first, and yields with a mark that says it did.
    let mut tail = String::new();
    // A topic always says how far it has got, even at 0/0: a topic
    // with no reading is a folder, and the human cannot see whether the
    // line of work is finished without opening it.
    if node.form == pearl_core::Form::Topic {
        let done = node
            .children
            .iter()
            .filter(|id| queue.get(**id).map(|n| n.is_settled()).unwrap_or(false))
            .count();
        tail.push_str(&format!("  {done}/{}", node.children.len()));
    } else if !node.children.is_empty() {
        tail.push_str(&format!("  [{}]", node.children.len()));
    }
    // The last reading on a row is how long it took. While the task is
    // at work that same reading counts up in the same place, so nothing
    // new appears on the row and nothing has to be learned twice.
    if node.state == pearl_core::NodeState::Running {
        let mut live = node.cost;
        live.ms = node.worked_ms();
        tail.push_str("  ");
        if node.cost.is_empty() {
            tail.push_str(&live.describe_time());
        } else {
            tail.push_str(&live.describe());
        }
    } else if !node.cost.is_empty() {
        tail.push_str("  ");
        tail.push_str(&node.cost.describe());
    }
    // A task waiting on the human says so on its own row, with the
    // question: a badge that only says "waiting" makes them hunt.
    if let (NodeState::AwaitingInput, Some(question)) = (node.state, &node.question) {
        tail.push_str("  ");
        tail.push_str(&format!(
            "? {}",
            question.lines().next().unwrap_or_default()
        ));
    }
    // A row is one line tall whatever was written: a draft may hold
    // paragraphs, and the row shows the sentence they open with. The
    // rest is in the report and behind `v` -- the row is a place in a
    // queue, not a page.
    let mut first = node.title.lines().next().unwrap_or_default().to_string();
    if node.form == pearl_core::Form::Topic {
        first.insert_str(0, "▸ ");
    }
    let title = if node.title.lines().count() > 1 {
        format!("{first}…")
    } else {
        first
    };
    // A cut row says where the rest of the sentence went. The mark
    // alone admits the loss without offering the remedy, and a key
    // nobody is told about is a key nobody presses.
    let mut hint = "";
    if crate::fit::width_of(&title) > room.saturating_sub(crate::fit::width_of(&tail)) {
        const SAID: &str = "  (v shows it all)";
        // Only where a few columns of title survive it: a row that
        // spends its whole width explaining itself has not helped.
        if crate::fit::width_of(&tail) + crate::fit::width_of(SAID) + 8 <= room {
            hint = SAID;
        }
    }
    let title_room = room.saturating_sub(crate::fit::width_of(&tail) + crate::fit::width_of(hint));
    // The badge sits at one column so the eye can read the states down
    // the pane, and what is left of the row after the title is spent
    // between the words and the readings rather than on their right.
    let head = crate::fit::head(&title, title_room);
    // A row with no reading is not padded out to the column: trailing spaces
    // are invisible, and a row that ends in them is one a `$` cannot see the
    // end of.
    let gap = if tail.is_empty() {
        String::new()
    } else {
        " ".repeat(
            room.saturating_sub(crate::fit::width_of(&head) + crate::fit::width_of(hint))
                .saturating_sub(crate::fit::width_of(&tail)),
        )
    };
    format!("{head_of_row}{head}{hint}{gap}{tail}")
}

/// Where the queue's window starts, in document rows: the cursor's place,
/// held near the middle of the list it sits in.
pub fn list_offset(queue: &Queue, height: usize) -> usize {
    let nodes = queue.view();
    let rows = height;
    let at = nodes
        .iter()
        .position(|n| Some(n.id) == queue.focus())
        .unwrap_or(0);
    at.saturating_sub(rows / 2)
        .min(nodes.len().saturating_sub(rows))
}

fn render_list(app: &App, frame: &mut Frame, area: Rect) {
    let queue = &app.queue;
    let focus = queue.focus();
    let block = topic(String::new());
    let inner = block.inner(area);
    let rows = inner.height as usize;
    // The page is the drawn height here too: a page key that moved by any
    // other number would not land a screen away from where the eye was.
    app.queue_page.set(rows.max(1));
    let lines = list_rows(queue, inner.width as usize);
    let start = list_offset(queue, rows);
    let items: Vec<ListItem> = lines
        .iter()
        .skip(start)
        .take(rows)
        .enumerate()
        .map(|(_, (id, text))| {
            let node = id.and_then(|id| queue.get(id));
            let style = match &node {
                // A topic is weight, not a character: the eye sorts a line of
                // work from a task before it reads either title.
                Some(n) if n.form == pearl_core::Form::Topic => {
                    Style::default().add_modifier(Modifier::BOLD)
                }
                Some(n) if Some(n.id) == focus => Style::default().add_modifier(Modifier::BOLD),
                _ => Style::default(),
            };
            // The badge keeps its colour: the row is one string so a copy of it
            // reads like the screen, and colour is drawn back on top of the
            // badge's one cell rather than argued into the string.
            let mut spans = vec![Span::raw(text.clone())];
            if id.is_none() {
                // The placeholder is a hole, not a row: no badge, no weight.
                return ListItem::new(Line::from(spans)).style(crate::style::dim());
            }
            if let Some(n) = &node {
                spans = vec![
                    Span::raw(text.chars().take(2).collect::<String>()),
                    Span::styled(
                        text.chars().nth(2).map(String::from).unwrap_or_default(),
                        badge_style(n.state),
                    ),
                    Span::raw(text.chars().skip(3).collect::<String>()),
                ];
            }
            ListItem::new(Line::from(spans)).style(style)
        })
        .collect();
    // Where the window sits in the whole, so a long queue is not a mystery.
    // The count is of rows the cursor can stand on: a placeholder left by a
    // lift is a hole, not a task, and counting it would make the list claim a
    // row the human cannot reach.
    let nodes = queue.view();
    let before = lines
        .iter()
        .take(start)
        .filter(|(id, _)| id.is_some())
        .count();
    let in_window = lines
        .iter()
        .skip(start)
        .take(rows)
        .filter(|(id, _)| id.is_some())
        .count();
    let shown = if nodes.is_empty() {
        String::from(" queue ")
    } else {
        if in_window == 0 {
            format!(" queue 0 of {} ", nodes.len())
        } else {
            format!(
                " queue {}-{} of {} ",
                before + 1,
                before + in_window,
                nodes.len()
            )
        }
    };
    // A level is a place, so the topic names it: "(narrowed)" says that you
    // moved without saying where to, and the way back is a key you must trust.
    let shown = match queue.scope().and_then(|id| queue.get(id)) {
        Some(parent) => format!("{shown}in \"{}\" ", parent.title),
        None => shown,
    };
    let (motions, _) = app.motion_hints();
    let block = headed(shown.clone(), hint_that_fits(area, &shown, motions));
    frame.render_widget(List::new(items).block(block), area);
}
