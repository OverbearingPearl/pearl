//! marks --- the answer's own marks, read where text becomes rows.
//!
//! A model answers in markdown, and the report used to print the syntax: a
//! heading arrived as `### 一句话`, a list as dashes in the first column, and a
//! code block as three backticks and a gap. The console owns presentation and
//! the session owns none (ADR 26), so the marks are read here, once, on the way
//! to the glass: what the log holds is the model's answer, and only its drawing
//! knows the answer was written in markdown.
//!
//! What is *not* here is as deliberate as what is. There is no diagram
//! renderer, because a terminal has no vector engine; a mermaid block is drawn
//! as the code it is, with its language named. There are no tables yet, so a
//! table's pipes are drawn as the text they are rather than as a grid that a
//! narrow pane would break.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// A rule, for a heading's underline and a horizontal line of its own.
const RULE: char = '─';
/// The left edge of a quote or a code block: content set off from prose
/// without a box, because the sides of the screen are for text.
const EDGE: &str = "│ ";

/// One model answer as rows, every row already fitted to `width` columns.
pub fn lines(text: &str, width: usize, body: Style) -> Vec<Line<'static>> {
    let width = width.max(1);
    let dim = Style::default().fg(Color::DarkGray);
    let mut out: Vec<Line<'static>> = Vec::new();
    let mut fence: Option<String> = None;
    for raw in text.lines() {
        let opens = raw.trim_start();
        if let Some(_) = &fence {
            if opens.starts_with("```") {
                fence = None;
                continue;
            }
            // Code is not wrapped. A command broken across two rows is not the
            // command that ran, so the right edge cuts and says it cut.
            let room = width.saturating_sub(UnicodeWidthStr::width(EDGE));
            out.push(Line::from(vec![
                Span::styled(EDGE, dim),
                Span::styled(crate::fit::head(raw, room), body),
            ]));
            continue;
        }
        if opens.starts_with("```") {
            let language = opens.trim_start_matches('`').trim().to_string();
            fence = Some(language.clone());
            let name = if language.is_empty() { String::from("code") } else { language };
            out.push(Line::from(vec![
                Span::styled(EDGE, dim),
                Span::styled(name, dim.add_modifier(Modifier::ITALIC)),
            ]));
            continue;
        }
        if let Some((level, title)) = heading(opens) {
            let style = body.add_modifier(Modifier::BOLD);
            let style = if level <= 2 { style.fg(Color::White) } else { style.fg(Color::Cyan) };
            out.extend(indent("", title, width, style, dim));
            // The first two levels are the document's own divisions, so they
            // are drawn as divisions: a rule under the words.
            if level <= 2 {
                out.push(Line::styled(RULE.to_string().repeat(width), dim));
            }
            continue;
        }
        if let Some(rest) = bullet(opens) {
            out.extend(indent("• ", rest, width, body, dim));
            continue;
        }
        if let Some((marker, rest)) = ordered(opens) {
            out.extend(indent(&marker, rest, width, body, dim));
            continue;
        }
        if let Some(rest) = quote(opens) {
            out.extend(indent(EDGE, rest, width, dim, dim));
            continue;
        }
        if is_rule(opens) {
            out.push(Line::styled(RULE.to_string().repeat(width), dim));
            continue;
        }
        out.extend(indent("", raw, width, body, dim));
    }
    out
}

/// `#` to `######`, a space, and the words: a heading and its level.
fn heading(text: &str) -> Option<(usize, &str)> {
    let level = text.chars().take_while(|c| *c == '#').count();
    if level == 0 || level > 6 {
        return None;
    }
    let rest = &text[level..];
    if rest.is_empty() {
        return Some((level, ""));
    }
    rest.starts_with(' ').then(|| (level, rest.trim_start()))
}

/// A bullet's words, when the line is a bullet.
fn bullet(text: &str) -> Option<&str> {
    ["- ", "* ", "+ "].iter().find_map(|mark| text.strip_prefix(mark))
}

/// A numbered item's marker and words. The marker keeps its own width so a
/// continuation row lines up under the words rather than under the number.
fn ordered(text: &str) -> Option<(String, &str)> {
    let digits = text.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits == 0 {
        return None;
    }
    let rest = &text[digits..];
    let punct = rest.chars().next()?;
    if punct != '.' && punct != ')' {
        return None;
    }
    let words = rest[1..].strip_prefix(' ')?;
    Some((format!("{}{punct} ", &text[..digits]), words))
}

/// A quoted line's words.
fn quote(text: &str) -> Option<&str> {
    let rest = text.strip_prefix('>')?;
    Some(rest.strip_prefix(' ').unwrap_or(rest))
}

/// A line that is only three or more rule characters.
fn is_rule(text: &str) -> bool {
    let marks = text.trim();
    marks.len() >= 3 && marks.chars().all(|c| c == '-' || c == '*' || c == '_')
}

/// `content` under `prefix`, with continuation rows indented to the prefix's
/// width: a list whose second row starts in the margin is two lists.
fn indent(prefix: &str, content: &str, width: usize, style: Style, dim: Style) -> Vec<Line<'static>> {
    let pad = UnicodeWidthStr::width(prefix);
    let room = width.saturating_sub(pad).max(1);
    let rows = wrap(inline(content, style), room);
    let mut out: Vec<Line<'static>> = Vec::new();
    for (i, row) in rows.into_iter().enumerate() {
        let mut spans: Vec<Span<'static>> = Vec::new();
        if i == 0 {
            if !prefix.is_empty() {
                spans.push(Span::styled(prefix.to_string(), dim));
            }
        } else if pad > 0 {
            spans.push(Span::raw(" ".repeat(pad)));
        }
        spans.extend(row);
        out.push(Line::from(spans));
    }
    if out.is_empty() {
        out.push(Line::styled(prefix.to_string(), dim));
    }
    out
}

/// The marks inside a paragraph: strong, emphasised, code, and a link.
///
/// The span that opens a mark is the span that closes it. A mark without its
/// partner is printed as the character it is, because guessing at what the
/// model meant is how a report starts saying things the answer did not.
fn inline(text: &str, body: Style) -> Vec<Span<'static>> {
    let code = Style::default().fg(Color::Cyan);
    let mut out: Vec<Span<'static>> = Vec::new();
    let mut plain = String::new();
    let mut rest = text;
    while let Some(c) = rest.chars().next() {
        let mut handled: Option<(String, Style, &str)> = None;
        if let Some(after) = rest.strip_prefix("**") {
            if let Some(end) = after.find("**") {
                handled = Some((after[..end].to_string(), body.add_modifier(Modifier::BOLD), &after[end + 2..]));
            }
        }
        if handled.is_none() {
            if let Some(after) = rest.strip_prefix('`') {
                if let Some(end) = after.find('`') {
                    handled = Some((after[..end].to_string(), code, &after[end + 1..]));
                }
            }
        }
        if handled.is_none() {
            let opener = if rest.starts_with('*') {
                Some('*')
            } else if rest.starts_with('_') {
                Some('_')
            } else {
                None
            };
            if let Some(mark) = opener {
                let after = &rest[1..];
                if let Some(end) = after.find(mark) {
                    if end > 0 {
                        handled = Some((
                            after[..end].to_string(),
                            body.add_modifier(Modifier::ITALIC),
                            &after[end + 1..],
                        ));
                    }
                }
            }
        }
        if handled.is_none() {
            if let Some(after) = rest.strip_prefix('[') {
                if let Some(mid) = after.find("](") {
                    if let Some(end) = after[mid..].find(')') {
                        let words = &after[..mid];
                        let url = &after[mid + 2..mid + end];
                        let mut shown = words.to_string();
                        if !url.is_empty() && url != words {
                            shown.push_str(&format!(" ({url})"));
                        }
                        handled = Some((shown, body, &after[mid + end + 1..]));
                    }
                }
            }
        }
        match handled {
            Some((said, style, after)) => {
                if !plain.is_empty() {
                    out.push(Span::styled(std::mem::take(&mut plain), body));
                }
                out.push(Span::styled(said, style));
                rest = after;
            }
            None => {
                plain.push(c);
                rest = &rest[c.len_utf8()..];
            }
        }
    }
    if !plain.is_empty() {
        out.push(Span::styled(plain, body));
    }
    out
}

/// Spans to rows, breaking at a space where the row holds one and hard at the
/// edge where it does not: prose breaks between words, and a language without
/// spaces breaks at the column. A row is never wider than `width`, which is
/// what keeps a wide character from being cut in half by the terminal.
fn wrap(spans: Vec<Span<'static>>, width: usize) -> Vec<Vec<Span<'static>>> {
    let width = width.max(1);
    let mut cells: Vec<(char, Style)> = Vec::new();
    for span in &spans {
        for c in span.content.chars() {
            cells.push((c, span.style));
        }
    }
    let mut rows: Vec<Vec<(char, Style)>> = Vec::new();
    let mut start = 0;
    while start < cells.len() {
        let mut end = start;
        let mut wide = 0;
        let mut last_space: Option<usize> = None;
        while end < cells.len() {
            let cell = UnicodeWidthChar::width(cells[end].0).unwrap_or(0);
            if wide + cell > width {
                break;
            }
            if cells[end].0 == ' ' {
                last_space = Some(end);
            }
            wide += cell;
            end += 1;
        }
        if end == cells.len() {
            rows.push(cells[start..].to_vec());
            break;
        }
        if let Some(space) = last_space.filter(|space| *space > start) {
            rows.push(cells[start..space].to_vec());
            start = space + 1;
            continue;
        }
        let stop = end.max(start + 1);
        rows.push(cells[start..stop].to_vec());
        start = stop;
    }
    if rows.is_empty() {
        rows.push(Vec::new());
    }
    rows.into_iter()
        .map(|row| {
            let mut spans: Vec<Span<'static>> = Vec::new();
            for (c, style) in row {
                match spans.last_mut() {
                    Some(last) if last.style == style => last.content.to_mut().push(c),
                    _ => spans.push(Span::styled(c.to_string(), style)),
                }
            }
            spans
        })
        .collect()
}
