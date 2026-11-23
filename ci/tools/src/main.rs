//! Command-line entry for `ci-tools`, the parsing half of the ci-rust suite.
//!
//! The shell orchestrator (`run-checks.sh`) owns discovery, the check table and
//! the gate; this binary owns everything that needs to read Rust or TOML: the
//! merged configuration, file classification, and the structural checks. Each
//! `check <name>` subcommand prints findings as tab-separated records on stdout
//! and exits zero; the orchestrator decides what fails.

mod checks;
mod config;
mod files;
mod findings;
mod size;

use anyhow::{Context, Result, bail};
use std::path::PathBuf;

fn main() {
    if let Err(err) = run() {
        eprintln!("ci-tools: {err:#}");
        std::process::exit(2);
    }
}

fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = args.first() else {
        bail!("usage: ci-tools <config|files|explain|check|size> ...");
    };
    // Every subcommand takes `--root DIR` (the project) and `--suite DIR` (the
    // ci/ checkout). They default to the working directory and its `ci/`.
    let root = flag(&args, "--root").unwrap_or_else(|| ".".into());
    let suite = flag(&args, "--suite").unwrap_or_else(|| "ci".into());
    let root = PathBuf::from(root);
    let suite = PathBuf::from(suite);
    let cfg = config::Config::load(&root, &suite).context("loading configuration")?;

    match cmd.as_str() {
        "config" => config::cli(&cfg, &args[1..]),
        "files" => files::cli(&cfg, &root, &args[1..]),
        "explain" => files::explain(&cfg, &root, &args[1..]),
        "size" => size::cli(&cfg, &root, &args[1..]),
        "check" => {
            let Some(name) = args.get(1) else {
                bail!("usage: ci-tools check <name> [--root DIR] [--suite DIR]");
            };
            let rest: Vec<String> = args[2..].to_vec();
            checks::dispatch(name, &cfg, &root, &suite, &rest)
        }
        other => bail!("unknown subcommand: {other}"),
    }
}

/// Read `--name VALUE` from an argument list.
pub fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}
