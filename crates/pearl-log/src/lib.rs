//! pearl-log --- the log, and folding as a projection over it.
//!
//! History is an append-only JSONL log with one writer; a kill and reopen finds
//! it where it stopped. A fold is a view over that log, never a rewrite, so
//! the real history is always recoverable and the task queue is never folded.

pub mod entry;
pub mod fold;
pub mod store;
pub mod tokens;

pub use entry::{Coverage, Entry, Role};
pub use fold::{
    Budget, FoldSummary, Projection, Split, Summarize, fold_results, project, split, view, view_in,
    view_in_owned,
};
pub use store::{Store, StoreError};
pub use tokens::{Meter, estimate_text};
