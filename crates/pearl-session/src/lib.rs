//! pearl-session --- the session log and compaction-as-projection.
//!
//! History is an append-only JSONL log with one writer; a kill and reopen finds
//! it where it stopped. Compaction is a view over that log, never a rewrite, so
//! the real history is always recoverable and the task queue is never folded.

pub mod compaction;
pub mod entry;
pub mod store;
pub mod tokens;

pub use compaction::{
    Budget, FoldSummary, Projection, Split, Summarize, project, split, view, view_in,
    view_in_owned,
};
pub use entry::{Coverage, Entry, Role};
pub use store::{SessionStore, StoreError};
pub use tokens::{Meter, estimate_text};
