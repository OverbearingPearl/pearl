//! pearl-session --- the session log and compaction-as-projection.
//!
//! History is an append-only JSONL log with one writer; a kill and reopen finds
//! it where it stopped. Compaction is a view over that log, never a rewrite, so
//! the real history is always recoverable and the task queue is never folded.

pub mod compaction;
pub mod entry;
pub mod store;

pub use compaction::{FoldSummary, Projection, Summarize, project};
pub use entry::{Entry, Role};
pub use store::{SessionStore, StoreError};
