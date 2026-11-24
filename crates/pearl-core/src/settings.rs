//! settings --- the knobs the human may turn while the work runs.
//!
//! A knob that only works before launch is not a knob: the model is wrong, the
//! thinking is too shallow, the window folds too late, and the human notices
//! mid-task. Settings is a handle like the queue -- the surface and the runner
//! hold the same one, so an edit is felt by the next request rather than by the
//! next process.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use crate::catalog::Candidate;

/// How hard the model should think, as the human means it. `None` is `auto`:
/// send nothing and leave the choice to the endpoint. Which of these a given
/// endpoint can actually express is `Dialect`'s answer, not this enum's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effort {
    /// The model does not think. Not the same as `auto`, which sends nothing
    /// and leaves the decision to the endpoint.
    Off,
    Minimal,
    Low,
    Medium,
    High,
    /// Everything the endpoint will give. Not every house has a word for it.
    Max,
}

impl Effort {
    /// The word on the wire and in the prompt.
    pub fn as_str(self) -> &'static str {
        match self {
            Effort::Off => "off",
            Effort::Minimal => "minimal",
            Effort::Low => "low",
            Effort::Medium => "medium",
            Effort::High => "high",
            Effort::Max => "max",
        }
    }

    pub fn parse(word: &str) -> Option<Self> {
        match word {
            "off" => Some(Effort::Off),
            "minimal" => Some(Effort::Minimal),
            "low" => Some(Effort::Low),
            "medium" => Some(Effort::Medium),
            "high" => Some(Effort::High),
            "max" => Some(Effort::Max),
            _ => None,
        }
    }
}

#[derive(Debug)]
struct Knobs {
    /// What the human may choose between, and which one is chosen. The list
    /// is never empty: a choice with nothing to choose is not a knob.
    candidates: Vec<Candidate>,
    chosen: usize,
    effort: Option<Effort>,
    window: u32,
    trigger: f64,
    keys: Keys,
    /// A dialect the human stated, overriding the house's. None means read it
    /// off the endpoint, which is right until the endpoint proves otherwise.
    dialect: Option<crate::dialect::Dialect>,
    /// The house `-` stands for. Core cannot read it off an empty endpoint,
    /// and the process that knows it says so once at startup.
    launch_house: String,
}

/// How the human moves. A style owns motion; the product's actions keep their
/// letters, so learning a second style is learning four keys, not a new tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Keys {
    /// Arrows and Tab: nothing that needs a key a laptop does not have.
    #[default]
    Plain,
    Vim,
    Emacs,
}

impl Keys {
    pub fn as_str(self) -> &'static str {
        match self {
            Keys::Plain => "plain",
            Keys::Vim => "vim",
            Keys::Emacs => "emacs",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "plain" => Some(Keys::Plain),
            "vim" => Some(Keys::Vim),
            "emacs" => Some(Keys::Emacs),
            _ => None,
        }
    }
}

/// `-` in a typed line means the endpoint the process was launched against.
fn unset(word: &str) -> &str {
    if word == "-" { "" } else { word }
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
                candidates: vec![Candidate::new("", model.into())],
                chosen: 0,
                effort: None,
                window: 128_000,
                trigger: 0.75,
                keys: Keys::Plain,
                dialect: None,
                launch_house: String::from("deepseek"),
            })),
        }
    }

    fn knobs(&self) -> MutexGuard<'_, Knobs> {
        self.knobs.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn model(&self) -> String {
        let k = self.knobs();
        k.candidates[k.chosen].model.clone()
    }

    /// The endpoint the chosen candidate speaks to. Empty means the one the
    /// process was launched against.
    pub fn endpoint(&self) -> String {
        let k = self.knobs();
        k.candidates[k.chosen].endpoint.clone()
    }

    /// The list the human chooses from, in the order it is shown.
    /// The row in force. The list is never empty, so there is always one.
    pub fn chosen(&self) -> Candidate {
        let k = self.knobs();
        k.candidates[k.chosen.min(k.candidates.len() - 1)].clone()
    }

    /// One row by position, for a surface that highlights before it chooses.
    pub fn chosen_at(&self, at: usize) -> Candidate {
        let k = self.knobs();
        k.candidates[at.min(k.candidates.len() - 1)].clone()
    }

    pub fn candidates(&self) -> Vec<Candidate> {
        self.knobs().candidates.clone()
    }

    /// Choose by the number the list shows, counting from one.
    pub fn choose(&self, nth: usize) -> Result<String, String> {
        let mut k = self.knobs();
        if nth == 0 || nth > k.candidates.len() {
            return Err(format!("there is no candidate {nth}"));
        }
        k.chosen = nth - 1;
        Ok(format!("model {}", k.candidates[k.chosen].label()))
    }

    /// Take a model id typed by hand. An id already on the list is chosen;
    /// one that is not joins it on the endpoint in force, so typing a name
    /// the house table never heard of still works.
    pub fn set_model(&self, model: impl Into<String>) {
        let mut k = self.knobs();
        let model = model.into();
        match k.candidates.iter().position(|c| c.model == model) {
            Some(i) => k.chosen = i,
            None => {
                let endpoint = k.candidates[k.chosen].endpoint.clone();
                k.candidates.push(Candidate::new(endpoint, model));
                k.chosen = k.candidates.len() - 1;
            }
        }
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
        let words: Vec<&str> = line.split_whitespace().collect();
        let key = *words.first().ok_or("nothing to set: models, model, effort, window, trigger")?;
        // A candidate is two fields by nature, so these lines are read whole
        // before anything moves: a typo must not leave half a row behind.
        match key {
            "models" => {
                return Ok(self
                    .candidates()
                    .iter()
                    .enumerate()
                    .map(|(i, c)| format!("{} {}", i + 1, c.label()))
                    .collect::<Vec<_>>()
                    .join("  "));
            }
            "add" | "edit" | "candidate" => return self.edit_candidate(&words),
            "remove" | "choose" => {
                let nth = Self::one_value(&words)?;
                let n: usize = nth.parse().map_err(|_| format!("{key} takes a row number"))?;
                return match key {
                    "choose" => self.choose(n),
                    _ => self.remove(n),
                };
            }
            _ => {}
        }
        let value = Self::one_value(&words)?;
        let mut k = self.knobs();
        match key {
            "model" => {
                drop(k);
                self.set_model(value);
                Ok(format!("model {value}"))
            }
            "effort" => {
                if value == "auto" {
                    k.effort = None;
                    return Ok(String::from("effort auto: the endpoint decides"));
                }
                let want = Effort::parse(value)
                    .ok_or("effort is auto, off, minimal, low, medium, high or max")?;
                let dialect = self.dialect_locked(&k);
                if !dialect.rungs().contains(&want) {
                    return Err(format!("{} cannot say {value}: {}", dialect.as_str(), dialect.describe()));
                }
                k.effort = Some(want);
                Ok(format!("effort {value}"))
            }
            "dialect" => {
                k.dialect = Some(crate::dialect::Dialect::parse(value)
                    .ok_or("dialect is openai, deepseek, qwen, glm or anthropic")?);
                k.effort = None;
                Ok(format!("dialect {value}: effort back to auto"))
            }
            "keys" => {
                k.keys = Keys::parse(value).ok_or("keys is plain, vim or emacs")?;
                Ok(format!("keys {value}"))
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
            other => Err(format!("no setting called {other}: models, add, remove, edit, choose, model, effort, window, trigger")),
        }
    }

    /// The single value a one-field knob takes, or the reason it was refused.
    fn one_value<'a>(words: &[&'a str]) -> Result<&'a str, String> {
        let key = words[0];
        match words.len() {
            1 => Err(format!("{key} needs a value")),
            2 => Ok(words[1]),
            _ => Err(format!("{key} takes one value")),
        }
    }

    /// Add a row, or rewrite one. Both take an endpoint and a model id; the
    /// house is derived, so there is nothing here to keep in agreement.
    fn edit_candidate(&self, words: &[&str]) -> Result<String, String> {
        let key = words[0];
        let mut k = self.knobs();
        if key == "add" || key == "candidate" {
            if words.len() != 3 {
                return Err(String::from("add takes an endpoint and a model id"));
            }
            let c = Candidate::new(unset(words[1]), words[2]);
            let said = format!("added {}", c.label());
            k.candidates.push(c);
            return Ok(said);
        }
        if words.len() != 4 {
            return Err(String::from("edit takes a row number, an endpoint and a model id"));
        }
        let n: usize = words[1].parse().map_err(|_| "edit takes a row number")?;
        if n == 0 || n > k.candidates.len() {
            return Err(format!("there is no candidate {n}"));
        }
        k.candidates[n - 1] = Candidate::new(unset(words[2]), words[3]);
        Ok(format!("row {n} is now {}", k.candidates[n - 1].label()))
    }

    /// Drop a row. The last one stays: a list with nothing in it is not a
    /// choice, and the runner would have nowhere to send the next request.
    fn remove(&self, nth: usize) -> Result<String, String> {
        let mut k = self.knobs();
        if nth == 0 || nth > k.candidates.len() {
            return Err(format!("there is no candidate {nth}"));
        }
        if k.candidates.len() == 1 {
            return Err(String::from("that is the last candidate; add another before removing it"));
        }
        let gone = k.candidates.remove(nth - 1);
        if k.chosen >= k.candidates.len() {
            k.chosen = k.candidates.len() - 1;
        }
        Ok(format!("removed {}", gone.label()))
    }

    /// What is in force, in one line the surface can show.
    /// The next model on the ladder. A model set by hand that is not on the
    /// ladder joins it at the front, so cycling never strands the choice.
    pub fn cycle_model(&self) {
        let mut k = self.knobs();
        k.chosen = (k.chosen + 1) % k.candidates.len();
    }

    /// Say which models the endpoint has. A model from another house is
    /// replaced by the first of these; an unknown endpoint offers nothing and
    /// the chosen model is left alone.
    pub fn set_models(&self, models: impl IntoIterator<Item = impl Into<String>>) {
        let mut k = self.knobs();
        let endpoint = k.candidates[k.chosen].endpoint.clone();
        let rows: Vec<Candidate> =
            models.into_iter().map(|m| Candidate::new(endpoint.clone(), m)).collect();
        if rows.is_empty() {
            return;
        }
        let chosen = k.candidates[k.chosen].model.clone();
        k.chosen = rows.iter().position(|c| c.model == chosen).unwrap_or(0);
        k.candidates = rows;
    }

    /// Say which house the launch endpoint belongs to, so a candidate that
    /// names no endpoint of its own still speaks the right dialect.
    pub fn set_launch_house(&self, house: impl Into<String>) {
        self.knobs().launch_house = house.into();
    }

    /// The dialect in force: what the human stated, else the chosen house's.
    pub fn dialect(&self) -> crate::dialect::Dialect {
        self.dialect_locked(&self.knobs())
    }

    fn dialect_locked(&self, k: &Knobs) -> crate::dialect::Dialect {
        k.dialect.unwrap_or_else(|| {
            let at = k.chosen.min(k.candidates.len() - 1);
            let c = &k.candidates[at];
            let house =
                if c.endpoint.is_empty() { k.launch_house.clone() } else { c.provider() };
            crate::dialect::Dialect::for_house(&house)
        })
    }

    /// The next thinking effort the endpoint can actually express. A house
    /// with no such knob stays on auto rather than being handed a word it
    /// would ignore.
    pub fn cycle_effort(&self) {
        let mut k = self.knobs();
        let rungs = self.dialect_locked(&k).rungs();
        let mut ladder: Vec<Option<Effort>> = vec![None];
        ladder.extend(rungs.iter().copied().map(Some));
        let at = ladder.iter().position(|e| *e == k.effort).unwrap_or(0);
        k.effort = ladder[(at + 1) % ladder.len()];
    }

    /// The next context window on the ladder. The fold threshold rides along
    /// as a share, so a smaller window folds sooner without a second edit.
    pub fn cycle_window(&self) {
        let ladder = Self::windows();
        let mut k = self.knobs();
        let at = ladder.iter().position(|w| *w == k.window);
        k.window = match at {
            Some(i) => ladder[(i + 1) % ladder.len()],
            None => ladder[0],
        };
    }

    /// The sizes the window key walks. One list, so the key that cycles and
    /// the list that chooses cannot offer different sizes.
    pub fn windows() -> Vec<u32> {
        vec![16_000, 32_000, 64_000, 128_000, 200_000, 256_000, 400_000, 1_000_000]
    }

    /// The keyboard the human moves with. Actions keep their letters in every
    /// style; motion is what a style is.
    pub fn keys(&self) -> Keys {
        self.knobs().keys
    }

    /// The next navigation style: plain, vim, emacs.
    pub fn cycle_keys(&self) {
        let mut k = self.knobs();
        k.keys = match k.keys {
            Keys::Plain => Keys::Vim,
            Keys::Vim => Keys::Emacs,
            Keys::Emacs => Keys::Plain,
        };
    }

    /// Write the knobs as the same lines the `:` prompt takes, so the file is
    /// editable by hand in the language the surface already speaks. A second
    /// grammar for the same knobs is how a file and a prompt disagree.
    pub fn save(&self, path: impl AsRef<std::path::Path>) -> std::io::Result<()> {
        let k = self.knobs();
        let mut lines: Vec<String> = k
            .candidates
            .iter()
            .map(|c| {
                // `-` is the endpoint the process was launched against: an
                // empty field would vanish from the line and the row would
                // come back one word short.
                let at = if c.endpoint.is_empty() { "-" } else { c.endpoint.as_str() };
                format!("candidate {at} {}", c.model)
            })
            .collect();
        lines.push(format!("choose {}", k.chosen + 1));
        if let Some(e) = k.effort {
            lines.push(format!("effort {}", e.as_str()));
        }
        lines.push(format!("window {}", k.window));
        lines.push(format!("trigger {}", k.trigger));
        lines.push(format!("keys {}", k.keys.as_str()));
        // The dialect goes first: it decides which effort rungs the lines
        // below are allowed to name, and a file read top to bottom has to
        // know that before it meets one.
        if let Some(d) = k.dialect {
            lines.insert(0, format!("dialect {}", d.as_str()));
        }
        drop(k);
        if let Some(dir) = path.as_ref().parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, format!("{}\n", lines.join("\n")))
    }

    /// Apply a saved config. A missing file is not an error: the defaults
    /// stand. Returns how many lines were taken.
    pub fn load(&self, path: impl AsRef<std::path::Path>) -> std::io::Result<usize> {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
            Err(e) => return Err(e),
        };
        let mut taken = 0;
        let mut listed = false;
        for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
            // A file that states the list means it: the first row replaces
            // the seeded default rather than joining it, or every load would
            // grow the list by one row nobody chose.
            if line.starts_with("candidate ") && !listed {
                self.knobs().candidates.clear();
                self.knobs().chosen = 0;
                listed = true;
            }
            self.apply(line).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
            taken += 1;
        }
        Ok(taken)
    }

    /// The dialect when it was stated or learned, and nothing when it is
    /// simply the house's own. A derived default is not news; a correction is.
    pub fn stated_dialect(&self) -> Option<crate::dialect::Dialect> {
        self.knobs().dialect
    }

    pub fn describe(&self) -> String {
        let k = self.knobs();
        let effort = k.effort.map(Effort::as_str).unwrap_or("auto");
        format!(
            "{} | effort {effort} | window {} | fold {:.0}%",
            k.candidates[k.chosen].label(),
            k.window,
            k.trigger * 100.0
        )
    }
}
