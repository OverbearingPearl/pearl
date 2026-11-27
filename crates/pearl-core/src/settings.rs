//! settings --- the knobs the human may turn while the work runs.
//!
//! A knob that only works before launch is not a knob: the model is wrong, the
//! thinking is too shallow, the window folds too late, and the human notices
//! mid-task. Settings is a handle like the queue -- the surface and the runner
//! hold the same one, so an edit is felt by the next request rather than by the
//! next process.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use crate::catalog::{Candidate, protocol_complaint};

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
    /// Above `high` on the houses that invented a word for it.
    XHigh,
    /// Everything the endpoint will give. Not every house has a word for it.
    Max,
}

impl Effort {
    /// The one ladder: every rung any house speaks, weakest first. A dialect
    /// says some of it; the words never change meaning, which is what lets the
    /// same menu stand in front of every endpoint.
    pub const LADDER: [Effort; 7] = [
        Effort::Off,
        Effort::Minimal,
        Effort::Low,
        Effort::Medium,
        Effort::High,
        Effort::XHigh,
        Effort::Max,
    ];

    /// Every rung on the ladder, for a list that shows all of them.
    pub fn ladder() -> &'static [Effort] {
        &Effort::LADDER
    }

    /// The word on the wire and in the prompt.
    pub fn as_str(self) -> &'static str {
        match self {
            Effort::Off => "off",
            Effort::Minimal => "minimal",
            Effort::Low => "low",
            Effort::Medium => "medium",
            Effort::High => "high",
            Effort::XHigh => "xhigh",
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
            "xhigh" => Some(Effort::XHigh),
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
    /// The model a fold is asked of, when the human named one. None means the
    /// chosen one, which is right until someone says otherwise: writing a page
    /// of prose from a transcript is not the work the expensive model is for.
    fold_model: Option<String>,
    /// Rungs an endpoint answered with a 4xx that named the thinking field.
    /// Keyed by the address that answered -- `-` for the launch endpoint, the
    /// same spelling a candidate line uses -- because a refusal is a fact
    /// about one server, and two gateways for one house disagree about which
    /// words they take. The next model may accept the word happily.
    refused: BTreeMap<String, Vec<Effort>>,
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
                fold_model: None,
                refused: BTreeMap::new(),
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

    /// The model a fold is asked of: the one named for it, else the one the
    /// work goes to.
    pub fn fold_model(&self) -> String {
        let k = self.knobs();
        k.fold_model
            .clone()
            .unwrap_or_else(|| k.candidates[k.chosen].model.clone())
    }

    /// The fold model when the human stated one. A derived default is not
    /// news, and a config file should not carry what it never chose.
    pub fn stated_fold_model(&self) -> Option<String> {
        self.knobs().fold_model.clone()
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
        let key = *words
            .first()
            .ok_or("nothing to set: models, model, effort, window, trigger")?;
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
            // `refused` names its house, because a config file is read in a
            // process that may be pointed at another one. `refused clear`
            // forgets all of them, which is the only way back.
            "refused" => {
                if words.len() == 2 && words[1] == "clear" {
                    let n = self.forget_refusals();
                    return Ok(format!("refused: {n} forgotten"));
                }
                if words.len() != 3 {
                    return Err(String::from(
                        "refused is `refused clear` or `refused <house> <rung>`",
                    ));
                }
                let rung = Effort::parse(words[2])
                    .ok_or("effort is auto, off, minimal, low, medium, high, xhigh or max")?;
                let mut k = self.knobs();
                let listed = k.refused.entry(String::from(words[1])).or_default();
                if !listed.contains(&rung) {
                    listed.push(rung);
                }
                return Ok(format!("{} refused {}", words[1], rung.as_str()));
            }
            "remove" | "choose" => {
                let nth = Self::one_value(&words)?;
                let n: usize = nth
                    .parse()
                    .map_err(|_| format!("{key} takes a row number"))?;
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
                    .ok_or("effort is auto, off, minimal, low, medium, high, xhigh or max")?;
                let dialect = self.dialect_locked(&k);
                if !dialect.rungs().contains(&want) {
                    return Err(format!(
                        "ask this endpoint for {value} and it has nothing to hear: {}",
                        dialect.describe()
                    ));
                }
                let at = Self::address_locked(&k);
                if k.refused.get(&at).is_some_and(|r| r.contains(&want)) {
                    return Err(format!(
                        "{} refused {value}: it stays on the list with its reason, not in force",
                        Self::label_of(&at)
                    ));
                }
                k.effort = Some(want);
                Ok(format!("effort {value}"))
            }
            "dialect" => {
                k.dialect = Some(
                    crate::dialect::Dialect::parse(value)
                        .ok_or("dialect is openai, deepseek, qwen, glm or anthropic")?,
                );
                k.effort = None;
                Ok(format!("dialect {value}: effort back to auto"))
            }
            "keys" => {
                k.keys = Keys::parse(value).ok_or("keys is plain, vim or emacs")?;
                Ok(format!("keys {value}"))
            }
            "fold" => {
                if value == "auto" {
                    k.fold_model = None;
                    return Ok(String::from("fold auto: the working model writes it"));
                }
                // Every request goes to the endpoint the chosen candidate
                // names, so a fold model listed on another house would be
                // asked of the wrong server. Refused here, where the human
                // can read why, rather than as a 404 that leaves the history
                // unfolded.
                let at = k.candidates[k.chosen].endpoint.clone();
                if let Some(c) = k.candidates.iter().find(|c| c.model == value) {
                    if c.endpoint != at {
                        return Err(format!(
                            "{value} is on another endpoint: a fold is asked of the one in force"
                        ));
                    }
                }
                k.fold_model = Some(String::from(value));
                Ok(format!("fold {value}"))
            }
            "window" => {
                let n: u32 = value.parse().map_err(|_| "window is a number of tokens")?;
                if n < 1_000 {
                    return Err(String::from("window is at least 1000 tokens"));
                }
                k.window = n;
                Ok(format!(
                    "window {n}, folds at {}",
                    (n as f64 * k.trigger) as u32
                ))
            }
            "trigger" => {
                let f: f64 = value
                    .parse()
                    .map_err(|_| "trigger is a fraction like 0.6")?;
                if !(0.1..=0.95).contains(&f) {
                    return Err(String::from("trigger is between 0.1 and 0.95"));
                }
                k.trigger = f;
                Ok(format!(
                    "trigger {f}, folds at {}",
                    (k.window as f64 * f) as u32
                ))
            }
            other => Err(format!(
                "no setting called {other}: models, add, remove, edit, choose, model, effort, refused, window, trigger, fold"
            )),
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
            // Two fields, and the endpoint one may be left out: a model id
            // alone means the launch endpoint, which is the row a human adds
            // most often. `-` still says the same thing out loud.
            let c = match words.len() {
                // A word carrying `://` is an address, not an id: half a line
                // is still half a line, and a row filed under a URL as its
                // model would be sent as one.
                2 if words[1] != "-" && !words[1].contains("://") => {
                    Candidate::new(String::new(), words[1])
                }
                3 => {
                    if let Some(why) = protocol_complaint(words[1]) {
                        return Err(why);
                    }
                    Candidate::new(unset(words[1]), words[2])
                }
                _ => {
                    return Err(String::from(
                        "add takes a model id, or an endpoint and a model id",
                    ));
                }
            };
            let said = format!("added {}", c.label());
            k.candidates.push(c);
            return Ok(said);
        }
        if words.len() != 4 {
            return Err(String::from(
                "edit takes a row number, an endpoint and a model id",
            ));
        }
        let n: usize = words[1].parse().map_err(|_| "edit takes a row number")?;
        if n == 0 || n > k.candidates.len() {
            return Err(format!("there is no candidate {n}"));
        }
        if let Some(why) = protocol_complaint(words[2]) {
            return Err(why);
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
            return Err(String::from(
                "that is the last candidate; add another before removing it",
            ));
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
        let rows: Vec<Candidate> = models
            .into_iter()
            .map(|m| Candidate::new(endpoint.clone(), m))
            .collect();
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
        k.dialect
            .unwrap_or_else(|| crate::dialect::Dialect::for_house(&Self::house_locked(k)))
    }

    /// The house `-` resolves to, and the one a refusal is filed under.
    fn house_locked(k: &Knobs) -> String {
        let at = k.chosen.min(k.candidates.len() - 1);
        let c = &k.candidates[at];
        if c.endpoint.is_empty() {
            k.launch_house.clone()
        } else {
            c.provider()
        }
    }

    /// The house whose server the next request is sent to.
    pub fn house(&self) -> String {
        Self::house_locked(&self.knobs())
    }

    /// The address a refusal is filed under: the candidate's endpoint, or `-`
    /// for the one the process was launched against.
    pub fn address(&self) -> String {
        Self::address_locked(&self.knobs())
    }

    /// The address as a sentence names it: `-` is a config-file spelling, and
    /// a notice that says `- refused high` makes the human decode a dash.
    pub fn address_label(&self) -> String {
        Self::label_of(&self.address())
    }

    /// The same, for callers already holding the knobs.
    fn label_of(at: &str) -> String {
        if at == "-" {
            String::from("the launch endpoint")
        } else {
            at.to_string()
        }
    }

    fn address_locked(k: &Knobs) -> String {
        let at = k.chosen.min(k.candidates.len() - 1);
        let at = &k.candidates[at].endpoint;
        if at.is_empty() {
            String::from("-")
        } else {
            at.clone()
        }
    }

    /// The rungs this endpoint has already said no to, weakest first.
    pub fn refused(&self) -> Vec<Effort> {
        let at = self.address();
        self.knobs().refused.get(&at).cloned().unwrap_or_default()
    }

    /// Remember that the endpoint refused this rung, and say so in the words
    /// the surface will show. The rung stays on the list: a row deleted is a
    /// fact hidden, a row marked is the fact told.
    pub fn refuse(&self, rung: Effort) -> String {
        let at = self.address();
        let mut k = self.knobs();
        let listed = k.refused.entry(at.clone()).or_default();
        if !listed.contains(&rung) {
            listed.push(rung);
        }
        format!(
            "{} refused {}: sending auto instead",
            Self::label_of(&at),
            rung.as_str()
        )
    }

    /// Forget every refusal: the human may be about to point at a server that
    /// takes the words the last one did not.
    /// File every rung this ladder has against the address in force, for an
    /// endpoint that does not take the thinking field at all. One refusal
    /// about the field is a refusal of all five rungs, and remembering only
    /// the one in force would let the human walk into the other four.
    pub fn refuse_the_field(&self) -> String {
        let rungs: Vec<Effort> = self.dialect().rungs().to_vec();
        let at = self.address();
        let said = format!(
            "{} does not take the thinking field: the whole ladder is off until `refused clear`",
            Self::label_of(&at)
        );
        let mut k = self.knobs();
        let listed = k.refused.entry(at).or_default();
        for rung in rungs {
            if !listed.contains(&rung) {
                listed.push(rung);
            }
        }
        said
    }

    pub fn forget_refusals(&self) -> usize {
        let mut k = self.knobs();
        let n: usize = k.refused.values().map(Vec::len).sum();
        k.refused.clear();
        n
    }

    /// The next thinking effort the endpoint can actually express. A house
    /// with no such knob stays on auto rather than being handed a word it
    /// would ignore, and a rung this house refused is stepped over rather
    /// than offered back.
    pub fn cycle_effort(&self) {
        let mut k = self.knobs();
        let at = Self::address_locked(&k);
        let refused = k.refused.get(&at).cloned().unwrap_or_default();
        let rungs = self.dialect_locked(&k).rungs();
        let mut ladder: Vec<Option<Effort>> = vec![None];
        ladder.extend(
            rungs
                .iter()
                .copied()
                .filter(|e| !refused.contains(e))
                .map(Some),
        );
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
        vec![
            16_000, 32_000, 64_000, 128_000, 200_000, 256_000, 400_000, 1_000_000,
        ]
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
                let at = if c.endpoint.is_empty() {
                    "-"
                } else {
                    c.endpoint.as_str()
                };
                format!("candidate {at} {}", c.model)
            })
            .collect();
        lines.push(format!("choose {}", k.chosen + 1));
        if let Some(e) = k.effort {
            lines.push(format!("effort {}", e.as_str()));
        }
        // A refusal is worth keeping: it is a fact the endpoint told us, and
        // the next session should not have to earn it with the same 400.
        for (at, rungs) in &k.refused {
            for rung in rungs {
                lines.push(format!("refused {at} {}", rung.as_str()));
            }
        }
        lines.push(format!("window {}", k.window));
        lines.push(format!("trigger {}", k.trigger));
        lines.push(format!("keys {}", k.keys.as_str()));
        if let Some(m) = &k.fold_model {
            lines.push(format!("fold {m}"));
        }
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
        for line in text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
        {
            // A file that states the list means it: the first row replaces
            // the seeded default rather than joining it, or every load would
            // grow the list by one row nobody chose.
            if line.starts_with("candidate ") && !listed {
                self.knobs().candidates.clear();
                self.knobs().chosen = 0;
                listed = true;
            }
            self.apply(line)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
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
