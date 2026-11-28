//! fit --- what a narrow window keeps, and what it admits it dropped.
//!
//! A row cut mid-word is worse than a row with fewer items: `ctx` without its
//! number and `b b` without its key look like readings and are not. So a row
//! yields whole items, in order of importance, and says that it yielded.
//!
//! Every measurement here is in display columns, not characters: a title
//! written in Chinese draws two cells per character, and a row fitted by
//! counting characters runs past the glass and is clipped by the terminal
//! without a mark -- see ADR 21.

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// How many columns a piece of text takes on the glass.
pub fn width_of(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

/// Join as many whole items as fit, least important last. When any are dropped
/// the row ends with `…`, because a silently shortened row claims to be whole.
pub fn fit(items: &[String], width: usize, sep: &str) -> String {
    let mut out = String::new();
    let mut dropped = false;
    for (i, item) in items.iter().enumerate() {
        let piece = if i == 0 {
            item.clone()
        } else {
            format!("{sep}{item}")
        };
        // Room for the marker is reserved from the first item on, so admitting
        // the drop never pushes the row over the edge itself.
        let reserve = if i + 1 < items.len() { 1 } else { 0 };
        if width_of(&out) + width_of(&piece) + reserve > width {
            dropped = true;
            break;
        }
        out.push_str(&piece);
    }
    if dropped {
        out.push('…');
    }
    out
}

/// Fit everything but a tail that must survive: the keys for help and for
/// leaving are not something a narrow window may drop.
pub fn fit_keeping(items: &[String], width: usize, sep: &str, tail: &str) -> String {
    let room = width.saturating_sub(width_of(tail));
    let head = fit(items, room, sep);
    format!("{head}{tail}")
}

/// The beginning of one piece of text, cut where the glass ends.
///
/// No room is no text. The mark that says something was cut costs a column of
/// its own, and spending one column on a mark in a place where there was no
/// column is how a four-column window took the screen down.
pub fn head(text: &str, room: usize) -> String {
    if room == 0 {
        return String::new();
    }
    if width_of(text) <= room {
        return text.to_string();
    }
    let keep = room.saturating_sub(1);
    let mut out = String::new();
    for c in text.chars() {
        let w = UnicodeWidthChar::width(c).unwrap_or(0);
        if width_of(&out) + w > keep {
            break;
        }
        out.push(c);
    }
    out.push('…');
    out
}

/// The end of one piece of text, cut where the glass begins. What a human is
/// typing is worth seeing at the end of the line, not at the start of it.
pub fn tail(text: &str, room: usize) -> String {
    if room == 0 {
        return String::new();
    }
    if width_of(text) <= room {
        return text.to_string();
    }
    let keep = room.saturating_sub(1);
    let mut taken: Vec<char> = Vec::new();
    let mut held = 0;
    for c in text.chars().rev() {
        let w = UnicodeWidthChar::width(c).unwrap_or(0);
        if held + w > keep {
            break;
        }
        held += w;
        taken.push(c);
    }
    let tail: String = taken.into_iter().rev().collect();
    format!("…{tail}")
}

/// Both ends of one piece of text, cut where the glass begins. A note about a
/// command begins with the tool that ran it and ends with what the command
/// did, and the head of it alone is often the same forty characters every
/// round: what is worth keeping is the shape of the whole, not its left edge.
pub fn middle(text: &str, room: usize) -> String {
    if room == 0 {
        return String::new();
    }
    if width_of(text) <= room {
        return text.to_string();
    }
    let keep = room.saturating_sub(1);
    let head_room = keep / 2;
    let tail_room = keep - head_room;
    let mut out = String::new();
    for c in text.chars() {
        let w = UnicodeWidthChar::width(c).unwrap_or(0);
        if width_of(&out) + w > head_room {
            break;
        }
        out.push(c);
    }
    let mut taken: Vec<char> = Vec::new();
    let mut held = 0;
    for c in text.chars().rev() {
        let w = UnicodeWidthChar::width(c).unwrap_or(0);
        if held + w > tail_room {
            break;
        }
        held += w;
        taken.push(c);
    }
    out.push('\u{2026}');
    out.extend(taken.into_iter().rev());
    out
}

/// A prompt keeps its label and yields the middle of what is typed: the label
/// says which act is being typed, and the end of the line is where the typing
/// is. A prompt that hides the label is a prompt you have to leave to identify.
pub fn label_keeping(label: &str, body: &str, width: usize) -> String {
    let room = width.saturating_sub(width_of(label));
    format!("{label}{}", tail(body, room))
}

/// Shorten a path from the left, keeping the tail: the last components are what
/// identify the directory, and a prefix of `~/Projects/…` identifies nothing.
pub fn tail_path(path: &str, width: usize) -> String {
    tail(path, width)
}
