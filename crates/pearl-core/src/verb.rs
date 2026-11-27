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
    /// Join a line of work: become a topic, or go inside one.
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
    /// them has run, because a turn that ran under a topic belongs to it.
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
    ///
    /// `state` is the state the work is in, which for a place is not the state
    /// written on it: a topic runs no turn of its own, so a part in flight
    /// leaves it `Queued` while the line is plainly under way. The tree reads
    /// that before it asks, so `Dissolve` here means "nothing inside has run"
    /// instead of a guess about it.
    pub fn accepts_form(self, state: NodeState, held: bool, form: Form) -> bool {
        use NodeState::*;
        if form == Form::Topic {
            return match self {
                Verb::Hold => matches!(state, Queued | Running),
                Verb::Run => matches!(state, Paused) || (state == Queued && held),
                // A line is stopped whole the way a task is: what cannot be
                // cancelled is what has already settled, and that row's
                // reading is `Drop`.
                Verb::Cancel => !settled(state),
                Verb::Drop => settled(state),
                Verb::Dissolve => state == Queued,
                // A part that has run can still leave one topic for another
                // -- its turns move with the topic -- but a topic cannot
                // join a topic: membership is one level deep.
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
            // A topic takes parts too: nesting under one is how a line of
            // work grows without going back up to the root first.
            // A finished task may take a part: `n` on a row is a list edit,
            // and the list is the human's to rearrange whatever the rows say.
            // Only work in flight is untouchable, because something is using
            // it, and its shape is what the runner is answering.
            Verb::Nest => !matches!(state, AwaitingInput),
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
            Verb::Take => "a queued, paused or finished row",
            Verb::Steer => "a queued, running, paused or waiting row",
            Verb::Rewrite => "a queued, running, paused or waiting row",
            Verb::Nest => "a task that is not waiting on you",
            Verb::Cancel => "a queued, running, paused or waiting row",
            Verb::Hold => "a queued or running row",
            Verb::Run => "a paused or held row",
            Verb::Drop => "a finished row",
            Verb::Dissolve => "a topic whose parts have not run",
            Verb::Join => "a row that has not run, or one that ran on the shared line",
            Verb::Retry => "a finished row",
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
            // the exception: a task becomes a topic, and a topic never
            // joins another.
            let live = states
                .iter()
                .any(|s| verb.accepts(*s, false) || verb.accepts_form(*s, false, Form::Topic));
            let stopped = verb.accepts(Queued, true) != verb.accepts(Queued, false);
            assert!(live || stopped, "{verb:?} would never be offered");
        }
    }
}
