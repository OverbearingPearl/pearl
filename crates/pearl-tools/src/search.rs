//! search --- find the lines that name a thing, without running a shell.
//!
//! A log's messages were mostly `grep` through the shell: 166 invocations in
//! one log, 54 of them asking for line numbers and 16 walking the tree. Every
//! one of those paid for a whole-workspace baseline before it ran, which a
//! command that only looks cannot need, and then for a subprocess to do the
//! looking. This does the looking here.
//!
//! The pattern is text, not a regular expression, and it says so rather than
//! quietly matching the wrong thing: a leading `^` is honoured because most of
//! the greps in that log were anchors (`^fn`, `^## `), and anything else that
//! only a regex engine understands is refused with the door that does it.

use crate::tool::ToolError;
use std::io::BufRead;
use std::path::{Path, PathBuf};

/// Directories never walked: version control and build output. The same list a
/// baseline skips, for the same reason -- they are not the human's work.
const SKIP: &[&str] = &[".git", "target", "node_modules", "dist", ".cache"];

/// Matches one search returns before it stops.
const MATCHES: usize = 200;
/// Bytes one search puts in the conversation.
const BYTES: usize = 32 * 1024;
/// Characters of a matching line shown. A hit in a minified bundle is a line
/// of a hundred thousand characters and the match is somewhere inside it.
const LINE_CHARS: usize = 400;
/// Files larger than this are not read: a search is for source, and a data
/// file of tens of megabytes would be read line by line to find nothing.
const FILE_BYTES: u64 = 2 * 1024 * 1024;

/// What a search found.
#[derive(Debug, Default)]
pub struct Found {
    /// `path:line:text`, in the order the tree was walked.
    pub hits: Vec<String>,
    /// How many matches there were, including the ones not shown.
    pub total: usize,
    /// How many files were read.
    pub files: usize,
}

/// Whether a pattern is text this can search, and what it means.
fn reading(pattern: &str) -> Result<(bool, &str), ToolError> {
    let (anchored, rest) = match pattern.strip_prefix('^') {
        Some(rest) => (true, rest),
        None => (false, pattern),
    };
    if rest.is_empty() {
        return Err(ToolError::Rejected(String::from(
            "search needs something to look for",
        )));
    }
    // A dot is left alone: `queue.get` and `Cargo.toml` are text people search
    // for far more often than they are a wildcard, and refusing them would
    // send most searches back to the shell this exists to replace.
    if rest.chars().any(|c| {
        matches!(
            c,
            '$' | '[' | ']' | '(' | ')' | '|' | '*' | '+' | '?' | '{' | '}' | '\\'
        )
    }) {
        return Err(ToolError::Rejected(format!(
            "this searches text, not a regular expression ({pattern}); \
             use bash grep for a pattern"
        )));
    }
    Ok((anchored, rest))
}

/// Find the lines naming `pattern` under `root`, or under `within` when given.
pub fn lines(
    root: &Path,
    within: Option<&str>,
    pattern: &str,
    ignore_case: bool,
) -> Result<Found, ToolError> {
    let (anchored, text) = reading(pattern)?;
    let needle = if ignore_case {
        text.to_lowercase()
    } else {
        text.to_string()
    };
    let start = match within {
        Some(rel) => {
            if Path::new(rel).is_absolute() || rel.split('/').any(|c| c == "..") {
                return Err(ToolError::Rejected(format!(
                    "search stays inside the workspace: {rel}"
                )));
            }
            root.join(rel)
        }
        None => root.to_path_buf(),
    };
    let mut found = Found::default();
    let mut held = 0usize;
    let mut stack = vec![start];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            // A file rather than a directory is a search of one file, which is
            // what `grep pattern file` was asking for.
            if dir.is_file() {
                read_one(
                    root,
                    &dir,
                    &needle,
                    anchored,
                    ignore_case,
                    &mut found,
                    &mut held,
                );
            }
            continue;
        };
        let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
        paths.sort();
        for path in paths {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            if path.is_dir() {
                if !SKIP.contains(&name.as_str()) {
                    stack.push(path);
                }
                continue;
            }
            read_one(
                root,
                &path,
                &needle,
                anchored,
                ignore_case,
                &mut found,
                &mut held,
            );
        }
    }
    Ok(found)
}

#[allow(clippy::too_many_arguments)]
fn read_one(
    root: &Path,
    path: &Path,
    needle: &str,
    anchored: bool,
    ignore_case: bool,
    found: &mut Found,
    held: &mut usize,
) {
    let Ok(meta) = path.metadata() else { return };
    if meta.len() > FILE_BYTES {
        return;
    }
    let Ok(file) = std::fs::File::open(path) else {
        return;
    };
    found.files += 1;
    let rel = path
        .strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string();
    let mut reader = std::io::BufReader::new(file);
    let mut number = 0usize;
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {}
            // A file that is not text is not a search's business, and one bad
            // byte does not make the whole search fail.
            Err(_) => return,
        }
        number += 1;
        while line.ends_with('\n') || line.ends_with('\r') {
            line.pop();
        }
        let hay = if ignore_case {
            line.to_lowercase()
        } else {
            line.clone()
        };
        let hit = if anchored {
            hay.starts_with(needle)
        } else {
            hay.contains(needle)
        };
        if !hit {
            continue;
        }
        found.total += 1;
        if found.hits.len() >= MATCHES || *held >= BYTES {
            continue;
        }
        let shown: String = if line.chars().count() > LINE_CHARS {
            let cut: String = line.chars().take(LINE_CHARS).collect();
            format!("{cut}\u{2026}")
        } else {
            line
        };
        let row = format!("{rel}:{number}:{shown}");
        *held += row.len();
        found.hits.push(row);
    }
}
