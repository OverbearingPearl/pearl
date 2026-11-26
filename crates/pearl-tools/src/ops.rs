//! ops --- the ledger of writes that can still be taken back.
//!
//! A write is only proved where a snapshot can take it back (ADR 16), and the
//! promise is the human's, not the model's: taking a write back must not need
//! the model's cooperation or a running turn. So the ledger is a handle, like
//! the queue and the settings -- the runner records into it and the surface
//! restores from it, without asking the loop for permission.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use crate::baseline::Baseline;

/// What an op left behind: a shell command's whole-tree before-image, or one
/// file's two sides. A file is kept as its own pair so taking it back needs no
/// tree at all, and so a second edit of the same file is not silently undone
/// along with the first (ADR 28).
#[derive(Debug)]
enum Entry {
    Shell {
        task: String,
        label: String,
        baseline: Baseline,
    },
    File {
        task: String,
        label: String,
        path: PathBuf,
        before: Option<Vec<u8>>,
        after: Vec<u8>,
    },
}

impl Entry {
    fn label(&self) -> &str {
        match self {
            Entry::Shell { label, .. } | Entry::File { label, .. } => label,
        }
    }

    fn task(&self) -> &str {
        match self {
            Entry::Shell { task, .. } | Entry::File { task, .. } => task,
        }
    }
}

#[derive(Debug, Default)]
struct Ledger {
    entries: HashMap<u64, Entry>,
    /// The next id, kept here rather than in a tool registry: two tasks run on
    /// two registries over one ledger, and ids that restart at zero would let
    /// one task's op 0 stand in for the other's.
    next: u64,
}

/// The ops that can still be undone, shared by whoever writes and whoever
/// takes it back.
#[derive(Debug, Clone, Default)]
pub struct Ops {
    ledger: Arc<Mutex<Ledger>>,
}

impl Ops {
    pub fn new() -> Self {
        Self::default()
    }

    fn ledger(&self) -> MutexGuard<'_, Ledger> {
        self.ledger.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The next free op id, so a writer never has to own a counter.
    pub fn next_op(&self) -> u64 {
        let mut ledger = self.ledger();
        let op = ledger.next;
        ledger.next += 1;
        op
    }

    /// Record a shell op's before-image and the command that produced it.
    pub fn record(
        &self,
        op: u64,
        task: impl Into<String>,
        command: impl Into<String>,
        baseline: Baseline,
    ) {
        self.ledger().entries.insert(
            op,
            Entry::Shell {
                task: task.into(),
                label: command.into(),
                baseline,
            },
        );
    }

    /// Record one file's two sides, so an edit or a write can be taken back
    /// like a command. `before` is `None` when the tool created the file.
    pub fn record_file(
        &self,
        op: u64,
        task: impl Into<String>,
        label: impl Into<String>,
        path: PathBuf,
        before: Option<Vec<u8>>,
        after: Vec<u8>,
    ) {
        self.ledger().entries.insert(
            op,
            Entry::File {
                task: task.into(),
                label: label.into(),
                path,
                before,
                after,
            },
        );
    }

    /// The live ops, oldest first, with what they were.
    pub fn list(&self) -> Vec<(u64, String)> {
        let ledger = self.ledger();
        let mut ops: Vec<(u64, String)> = ledger
            .entries
            .iter()
            .map(|(id, entry)| (*id, entry.label().to_string()))
            .collect();
        ops.sort_by_key(|(id, _)| *id);
        ops
    }

    /// Restore one op's before-image. The entry is consumed: an op is taken
    /// back once, and a second attempt says there is nothing there.
    pub fn revert(&self, op: u64) -> std::io::Result<usize> {
        match self.ledger().entries.remove(&op) {
            Some(Entry::Shell { baseline, .. }) => baseline.restore(),
            Some(Entry::File {
                path,
                before,
                after,
                ..
            }) => {
                // The file is the human's too. If it no longer holds what this
                // op wrote, someone wrote over it afterwards, and restoring the
                // older bytes would take their work with it.
                let now = std::fs::read(&path).unwrap_or_default();
                if now != after {
                    return Err(std::io::Error::other(format!(
                        "{} changed after that write, so it was left alone",
                        path.display()
                    )));
                }
                match before {
                    Some(bytes) => std::fs::write(&path, bytes)?,
                    None => {
                        if path.exists() {
                            std::fs::remove_file(&path)?;
                        }
                    }
                }
                Ok(1)
            }
            None => Ok(0),
        }
    }

    /// What one task touched, as words a screen can show: its files by op,
    /// oldest first, and how many commands it ran whose writes are only
    /// visible as a whole-tree image (ADR 28 leaves those unlisted per file).
    pub fn report(&self, task: &str) -> Option<String> {
        let ledger = self.ledger();
        let mut ops: Vec<(u64, &Entry)> = ledger
            .entries
            .iter()
            .filter(|(_, entry)| entry.task() == task)
            .map(|(id, entry)| (*id, entry))
            .collect();
        ops.sort_by_key(|(id, _)| *id);
        if ops.is_empty() {
            return None;
        }
        let mut out = String::from("what this task touched\n\n");
        let mut commands = 0;
        for (id, entry) in &ops {
            match entry {
                Entry::File { path, .. } => out.push_str(&format!("op {id}  {}\n", path.display())),
                Entry::Shell { label, .. } => {
                    commands += 1;
                    out.push_str(&format!("op {id}  {label}\n"));
                }
            }
        }
        if commands > 0 {
            out.push_str(&format!(
                "\n{commands} command(s) ran; a command may have written anywhere, and its op is undone as a whole tree\n",
            ));
        }
        let files = ops
            .iter()
            .filter(|(_, e)| matches!(e, Entry::File { .. }))
            .count();
        out.push_str(&format!("\n{files} file(s), {commands} command(s)\n"));
        Some(out)
    }

    /// Take the newest op back, naming what it was so the human can match it
    /// against what they saw. `None` when nothing is left to undo.
    /// What other work in this workspace has touched since `own` last did.
    ///
    /// A topic scopes the conversation, not the files: two lines of work share
    /// one tree, so a task that starts after another line of work has written
    /// needs to be told which paths moved, or it will read a file as it was.
    pub fn elsewhere(&self, own: &[String]) -> Option<String> {
        let ledger = self.ledger();
        let mut mine = 0;
        let mut paths: Vec<&PathBuf> = Vec::new();
        for (id, entry) in &ledger.entries {
            if own.iter().any(|task| task == entry.task()) {
                mine = mine.max(*id);
            }
        }
        for (id, entry) in &ledger.entries {
            if *id <= mine || own.iter().any(|task| task == entry.task()) {
                continue;
            }
            if let Entry::File { path, .. } = entry {
                if !paths.contains(&path) {
                    paths.push(path);
                }
            }
        }
        if paths.is_empty() {
            return None;
        }
        let names: Vec<String> = paths.iter().map(|p| p.display().to_string()).collect();
        Some(format!(
            "elsewhere in this workspace, other work changed {}",
            names.join(", ")
        ))
    }

    pub fn revert_last(&self) -> std::io::Result<Option<String>> {
        let Some((op, command)) = self.list().last().cloned() else {
            return Ok(None);
        };
        let restored = self.revert(op)?;
        Ok(Some(format!(
            "undone op {op} ({command}): {restored} path(s) restored"
        )))
    }
}
