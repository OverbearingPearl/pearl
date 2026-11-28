//! scheduler --- the event-loop policy that drives the queue.
//!
//! The queue holds the data and the legal transitions; the scheduler decides
//! what runs next and when steering is injected. It is deliberately thin: the
//! guarantees live in the queue's invariants, not here.

use crate::event::Event;
use crate::id::TaskId;
use crate::queue::Queue;

/// Dispatch policy over a queue. The loop calls these at the points where
/// a transition is safe.
#[derive(Debug, Default)]
pub struct Scheduler;

impl Scheduler {
    pub fn new() -> Self {
        Self
    }

    /// Start the next queued top-level task, if any. Returns the task and the
    /// events its dispatch produced.
    pub fn dispatch_next(&self, queue: &mut Queue) -> Option<(TaskId, Vec<Event>)> {
        let id = queue.next_queued()?;
        let events = queue.start(id).ok()?;
        Some((id, events))
    }

    /// Drain pending steering at a safe boundary. The loop calls this between
    /// tool calls, never mid-token; each drained message becomes an injection.
    pub fn at_boundary(&self, queue: &mut Queue, id: TaskId) -> Vec<Event> {
        queue.drain_steering(id).unwrap_or_default()
    }
}
