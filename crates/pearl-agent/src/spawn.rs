//! spawn --- a node fans out into derived sessions.
//!
//! A subagent is a child task with its own session log beside the parent's. The
//! children are the parallel execution; the human keeps one focus on the
//! parent. Only the settled result flows back to the queue, so a subagent's
//! context never pollutes the parent's.

use std::path::PathBuf;

use pearl_core::{Queue, TaskId};
use pearl_session::SessionStore;

use crate::agent::AgentError;

/// A spawned child and the derived session it runs in.
#[derive(Debug, Clone)]
pub struct Spawned {
    pub child: TaskId,
    pub session: PathBuf,
}

/// Creates child tasks under a parent, each with a derived session file.
pub struct SubagentPool {
    dir: PathBuf,
}

impl SubagentPool {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    /// Fan `parent` out into one child per title. Each child gets a session
    /// file `parent-<child>.jsonl` beside the parent's log, created empty so
    /// the subagent has its own context from the first append.
    pub fn spawn(
        &self,
        queue: &mut Queue,
        _parent_store: &SessionStore,
        parent: TaskId,
        titles: &[impl AsRef<str>],
    ) -> Result<Vec<Spawned>, AgentError> {
        let mut out = Vec::new();
        for title in titles {
            let (child, _) = queue.add_child(parent, title.as_ref()).map_err(|e| AgentError::Queue(e.to_string()))?;
            let session = self.dir.join(format!("parent-{child}.jsonl"));
            SessionStore::open(&session)?;
            out.push(Spawned { child, session });
        }
        Ok(out)
    }
}
