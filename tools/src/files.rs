//! Discovering the tree and answering "what class is this file?".
//!
//! Discovery is `git ls-files`, so the suite sees exactly what the repository
//! tracks plus untracked-but-not-ignored work, and never a build artifact.
//! Classification is by directory, the one place classes are decided; a check
//! asks for a class and never re-derives it from a file name.

use crate::config::Config;
use anyhow::{Context, Result, bail};
use std::path::Path;
use std::process::Command;

/// The five module classes, in the order the report prints them.
pub const CLASSES: [&str; 5] = ["main", "business", "test", "tooling", "vendor"];

/// Every tracked (and untracked, unignored) file, relative to the root.
pub fn tracked(root: &Path) -> Result<Vec<String>> {
    let out = Command::new("git")
        .args(["ls-files", "--cached", "--others", "--exclude-standard"])
        .current_dir(root)
        .output()
        .context("running git ls-files (is this a git repository?)")?;
    if !out.status.success() {
        bail!("git ls-files failed: {}", String::from_utf8_lossy(&out.stderr));
    }
    let mut files: Vec<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| l.to_string())
        .filter(|l| !l.is_empty())
        .collect();
    files.sort();
    files.dedup();
    Ok(files)
}

/// The class of a relative path, or `None` when it is not a module the suite
/// classifies (a Markdown page, a shell script, an asset).
pub fn classify(cfg: &Config, path: &str) -> Option<&'static str> {
    let p = path.replace('\\', "/");
    // Only Rust source is a module the per-file checks read. Shell, Markdown,
    // TOML and the rest are handled by the global checks (meta-info, doc-links,
    // adr-format) or are not the suite's business at all.
    if !p.ends_with(".rs") {
        return None;
    }
    let under = |dir: &str| !dir.is_empty() && (p == dir || p.starts_with(&format!("{dir}/")));
    if under(&cfg.vendor_dir) {
        return Some("vendor");
    }
    if under(&cfg.tooling_dir) || under("tools") {
        return Some("tooling");
    }
    let is_test = under(&cfg.test_dir)
        || p.contains("/tests/")
        || p.ends_with("-tests.rs")
        || p.ends_with("_tests.rs");
    if is_test {
        return Some("test");
    }
    // A Rust source file under the business directory belongs to its crate;
    // the designated entry crate is `main`, the rest are `business`.
    if under(&cfg.src_dir) || p.starts_with("src/") {
        if !cfg.main_crate.is_empty() && crate_dir_of(cfg, &p).as_deref() == Some(&cfg.main_crate) {
            return Some("main");
        }
        return Some("business");
    }
    Some("business")
}

/// The crate directory basename a source path lives in (`crates/foo/src/..`
/// yields `foo`); the root `src/` yields the empty string.
pub fn crate_dir_of(cfg: &Config, path: &str) -> Option<String> {
    let prefix = format!("{}/", cfg.src_dir);
    if let Some(rest) = path.strip_prefix(&prefix) {
        return rest.split('/').next().map(|s| s.to_string());
    }
    None
}

/// `ci-tools files [--class C] [--tsv]`: the discovered modules. With no flag,
/// a grouped report; `--class` prints one path per line for that class; `--tsv`
/// prints `class<TAB>path` for every classified file.
pub fn cli(cfg: &Config, root: &Path, args: &[String]) -> Result<()> {
    let files = tracked(root)?;
    let want_class = crate::flag(args, "--class");
    let tsv = args.iter().any(|a| a == "--tsv");
    let mut rows: Vec<(String, &'static str)> = Vec::new();
    for f in &files {
        if let Some(class) = classify(cfg, f) {
            rows.push((f.clone(), class));
        }
    }
    if tsv {
        for (f, c) in &rows {
            println!("{c}\t{f}");
        }
        return Ok(());
    }
    if let Some(want) = want_class {
        if !CLASSES.contains(&want.as_str()) {
            bail!("unknown class: {want}");
        }
        for (f, c) in &rows {
            if *c == want {
                println!("{f}");
            }
        }
        return Ok(());
    }
    for class in CLASSES {
        let members: Vec<&String> = rows.iter().filter(|(_, c)| *c == class).map(|(f, _)| f).collect();
        println!("{class} ({}):", members.len());
        for f in members {
            println!("  {f}");
        }
    }
    Ok(())
}

/// The positional arguments, skipping `--root`/`--suite`/`--class` and their
/// values, so a subcommand can find the file name among the flags.
fn positional(args: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut skip = false;
    for a in args {
        if skip {
            skip = false;
            continue;
        }
        match a.as_str() {
            "--root" | "--suite" | "--class" => skip = true,
            "--tsv" => {}
            s if s.starts_with("--") => {}
            s => out.push(s.to_string()),
        }
    }
    out
}

/// `ci-tools explain <file>`: the class and the profile that governs it. The
/// orchestrator adds which checks apply, since it owns the check table.
pub fn explain(cfg: &Config, _root: &Path, args: &[String]) -> Result<()> {
    let files = positional(args);
    let Some(file) = files.first() else {
        bail!("usage: ci-tools explain <file>");
    };
    match classify(cfg, file) {
        None => println!("{file}: not a classified module"),
        Some(class) => {
            println!("{file}: class {class}");
            match cfg.profiles.get(class) {
                None => println!("  no profile (the policy check reports this)"),
                Some(p) => {
                    println!("  may require: {}", p.may_require.join(", "));
                    println!("  headers require: {}", p.headers_require.join(", "));
                    println!("  headers forbid: {}", p.headers_forbid.join(", "));
                    println!("  commentary: min {} max {}", p.commentary_min, p.commentary_max);
                }
            }
        }
    }
    Ok(())
}
