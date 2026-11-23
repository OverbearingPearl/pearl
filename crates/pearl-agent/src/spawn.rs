//! spawn --- a node fans out into derived sessions, each in its own copy.
//!
//! A subagent is a child task with its own session log beside the parent's and
//! its own copy of the workspace. The copy is what makes the children parallel:
//! two children editing the same file cannot collide, because they are not
//! editing the same file. Only the settled result flows back to the queue, so a
//! subagent's context never pollutes the parent's.

use std::path::PathBuf;

use pearl_core::{Queue, TaskId};
use pearl_session::SessionStore;
use pearl_tools::copy_workspace;

use crate::agent::AgentError;

/// A spawned child, the derived session it runs in, and the copy it works on.
#[derive(Debug, Clone)]
pub struct Spawned {
    pub child: TaskId,
    pub session: PathBuf,
    pub workspace: PathBuf,
}

/// Creates child tasks under a parent, each with a derived session and a
/// workspace copy.
pub struct SubagentPool {
    dir: PathBuf,
}

impl SubagentPool {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    /// Fan `parent` out into one child per title. Each child gets a session
    /// file `parent-<child>.jsonl` beside the parent's log, created empty so
    /// the subagent has its own context from the first append, and a copy of
    /// `workspace` under `children/<child>/` so its writes are its own.
    pub fn spawn(
        &self,
        queue: &mut Queue,
        parent: TaskId,
        titles: &[impl AsRef<str>],
        workspace: Option<&std::path::Path>,
    ) -> Result<Vec<Spawned>, AgentError> {
        let mut out = Vec::new();
        for title in titles {
            let (child, _) = queue
                .add_child(parent, title.as_ref())
                .map_err(|e| AgentError::Queue(e.to_string()))?;
            let session = self.dir.join(format!("parent-{child}.jsonl"));
            SessionStore::open(&session)?;
            let mut copy: Option<PathBuf> = None;
            if let Some(src) = workspace {
                let dest = self.dir.join("children").join(child.to_string());
                copy_workspace(src, &dest).map_err(|e| AgentError::Workspace(e.to_string()))?;
                copy = Some(dest);
            }
            out.push(Spawned {
                child,
                session,
                workspace: copy.unwrap_or_default(),
            });
        }
        Ok(out)
    }
}
