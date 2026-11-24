//! fit --- what a narrow window keeps, and what it admits it dropped.
//!
//! A row cut mid-word is worse than a row with fewer items: `ctx` without its
//! number and `b b` without its key look like readings and are not. So a row
//! yields whole items, in order of importance, and says that it yielded.

/// Join as many whole items as fit, least important last. When any are dropped
/// the row ends with `…`, because a silently shortened row claims to be whole.
pub fn fit(items: &[String], width: usize, sep: &str) -> String {
    let mut out = String::new();
    let mut dropped = false;
    for (i, item) in items.iter().enumerate() {
        let piece = if i == 0 { item.clone() } else { format!("{sep}{item}") };
        // Room for the marker is reserved from the first item on, so admitting
        // the drop never pushes the row over the edge itself.
        let reserve = if i + 1 < items.len() { 1 } else { 0 };
        if out.chars().count() + piece.chars().count() + reserve > width {
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
    let room = width.saturating_sub(tail.chars().count());
    let head = fit(items, room, sep);
    format!("{head}{tail}")
}

/// Shorten a path from the left, keeping the tail: the last components are what
/// identify the directory, and a prefix of `~/Projects/…` identifies nothing.
pub fn tail_path(path: &str, width: usize) -> String {
    if path.chars().count() <= width {
        return path.to_string();
    }
    let keep = width.saturating_sub(1);
    let tail: String = path.chars().rev().take(keep).collect::<Vec<_>>().into_iter().rev().collect();
    format!("…{tail}")
}
