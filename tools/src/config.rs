//! The single source of policy, and its project override.
//!
//! `ci/config.toml` is the suite's default and is replaced wholesale on a
//! subtree pull; `config/ci.toml` belongs to the project and a pull never
//! touches it. The two are deep-merged, the project winning key by key, so a
//! project lists only what it differs in. Everything a check may tune lives
//! here; no check reads a second source of truth.

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;

/// One module class's rules: what its files must carry, what it may depend on,
/// and how long its module documentation may be. A class missing from the
/// table is a mistake the `policy` check reports, never a deliberate exemption.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Profile {
    #[serde(default)]
    pub headers_require: Vec<String>,
    #[serde(default)]
    pub headers_forbid: Vec<String>,
    #[serde(default)]
    pub may_require: Vec<String>,
    #[serde(default = "one")]
    pub commentary_min: usize,
    /// Zero means unbounded.
    #[serde(default)]
    pub commentary_max: usize,
}

fn one() -> usize {
    1
}

/// A banned construct: a regex and the replacement to suggest.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Banned {
    pub pattern: String,
    pub suggestion: String,
}

/// The layer stack. `order` is the direction: a crate may depend on its own
/// layer or any layer listed before it. `members` maps a layer to its crates.
/// `may_require`, when set for a layer, overrides the order for a shape that is
/// not a stack. An empty `order` disables `layer-order`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Layers {
    #[serde(default)]
    pub order: Vec<String>,
    #[serde(default)]
    pub members: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub may_require: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Budget {
    pub max: Option<f64>,
    pub min: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct SizeCfg {
    #[serde(default = "size_dirs")]
    pub directories: Vec<String>,
    #[serde(default = "four")]
    pub window: usize,
    #[serde(default = "three_hundred")]
    pub large: usize,
    #[serde(default = "twenty")]
    pub shown: usize,
    #[serde(default)]
    pub skip: Vec<String>,
}

fn size_dirs() -> Vec<String> {
    vec!["crates".into()]
}
fn four() -> usize {
    4
}
fn three_hundred() -> usize {
    300
}
fn twenty() -> usize {
    20
}

impl Default for SizeCfg {
    fn default() -> Self {
        toml::from_str("").unwrap()
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct CoverageCfg {
    #[serde(default = "size_dirs")]
    pub directories: Vec<String>,
}

impl Default for CoverageCfg {
    fn default() -> Self {
        toml::from_str("").unwrap()
    }
}

/// The whole policy.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Config {
    #[serde(default = "msrv")]
    pub msrv: String,
    #[serde(default = "edition")]
    pub edition: String,
    #[serde(default = "src_dir")]
    pub src_dir: String,
    #[serde(default = "test_dir")]
    pub test_dir: String,
    #[serde(default = "tooling_dir")]
    pub tooling_dir: String,
    #[serde(default = "vendor_dir")]
    pub vendor_dir: String,
    #[serde(default = "width")]
    pub comment_width: usize,
    #[serde(default)]
    pub crate_prefix: String,
    #[serde(default)]
    pub main_crate: String,
    #[serde(default)]
    pub layers: Layers,
    #[serde(default)]
    pub profiles: BTreeMap<String, Profile>,
    #[serde(default)]
    pub banned: Vec<Banned>,
    #[serde(default)]
    pub size: SizeCfg,
    #[serde(default)]
    pub coverage: CoverageCfg,
    #[serde(default)]
    pub perf: BTreeMap<String, Budget>,
}

fn msrv() -> String {
    "1.85".into()
}
fn edition() -> String {
    "2024".into()
}
fn src_dir() -> String {
    "crates".into()
}
fn test_dir() -> String {
    "tests".into()
}
fn tooling_dir() -> String {
    "ci".into()
}
fn vendor_dir() -> String {
    "vendor".into()
}
fn width() -> usize {
    80
}

impl Default for Config {
    fn default() -> Self {
        toml::from_str("").unwrap()
    }
}

impl Config {
    /// Load the suite default, then deep-merge the project override.
    pub fn load(root: &Path, suite: &Path) -> Result<Self> {
        let default_path = suite.join("config.toml");
        let mut value = if default_path.exists() {
            let text = std::fs::read_to_string(&default_path)
                .with_context(|| format!("reading {}", default_path.display()))?;
            text.parse::<toml::Value>()
                .with_context(|| format!("parsing {}", default_path.display()))?
        } else {
            toml::Value::Table(toml::map::Map::new())
        };
        let override_path = root.join("config/ci.toml");
        if override_path.exists() {
            let text = std::fs::read_to_string(&override_path)
                .with_context(|| format!("reading {}", override_path.display()))?;
            let over = text
                .parse::<toml::Value>()
                .with_context(|| format!("parsing {}", override_path.display()))?;
            merge(&mut value, over);
        }
        let cfg: Config = value.try_into().context("interpreting merged configuration")?;
        Ok(cfg)
    }

    /// The layer a crate belongs to, if the stack names it.
    pub fn layer_of(&self, crate_name: &str) -> Option<&str> {
        self.layers
            .members
            .iter()
            .find(|(_, crates)| crates.iter().any(|c| c == crate_name))
            .map(|(layer, _)| layer.as_str())
    }

    /// The layers a crate in `layer` may depend on: the override if present,
    /// else its own layer and every layer before it in the order.
    pub fn allowed_layers(&self, layer: &str) -> Vec<String> {
        if let Some(explicit) = self.layers.may_require.get(layer) {
            return explicit.clone();
        }
        let mut out = Vec::new();
        for l in &self.layers.order {
            out.push(l.clone());
            if l == layer {
                break;
            }
        }
        out
    }
}

/// Recursively overlay `over` onto `base`; tables merge, everything else wins.
fn merge(base: &mut toml::Value, over: toml::Value) {
    match (base, over) {
        (toml::Value::Table(b), toml::Value::Table(o)) => {
            for (k, v) in o {
                merge(b.entry(k).or_insert(toml::Value::Table(toml::map::Map::new())), v);
            }
        }
        (b, o) => *b = o,
    }
}

/// `ci-tools config get <key>` prints one scalar; `config env` prints shell
/// assignments the orchestrator can eval. Keys use the TOML spelling.
pub fn cli(cfg: &Config, args: &[String]) -> Result<()> {
    let Some(sub) = args.first() else {
        bail!("usage: ci-tools config <get|env> [key]");
    };
    match sub.as_str() {
        "get" => {
            let Some(key) = args.get(1) else {
                bail!("usage: ci-tools config get <key>");
            };
            if let Some(name) = key.strip_prefix("perf.") {
                let Some(b) = cfg.perf.get(name) else {
                    bail!("unknown perf budget: {name}");
                };
                println!(
                    "max={} min={}",
                    b.max.map(|v| v.to_string()).unwrap_or_default(),
                    b.min.map(|v| v.to_string()).unwrap_or_default()
                );
                return Ok(());
            }
            println!("{}", scalar(cfg, key)?);
            Ok(())
        }
        "env" => {
            for key in [
                "msrv",
                "edition",
                "src-dir",
                "test-dir",
                "tooling-dir",
                "vendor-dir",
                "comment-width",
                "crate-prefix",
                "main-crate",
            ] {
                let name = format!("CI_{}", key.to_uppercase().replace('-', "_"));
                println!("{name}={}", shell_quote(&scalar(cfg, key)?));
            }
            Ok(())
        }
        other => bail!("unknown config subcommand: {other}"),
    }
}

fn scalar(cfg: &Config, key: &str) -> Result<String> {
    Ok(match key {
        "msrv" => cfg.msrv.clone(),
        "edition" => cfg.edition.clone(),
        "src-dir" => cfg.src_dir.clone(),
        "test-dir" => cfg.test_dir.clone(),
        "tooling-dir" => cfg.tooling_dir.clone(),
        "vendor-dir" => cfg.vendor_dir.clone(),
        "comment-width" => cfg.comment_width.to_string(),
        "crate-prefix" => cfg.crate_prefix.clone(),
        "main-crate" => cfg.main_crate.clone(),
        "coverage-dirs" => cfg.coverage.directories.join(" "),
        other => bail!("unknown config key: {other}"),
    })
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}
