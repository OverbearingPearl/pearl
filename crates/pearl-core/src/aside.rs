//! aside --- a side question that is not work.
//!
//! An aside is the `/btw` move: a quick question asked while the queue runs,
//! answered against the log as read-only context. It is a single loose
//! node outside the tree: the scheduler never dispatches it, it has no
//! children, and nothing about it enters the log. It lives on the
//! surface until dismissed.

/// Where an aside is in its short life.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsideState {
    /// The question is with the model.
    Asking,
    /// The answer came back.
    Answered,
    /// The ask failed; the reason is in `answer`.
    Failed,
}

/// A side question and its answer, held outside the queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aside {
    pub question: String,
    pub answer: Option<String>,
    pub state: AsideState,
}

/// What an aside refused to do.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AsideError {
    #[error("aside is {0:?}, not Asking")]
    NotAsking(AsideState),
}

impl Aside {
    /// A fresh question, on its way to the model.
    pub fn new(question: impl Into<String>) -> Self {
        Self {
            question: question.into(),
            answer: None,
            state: AsideState::Asking,
        }
    }

    /// Settle the aside with its answer. Only an asking aside can be answered.
    pub fn answer(&mut self, text: impl Into<String>) -> Result<(), AsideError> {
        if self.state != AsideState::Asking {
            return Err(AsideError::NotAsking(self.state));
        }
        self.answer = Some(text.into());
        self.state = AsideState::Answered;
        Ok(())
    }

    /// Settle the aside with a failure reason.
    pub fn fail(&mut self, why: impl Into<String>) -> Result<(), AsideError> {
        if self.state != AsideState::Asking {
            return Err(AsideError::NotAsking(self.state));
        }
        self.answer = Some(why.into());
        self.state = AsideState::Failed;
        Ok(())
    }

    /// True once the aside has an answer or a failure.
    pub fn is_settled(&self) -> bool {
        self.state != AsideState::Asking
    }
}
