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
use pearl_tools::{Own, copy_workspace_except};

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
    own: Own,
}

impl SubagentPool {
    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            own: Own::new(),
        }
    }

    /// Share the parent's notion of pearl's own state, so a child's copy holds
    /// the human's files and not the parent's conversation.
    pub fn with_own(mut self, own: Own) -> Self {
        self.own = own;
        self
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
            // The child's log sits beside the parent's, which is usually inside
            // the parent's workspace: name it before any baseline is taken.
            self.own.add(&session);
            self.own.add(session.with_extension("lock"));
            let mut copy: Option<PathBuf> = None;
            if let Some(src) = workspace {
                // Beside the workspace, never inside it: a copy within the
                // source would copy itself, and the parent's baseline would
                // snapshot the children's work as if it were the human's.
                let beside = src.parent().unwrap_or(src);
                let dest = beside.join(".pearl-children").join(child.to_string());
                copy_workspace_except(src, &dest, &self.own)
                    .map_err(|e| AgentError::Workspace(e.to_string()))?;
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
