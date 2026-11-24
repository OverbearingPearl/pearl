//! settings --- the knobs the human may turn while the work runs.
//!
//! A knob that only works before launch is not a knob: the model is wrong, the
//! thinking is too shallow, the window folds too late, and the human notices
//! mid-task. Settings is a handle like the queue -- the surface and the runner
//! hold the same one, so an edit is felt by the next request rather than by the
//! next process.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

/// How hard the model should think before answering. `None` leaves the choice
/// to the endpoint, which is what an endpoint that has no such knob expects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effort {
    Low,
    Medium,
    High,
}

impl Effort {
    /// The word on the wire and in the prompt.
    pub fn as_str(self) -> &'static str {
        match self {
            Effort::Low => "low",
            Effort::Medium => "medium",
            Effort::High => "high",
        }
    }

    fn parse(word: &str) -> Option<Self> {
        match word {
            "low" => Some(Effort::Low),
            "medium" => Some(Effort::Medium),
            "high" => Some(Effort::High),
            _ => None,
        }
    }
}

#[derive(Debug)]
struct Knobs {
    model: String,
    effort: Option<Effort>,
    window: u32,
    trigger: f64,
}

/// The live configuration, shared by the surface and the runner.
#[derive(Debug, Clone)]
pub struct Settings {
    knobs: Arc<Mutex<Knobs>>,
}

impl Settings {
    /// Defaults for a DeepSeek-shaped endpoint: its chat model, no effort
    /// preference, a 128k window folding at three quarters.
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            knobs: Arc::new(Mutex::new(Knobs {
                model: model.into(),
                effort: None,
                window: 128_000,
                trigger: 0.75,
            })),
        }
    }

    fn knobs(&self) -> MutexGuard<'_, Knobs> {
        self.knobs.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn model(&self) -> String {
        self.knobs().model.clone()
    }

    pub fn set_model(&self, model: impl Into<String>) {
        self.knobs().model = model.into();
    }

    pub fn effort(&self) -> Option<Effort> {
        self.knobs().effort
    }

    pub fn window(&self) -> u32 {
        self.knobs().window
    }

    pub fn trigger(&self) -> f64 {
        self.knobs().trigger
    }

    /// The prompt size at which the oldest turns fold into a summary.
    pub fn fold_at(&self) -> u32 {
        let k = self.knobs();
        (k.window as f64 * k.trigger) as u32
    }

    /// Apply one line the human typed. Nothing changes unless the whole line
    /// parses, so a typo cannot leave half a configuration in force.
    pub fn apply(&self, line: &str) -> Result<String, String> {
        let mut words = line.split_whitespace();
        let key = words.next().ok_or("nothing to set: model, effort, window, trigger")?;
        let value = words.next().ok_or_else(|| format!("{key} needs a value"))?;
        if words.next().is_some() {
            return Err(format!("{key} takes one value"));
        }
        let mut k = self.knobs();
        match key {
            "model" => {
                k.model = value.to_string();
                Ok(format!("model {value}"))
            }
            "effort" => {
                if value == "off" {
                    k.effort = None;
                    return Ok(String::from("effort off: the endpoint decides"));
                }
                k.effort = Some(Effort::parse(value).ok_or("effort is low, medium, high or off")?);
                Ok(format!("effort {value}"))
            }
            "window" => {
                let n: u32 = value.parse().map_err(|_| "window is a number of tokens")?;
                if n < 1_000 {
                    return Err(String::from("window is at least 1000 tokens"));
                }
                k.window = n;
                Ok(format!("window {n}, folds at {}", (n as f64 * k.trigger) as u32))
            }
            "trigger" => {
                let f: f64 = value.parse().map_err(|_| "trigger is a fraction like 0.6")?;
                if !(0.1..=0.95).contains(&f) {
                    return Err(String::from("trigger is between 0.1 and 0.95"));
                }
                k.trigger = f;
                Ok(format!("trigger {f}, folds at {}", (k.window as f64 * f) as u32))
            }
            other => Err(format!("no setting called {other}: model, effort, window, trigger")),
        }
    }

    /// What is in force, in one line the surface can show.
    pub fn describe(&self) -> String {
        let k = self.knobs();
        let effort = k.effort.map(Effort::as_str).unwrap_or("auto");
        format!("{} | effort {effort} | window {} | fold {:.0}%", k.model, k.window, k.trigger * 100.0)
    }
}
