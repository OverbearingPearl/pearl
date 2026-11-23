//! `make size`: a report, never a gate.
//!
//! It reads the tree the way the policy's `:size` table says, names modules
//! past the large threshold, and finds runs of consecutive significant lines
//! shared between files -- the shape of copy-paste. It prints and exits zero;
//! a number here is information, not a verdict.

use crate::config::Config;
use crate::files::tracked;
use anyhow::Result;
use regex::Regex;
use std::collections::HashMap;
use std::path::Path;

pub fn cli(cfg: &Config, root: &Path, _args: &[String]) -> Result<()> {
    let all = tracked(root)?;
    let skip: Vec<Regex> = cfg.size.skip.iter().filter_map(|p| Regex::new(p).ok()).collect();
    let mut files: Vec<(String, Vec<String>)> = Vec::new();

    for f in &all {
        if !f.ends_with(".rs") {
            continue;
        }
        let in_dir = cfg.size.directories.iter().any(|d| {
            d == "." || f.starts_with(&format!("{d}/"))
        });
        if !in_dir {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(root.join(f)) else { continue };
        let sig: Vec<String> = text
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .filter(|l| !skip.iter().any(|re| re.is_match(l)))
            .map(|l| l.to_string())
            .collect();
        files.push((f.clone(), sig));
    }

    let total: usize = files.iter().map(|(_, s)| s.len()).sum();
    println!("size report (significant lines, report-only)");
    println!("  files: {}   significant lines: {total}", files.len());
    println!();

    // Large modules.
    let mut large: Vec<&(String, Vec<String>)> =
        files.iter().filter(|(_, s)| s.len() >= cfg.size.large).collect();
    large.sort_by_key(|(_, s)| std::cmp::Reverse(s.len()));
    println!("large modules (>= {} significant lines):", cfg.size.large);
    if large.is_empty() {
        println!("  none");
    }
    for (f, s) in large.iter().take(cfg.size.shown) {
        println!("  {:>6}  {f}", s.len());
    }
    println!();

    // Copy-paste: runs of `window` consecutive significant lines seen in two
    // different files.
    let window = cfg.size.window.max(2);
    let mut seen: HashMap<String, String> = HashMap::new();
    let mut runs: Vec<(String, String, usize)> = Vec::new();
    for (f, sig) in &files {
        if sig.len() < window {
            continue;
        }
        for i in 0..=sig.len() - window {
            let key = sig[i..i + window].join("\n");
            match seen.get(&key) {
                Some(other) if other != f => runs.push((other.clone(), f.clone(), window)),
                Some(_) => {}
                None => {
                    seen.insert(key, f.clone());
                }
            }
        }
    }
    println!("shared runs (>= {window} consecutive lines in two files):");
    if runs.is_empty() {
        println!("  none");
    }
    let mut shown = 0;
    let mut dedup: std::collections::HashSet<(String, String)> = Default::default();
    for (a, b, w) in &runs {
        let pair = if a < b { (a.clone(), b.clone()) } else { (b.clone(), a.clone()) };
        if dedup.insert(pair) {
            println!("  {a} <-> {b}  ({w}+ lines)");
            shown += 1;
            if shown >= cfg.size.shown {
                break;
            }
        }
    }
    Ok(())
}
