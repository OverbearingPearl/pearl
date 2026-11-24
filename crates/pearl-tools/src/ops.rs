//! ops --- the ledger of writes that can still be taken back.
//!
//! A write is only proved where a snapshot can take it back (ADR 16), and the
//! promise is the human's, not the model's: taking a write back must not need
//! the model's cooperation or a running turn. So the ledger is a handle, like
//! the queue and the settings -- the runner records into it and the surface
//! restores from it, without asking the loop for permission.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use crate::baseline::Baseline;

#[derive(Debug, Default)]
struct Ledger {
    /// The before-image of each op, and what the op was.
    entries: HashMap<u64, (String, Baseline)>,
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

    /// Record an op's before-image and the command that produced it.
    pub fn record(&self, op: u64, command: impl Into<String>, baseline: Baseline) {
        self.ledger().entries.insert(op, (command.into(), baseline));
    }

    /// The live ops, oldest first, with what they were.
    pub fn list(&self) -> Vec<(u64, String)> {
        let ledger = self.ledger();
        let mut ops: Vec<(u64, String)> =
            ledger.entries.iter().map(|(id, (cmd, _))| (*id, cmd.clone())).collect();
        ops.sort_by_key(|(id, _)| *id);
        ops
    }

    /// Restore one op's before-image. The entry is consumed: an op is taken
    /// back once, and a second attempt says there is nothing there.
    pub fn revert(&self, op: u64) -> std::io::Result<usize> {
        match self.ledger().entries.remove(&op) {
            Some((_, baseline)) => baseline.restore(),
            None => Ok(0),
        }
    }

    /// Take the newest op back, naming what it was so the human can match it
    /// against what they saw. `None` when nothing is left to undo.
    pub fn revert_last(&self) -> std::io::Result<Option<String>> {
        let Some((op, command)) = self.list().last().cloned() else {
            return Ok(None);
        };
        let restored = self.revert(op)?;
        Ok(Some(format!("undone op {op} ({command}): {restored} path(s) restored")))
    }
}
