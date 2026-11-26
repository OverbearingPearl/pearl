//! Which verbs a task can take, in one place.
//!
//! The queue answers a verb with a refusal that names the states it wanted, and
//! the row at the bottom of the screen answers the same question by showing the
//! key only when it would work. Written twice, the two answers drift: a row
//! that offers `s` over a finished task, a refusal that says "needs queued or
//! running" where the row said nothing of the kind. Both read this table.

use crate::node::{Form, NodeState};
use crate::{QueueError, TaskId};

/// A move the human can make on one task.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verb {
    /// Lift the words out of the queue and into the human's hand.
    Take,
    /// Say something to a task that is running, without stopping it.
    Steer,
    /// Put different words where the task's words are.
    Rewrite,
    /// Hang a new task under this one.
    Nest,
    /// Join a line of work: become a heading, or go inside one.
    Join,
    /// Stop a live task and take it off the queue.
    Cancel,
    /// Stop a live task where it is, keeping its place.
    Hold,
    /// Let a stopped task go on.
    Run,
    /// Take a settled task's line away.
    Drop,
    /// Take a topic apart: its parts stand where it stood. Only while none of
    /// them has run, because a turn that ran under a heading belongs to it.
    Dissolve,
    /// Start a settled task again, with the words it already has.
    Retry,
}

/// A finished task: its answer is written, or its work was given up.
fn settled(state: NodeState) -> bool {
    matches!(
        state,
        NodeState::Done | NodeState::Failed | NodeState::Cancelled
    )
}

impl Verb {
    /// Whether this verb would be accepted, given the state a task is in.
    ///
    /// A held task is a queued task nobody will pick up: it can be run, moved
    /// or dropped, but `Ready` is what the scheduler reads, so `accepts` reads
    /// the same two facts the queue's arms read.
    pub fn accepts(self, state: NodeState, held: bool) -> bool {
        self.accepts_form(state, held, Form::Task)
    }

    /// The same question with the node's form in hand: a topic is a place, and
    /// the verbs that ask it to have words or do work do not apply to it.
    pub fn accepts_form(self, state: NodeState, held: bool, form: Form) -> bool {
        use NodeState::*;
        if form == Form::Topic {
            return match self {
                Verb::Hold => matches!(state, Queued | Running),
                Verb::Run => matches!(state, Paused) || (state == Queued && held),
                Verb::Cancel | Verb::Drop => settled(state),
                Verb::Dissolve => state == Queued,
                // A part that has run can still leave one heading for another
                // -- its turns move with the topic -- but a heading cannot
                // join a heading: membership is one level deep.
                Verb::Join => false,
                _ => false,
            };
        }
        match self {
            // A finished row can be moved and dropped: the list is the human's
            // to rearrange, and a task that ran is still a row on it. Only a
            // task in flight is untouchable, because something is using it.
            Verb::Take => !matches!(state, Running | AwaitingInput),
            Verb::Steer => matches!(state, Queued | Running | Paused | AwaitingInput),
            Verb::Rewrite => matches!(state, Queued | Running | Paused | AwaitingInput),
            // A heading takes parts too: nesting under one is how a line of
            // work grows without going back up to the root first.
            Verb::Nest => matches!(state, Queued | Running | Paused),
            // Only work that has not started can join a line: its turns would
            // otherwise belong to the conversation it already had.
            Verb::Join => state == Queued,
            Verb::Cancel => !settled(state),
            Verb::Hold => matches!(state, Queued | Running),
            Verb::Run => state == Paused || (state == Queued && held),
            Verb::Drop => settled(state),
            Verb::Dissolve => false,
            Verb::Retry => settled(state),
        }
    }

    /// The states the refusal names, in the order a human reads them.
    ///
    /// One string, used by every arm: the surface's `refused` line and the
    /// queue's own error are then the same sentence about the same rule.
    pub fn expected(self) -> &'static str {
        match self {
            Verb::Take => "queued, paused, or finished",
            Verb::Steer => "queued, running, paused or awaiting-input",
            Verb::Rewrite => "queued, running, paused or awaiting-input",
            Verb::Nest => "queued, running or paused",
            Verb::Cancel => "queued, running, paused or awaiting-input",
            Verb::Hold => "queued or running",
            Verb::Run => "paused or held-queued",
            Verb::Drop => "done, failed or cancelled",
            Verb::Dissolve => "a topic whose parts have not run",
            Verb::Join => "a task that is not in flight, or one that ran on the shared line",
            Verb::Retry => "done, failed or cancelled",
        }
    }

    /// The refusal, when the verb does not apply.
    pub fn wrong_state(self, id: TaskId, state: NodeState) -> QueueError {
        QueueError::WrongState {
            id,
            found: state,
            expected: self.expected(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_verb_names_a_state_it_accepts() {
        use NodeState::*;
        for verb in [
            Verb::Take,
            Verb::Steer,
            Verb::Rewrite,
            Verb::Nest,
            Verb::Cancel,
            Verb::Hold,
            Verb::Run,
            Verb::Drop,
            Verb::Dissolve,
            Verb::Join,
            Verb::Retry,
        ] {
            let states = [
                Queued,
                Running,
                AwaitingInput,
                Paused,
                Done,
                Failed,
                Cancelled,
            ];
            // A topic has no lifecycle of its own, so the form is asked too: a
            // verb can be offered on one form and never on the other. Join is
            // the exception: a task becomes a heading, and a heading never
            // joins another.
            let live = states
                .iter()
                .any(|s| verb.accepts(*s, false) || verb.accepts_form(*s, false, Form::Topic));
            let stopped = verb.accepts(Queued, true) != verb.accepts(Queued, false);
            assert!(live || stopped, "{verb:?} would never be offered");
        }
    }
}
