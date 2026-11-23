//! The structural checks: everything that reads Rust, TOML or Markdown rather
//! than invoking a cargo tool. Each returns findings; `dispatch` prints them.
//!
//! The three tool checks (rustdoc, cargo-lint, build) are shell wrappers in
//! `checks/` that translate cargo's JSON diagnostics into the same records, so
//! the orchestrator treats every check identically.

use crate::config::Config;
use crate::files::{self, classify, tracked};
use crate::findings::{Finding, emit_all};
use anyhow::{Result, bail};
use regex::Regex;
use std::path::Path;

/// Run one structural check by name and emit its findings.
pub fn dispatch(
    name: &str,
    cfg: &Config,
    root: &Path,
    suite: &Path,
    rest: &[String],
) -> Result<()> {
    let files = positional(rest);
    let findings = match name {
        "blank-lines" => blank_lines(cfg, root, &files),
        "header-comment" => header_comment(cfg, root, &files),
        "comment-width" => comment_width(cfg, root, &files),
        "banned-construct" => banned_construct(cfg, root, &files),
        "meta-info" => meta_info(cfg, root),
        "module-name" => module_name(cfg, root, &files),
        "crate-prefix" => crate_prefix(cfg, root),
        "naming" => naming(cfg, root, &files),
        "test-suffix" => test_suffix(cfg, root, &files),
        "dependency-direction" => dependency_direction(cfg, root),
        "msrv" => msrv(cfg, root),
        "edition" => edition(cfg, root),
        "text-hygiene" => text_hygiene(cfg, root, &files),
        "policy" => policy(cfg, root),
        "project-neutral" => project_neutral(cfg, root, suite),
        "doc-links" => doc_links(cfg, root),
        "header-name" => header_name(cfg, root, &files),
        "adr-format" => adr_format(cfg, root),
        "layer-order" => layer_order(cfg, root),
        "stub-orphans" => stub_orphans(cfg, root, &files),
        other => bail!("unknown structural check: {other}"),
    };
    emit_all(&findings);
    Ok(())
}

/// Positional file arguments: everything that is not a flag or a flag's value.
fn positional(args: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut skip = false;
    for a in args {
        if skip {
            skip = false;
            continue;
        }
        if a == "--root" || a == "--suite" || a == "--class" {
            skip = true;
            continue;
        }
        if a.starts_with("--") {
            continue;
        }
        out.push(a.clone());
    }
    out
}

fn read(root: &Path, file: &str) -> Option<String> {
    std::fs::read_to_string(root.join(file)).ok()
}

fn lines(text: &str) -> Vec<&str> {
    text.split('\n').collect()
}

// --- 03 blank-lines -------------------------------------------------------

fn blank_lines(_cfg: &Config, root: &Path, files: &[String]) -> Vec<Finding> {
    let mut out = Vec::new();
    for f in files {
        let Some(text) = read(root, f) else { continue };
        let ls = lines(&text);
        let mut run = 0;
        let mut leading = true;
        for (i, line) in ls.iter().enumerate() {
            let blank = line.trim().is_empty();
            if leading && blank {
                out.push(Finding::error("blank-lines", f, i + 1, "file opens with a blank line"));
                leading = false;
            }
            if !blank {
                leading = false;
                run = 0;
            } else {
                run += 1;
                if run == 3 {
                    out.push(Finding::error("blank-lines", f, i + 1, "three consecutive blank lines"));
                }
            }
        }
    }
    out
}

// --- 04 header-comment ----------------------------------------------------

fn header_comment(cfg: &Config, root: &Path, files: &[String]) -> Vec<Finding> {
    let mut out = Vec::new();
    for f in files {
        let class = classify(cfg, f).unwrap_or("business");
        if class == "vendor" {
            continue;
        }
        let Some(text) = read(root, f) else { continue };
        let first = text.lines().find(|l| !l.trim().is_empty());
        let needs_doc = class == "business" || class == "main";
        match first {
            Some(l) if l.starts_with("//!") => {}
            Some(l) if l.starts_with("//") && !needs_doc => {}
            Some(l) if l.starts_with("//") => {
                out.push(Finding::error(
                    "header-comment",
                    f,
                    1,
                    "a business module opens with a `//!` module doc, not a plain comment",
                ));
            }
            _ => out.push(Finding::error(
                "header-comment",
                f,
                1,
                "file has no header comment",
            )),
        }
    }
    out
}

// --- 05 comment-width -----------------------------------------------------

fn comment_width(cfg: &Config, root: &Path, files: &[String]) -> Vec<Finding> {
    let mut out = Vec::new();
    for f in files {
        if classify(cfg, f) == Some("vendor") {
            continue;
        }
        let Some(text) = read(root, f) else { continue };
        for (i, line) in lines(&text).iter().enumerate() {
            let t = line.trim_start();
            let is_comment = t.starts_with("//") || t.starts_with("/*") || t.starts_with("*");
            if is_comment && line.chars().count() > cfg.comment_width && !line.contains("http") {
                out.push(Finding::error(
                    "comment-width",
                    f,
                    i + 1,
                    format!("comment is {} columns, limit is {}", line.chars().count(), cfg.comment_width),
                ));
            }
        }
    }
    out
}

// --- 06 banned-construct --------------------------------------------------

fn banned_construct(cfg: &Config, root: &Path, files: &[String]) -> Vec<Finding> {
    let mut out = Vec::new();
    let compiled: Vec<(Regex, &str)> = cfg
        .banned
        .iter()
        .filter_map(|b| Regex::new(&b.pattern).ok().map(|r| (r, b.suggestion.as_str())))
        .collect();
    for f in files {
        if classify(cfg, f) == Some("vendor") {
            continue;
        }
        let Some(text) = read(root, f) else { continue };
        for (i, line) in lines(&text).iter().enumerate() {
            for (re, suggestion) in &compiled {
                if re.is_match(line) {
                    out.push(Finding::error(
                        "banned-construct",
                        f,
                        i + 1,
                        format!("banned construct: {suggestion}"),
                    ));
                }
            }
        }
    }
    out
}

// --- 07 meta-info ---------------------------------------------------------

fn manifests(root: &Path) -> Vec<(String, toml::Value)> {
    let mut out = Vec::new();
    if let Ok(all) = tracked(root) {
        for f in all {
            if f.ends_with("Cargo.toml") {
                if let Some(text) = read(root, &f) {
                    if let Ok(v) = text.parse::<toml::Value>() {
                        out.push((f, v));
                    }
                }
            }
        }
    }
    out
}

fn meta_info(_cfg: &Config, root: &Path) -> Vec<Finding> {
    let mut out = Vec::new();
    for (f, v) in manifests(root) {
        // A pure workspace manifest has no [package]; only package manifests
        // owe the metadata.
        let Some(pkg) = v.get("package").and_then(|p| p.as_table()) else { continue };
        for key in ["name", "version", "license", "description", "edition"] {
            if !pkg.contains_key(key) {
                out.push(Finding::error("meta-info", &f, 1, format!("[package] is missing `{key}`")));
            }
        }
    }
    out
}

// --- 08 module-name -------------------------------------------------------

fn module_name(_cfg: &Config, _root: &Path, files: &[String]) -> Vec<Finding> {
    let mut out = Vec::new();
    let re = Regex::new(r"^[a-z0-9_\-]+$").unwrap();
    for f in files {
        if !f.ends_with(".rs") {
            continue;
        }
        let stem = Path::new(f).file_stem().and_then(|s| s.to_str()).unwrap_or("");
        if !re.is_match(stem) {
            out.push(Finding::error(
                "module-name",
                f,
                1,
                format!("module file name `{stem}` is not lower snake_case"),
            ));
        }
    }
    out
}

// --- 09 crate-prefix ------------------------------------------------------

fn crate_prefix(cfg: &Config, root: &Path) -> Vec<Finding> {
    let mut out = Vec::new();
    if cfg.crate_prefix.is_empty() {
        return out;
    }
    for (f, v) in manifests(root) {
        let Some(name) = v.get("package").and_then(|p| p.get("name")).and_then(|n| n.as_str()) else {
            continue;
        };
        // Classify the crate by where its source lives, not by the manifest
        // path: a Cargo.toml is not a Rust module, so probe a representative
        // source file under it. The suite's own helper under ci/ is tooling.
        let probe = f.replace("Cargo.toml", "src/lib.rs");
        let class = classify(cfg, &probe).unwrap_or("business");
        if (class == "business" || class == "main") && !name.starts_with(&cfg.crate_prefix) {
            out.push(Finding::error(
                "crate-prefix",
                &f,
                1,
                format!("business crate `{name}` does not carry the `{}` prefix", cfg.crate_prefix),
            ));
        }
    }
    out
}

// --- 10 naming ------------------------------------------------------------

fn naming(_cfg: &Config, root: &Path, files: &[String]) -> Vec<Finding> {
    let mut out = Vec::new();
    let ty = Regex::new(r"\b(?:struct|enum|trait|type)\s+([A-Za-z0-9_]+)").unwrap();
    let fnre = Regex::new(r"\bfn\s+([A-Za-z0-9_]+)").unwrap();
    let camel = Regex::new(r"^[A-Z][A-Za-z0-9]*$").unwrap();
    let snake = Regex::new(r"^[a-z_][a-z0-9_]*$").unwrap();
    for f in files {
        if classify(_cfg, f) == Some("vendor") {
            continue;
        }
        let Some(text) = read(root, f) else { continue };
        for (i, line) in lines(&text).iter().enumerate() {
            // Names live in code, not prose: a doc comment that says "the trait
            // keeps" is not a type declaration. Skip comment lines.
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") || trimmed.starts_with('*') || trimmed.starts_with("/*") {
                continue;
            }
            for cap in ty.captures_iter(line) {
                let name = &cap[1];
                if !camel.is_match(name) {
                    out.push(Finding::error("naming", f, i + 1, format!("type `{name}` is not CamelCase")));
                }
            }
            for cap in fnre.captures_iter(line) {
                let name = &cap[1];
                if !snake.is_match(name) {
                    out.push(Finding::error("naming", f, i + 1, format!("function `{name}` is not snake_case")));
                }
            }
        }
    }
    out
}

// --- 11 test-suffix -------------------------------------------------------

fn test_suffix(cfg: &Config, _root: &Path, files: &[String]) -> Vec<Finding> {
    let mut out = Vec::new();
    for f in files {
        if !f.ends_with(".rs") {
            continue;
        }
        let class = classify(cfg, f);
        let stem = Path::new(f).file_stem().and_then(|s| s.to_str()).unwrap_or("");
        // A file named like a test must live in the test class; a business
        // module must not wear the test suffix.
        let looks_test = stem.ends_with("-tests") || stem.ends_with("_tests");
        if looks_test && class != Some("test") {
            out.push(Finding::error("test-suffix", f, 1, "a `-tests` file is not in the test class"));
        }
    }
    out
}

// --- 12 dependency-direction ----------------------------------------------

fn workspace_crates(root: &Path) -> Vec<(String, String, Vec<String>)> {
    // (crate name, manifest path, dependency crate names) from cargo metadata.
    let Ok(meta) = cargo_metadata::MetadataCommand::new()
        .current_dir(root)
        .no_deps()
        .exec()
    else {
        return Vec::new();
    };
    let members: std::collections::HashSet<_> = meta.workspace_members.iter().collect();
    let mut out = Vec::new();
    for pkg in &meta.packages {
        if !members.contains(&pkg.id) {
            continue;
        }
        let manifest = pkg.manifest_path.to_string();
        let deps: Vec<String> = pkg
            .dependencies
            .iter()
            .map(|d| d.name.clone())
            .collect();
        out.push((pkg.name.clone(), manifest, deps));
    }
    out
}

fn class_of_manifest(cfg: &Config, root: &Path, manifest: &str) -> &'static str {
    let rel = Path::new(manifest)
        .strip_prefix(root)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| manifest.to_string());
    classify(cfg, &rel).unwrap_or("business")
}

fn dependency_direction(cfg: &Config, root: &Path) -> Vec<Finding> {
    let mut out = Vec::new();
    let crates = workspace_crates(root);
    let class_by_name: std::collections::HashMap<String, &'static str> = crates
        .iter()
        .map(|(name, manifest, _)| (name.clone(), class_of_manifest(cfg, root, manifest)))
        .collect();
    for (name, manifest, deps) in &crates {
        let class = class_by_name.get(name).copied().unwrap_or("business");
        let Some(profile) = cfg.profiles.get(class) else { continue };
        let rel = Path::new(manifest)
            .strip_prefix(root)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| manifest.clone());
        for dep in deps {
            let Some(dep_class) = class_by_name.get(dep) else { continue }; // external
            if !profile.may_require.iter().any(|c| c == dep_class) {
                out.push(Finding::error(
                    "dependency-direction",
                    &rel,
                    1,
                    format!("crate `{name}` ({class}) may not depend on `{dep}` ({dep_class})"),
                ));
            }
        }
    }
    out
}

// --- 13 msrv --------------------------------------------------------------

fn msrv(cfg: &Config, root: &Path) -> Vec<Finding> {
    let mut out = Vec::new();
    let floor = parse_ver(&cfg.msrv);
    for (f, v) in manifests(root) {
        if let Some(rv) = v.get("package").and_then(|p| p.get("rust-version")).and_then(|x| x.as_str()) {
            if parse_ver(rv) < floor {
                out.push(Finding::error(
                    "msrv",
                    &f,
                    1,
                    format!("rust-version {rv} is below the suite floor {}", cfg.msrv),
                ));
            }
        }
    }
    out
}

fn parse_ver(s: &str) -> (u64, u64, u64) {
    let mut it = s.split('.').map(|p| p.parse::<u64>().unwrap_or(0));
    (it.next().unwrap_or(0), it.next().unwrap_or(0), it.next().unwrap_or(0))
}

// --- 14 edition -----------------------------------------------------------

fn edition(cfg: &Config, root: &Path) -> Vec<Finding> {
    let mut out = Vec::new();
    for (f, v) in manifests(root) {
        if let Some(ed) = v.get("package").and_then(|p| p.get("edition")).and_then(|x| x.as_str()) {
            if ed != cfg.edition {
                out.push(Finding::error(
                    "edition",
                    &f,
                    1,
                    format!("edition {ed} differs from the configured {}", cfg.edition),
                ));
            }
        }
    }
    out
}

// --- 15 text-hygiene ------------------------------------------------------

fn text_hygiene(cfg: &Config, root: &Path, files: &[String]) -> Vec<Finding> {
    let mut out = Vec::new();
    for f in files {
        if classify(cfg, f) == Some("vendor") {
            continue;
        }
        let Some(text) = read(root, f) else { continue };
        if text.contains('\r') {
            out.push(Finding::error("text-hygiene", f, 1, "file has CRLF line endings"));
        }
        if !text.is_empty() && !text.ends_with('\n') {
            out.push(Finding::error("text-hygiene", f, lines(&text).len(), "file does not end with a newline"));
        }
        if text.ends_with("\n\n") {
            out.push(Finding::error("text-hygiene", f, lines(&text).len(), "file ends with more than one newline"));
        }
        for (i, line) in lines(&text).iter().enumerate() {
            let trimmed = line.trim_end_matches('\r');
            if trimmed != trimmed.trim_end() {
                out.push(Finding::error("text-hygiene", f, i + 1, "trailing whitespace"));
            }
            if trimmed.contains('\t') {
                out.push(Finding::warn("text-hygiene", f, i + 1, "tab character (rustfmt uses spaces)"));
            }
        }
    }
    out
}

// --- 16 policy ------------------------------------------------------------

fn policy(cfg: &Config, root: &Path) -> Vec<Finding> {
    let mut out = Vec::new();
    for class in files::CLASSES {
        if !cfg.profiles.contains_key(class) {
            out.push(Finding::error("policy", "config/ci.toml", 1, format!("class `{class}` has no profile")));
        }
    }
    let known: std::collections::HashSet<String> = workspace_crates(root).into_iter().map(|(n, _, _)| n).collect();
    for layer in &cfg.layers.order {
        if !cfg.layers.members.contains_key(layer) {
            out.push(Finding::error("policy", "config/ci.toml", 1, format!("layer `{layer}` is in the order but has no members")));
        }
    }
    for (layer, crates) in &cfg.layers.members {
        for c in crates {
            if !known.is_empty() && !known.contains(c) {
                out.push(Finding::error("policy", "config/ci.toml", 1, format!("layer `{layer}` names unknown crate `{c}`")));
            }
        }
    }
    for b in &cfg.banned {
        if Regex::new(&b.pattern).is_err() {
            out.push(Finding::error("policy", "config/ci.toml", 1, format!("banned pattern does not compile: {}", b.pattern)));
        }
    }
    out
}

// --- 17 project-neutral ---------------------------------------------------

fn project_neutral(cfg: &Config, root: &Path, suite: &Path) -> Vec<Finding> {
    let mut out = Vec::new();
    if cfg.crate_prefix.is_empty() {
        return out;
    }
    // The suite is generic: its own source must not name the project it happens
    // to be checking. Scan the suite checkout for the project prefix.
    let mut scan = |_dir: &Path, label: &str| {
        let Ok(entries) = tracked(root) else { return };
        for f in entries {
            let p = f.replace('\\', "/");
            if !(p.starts_with(&format!("{label}/")) || p == label) {
                continue;
            }
            if !(p.ends_with(".rs") || p.ends_with(".sh") || p.ends_with(".toml")) {
                continue;
            }
            if let Some(text) = read(root, &f) {
                for (i, line) in lines(&text).iter().enumerate() {
                    if line.contains(&cfg.crate_prefix) {
                        out.push(Finding::error(
                            "project-neutral",
                            &f,
                            i + 1,
                            format!("the suite names the project prefix `{}`; keep it generic", cfg.crate_prefix),
                        ));
                    }
                }
            }
        }
    };
    let _ = suite;
    scan(&root.join(&cfg.tooling_dir), &cfg.tooling_dir);
    scan(&root.join("tools"), "tools");
    out
}

// --- 18 doc-links ---------------------------------------------------------

fn doc_links(_cfg: &Config, root: &Path) -> Vec<Finding> {
    let mut out = Vec::new();
    let Ok(all) = tracked(root) else { return out };
    let link = Regex::new(r"\[[^\]]*\]\(([^)]+)\)").unwrap();
    for f in all {
        if !f.ends_with(".md") {
            continue;
        }
        let Some(text) = read(root, &f) else { continue };
        let dir = Path::new(&f).parent().unwrap_or(Path::new(""));
        for (i, line) in lines(&text).iter().enumerate() {
            for cap in link.captures_iter(line) {
                let target = &cap[1];
                if target.starts_with("http://") || target.starts_with("https://") || target.starts_with("mailto:") {
                    continue;
                }
                let (path_part, anchor) = match target.split_once('#') {
                    Some((p, a)) => (p, Some(a)),
                    None => (target, None),
                };
                if !path_part.is_empty() {
                    let resolved = root.join(dir).join(path_part);
                    if !resolved.exists() {
                        out.push(Finding::error("doc-links", &f, i + 1, format!("broken link target `{target}`")));
                        continue;
                    }
                }
                if let Some(anchor) = anchor {
                    let target_file = if path_part.is_empty() { root.join(&f) } else { root.join(dir).join(path_part) };
                    if let Ok(body) = std::fs::read_to_string(&target_file) {
                        if !has_anchor(&body, anchor) {
                            out.push(Finding::error("doc-links", &f, i + 1, format!("broken anchor `#{anchor}` in `{target}`")));
                        }
                    }
                }
            }
        }
    }
    out
}

fn has_anchor(body: &str, anchor: &str) -> bool {
    for line in body.lines() {
        if let Some(h) = line.strip_prefix('#') {
            let title = h.trim_start_matches('#').trim();
            let slug = title
                .to_lowercase()
                .chars()
                .map(|c| if c.is_alphanumeric() { c } else if c == ' ' || c == '-' { '-' } else { '\0' })
                .filter(|c| *c != '\0')
                .collect::<String>();
            if slug == anchor.to_lowercase() {
                return true;
            }
        }
    }
    false
}

// --- 19 header-name -------------------------------------------------------

fn header_name(_cfg: &Config, root: &Path, files: &[String]) -> Vec<Finding> {
    let mut out = Vec::new();
    let re = Regex::new(r"^//!\s*([a-z0-9_\-]+)\s+---").unwrap();
    for f in files {
        if !f.ends_with(".rs") {
            continue;
        }
        let Some(text) = read(root, f) else { continue };
        let stem = Path::new(f).file_stem().and_then(|s| s.to_str()).unwrap_or("");
        // lib.rs / main.rs / mod.rs name the crate or module in their header,
        // not the file stem; there is nothing to compare them against.
        if matches!(stem, "lib" | "main" | "mod") {
            continue;
        }
        if let Some(first) = text.lines().next() {
            if let Some(cap) = re.captures(first) {
                if &cap[1] != stem {
                    out.push(Finding::error("header-name", f, 1, format!("header names `{}` but the file is `{stem}`", &cap[1])));
                }
            }
        }
    }
    out
}

// --- 20 adr-format --------------------------------------------------------

fn adr_format(_cfg: &Config, root: &Path) -> Vec<Finding> {
    let mut out = Vec::new();
    let dir = root.join("docs/decisions");
    if !dir.is_dir() {
        return out;
    }
    let Ok(all) = tracked(root) else { return out };
    let num = Regex::new(r"^docs/decisions/(\d{4})-").unwrap();
    for f in all {
        let Some(cap) = num.captures(&f) else { continue };
        if &cap[1] == "0000" {
            continue;
        }
        let Some(text) = read(root, &f) else { continue };
        let n = cap[1].trim_start_matches('0');
        let n = if n.is_empty() { "0" } else { n };
        if !text.lines().any(|l| l.starts_with(&format!("# {n}. "))) {
            out.push(Finding::error("adr-format", &f, 1, format!("missing `# {n}. Title` heading")));
        }
        if !text.lines().any(|l| l.starts_with("Date: ")) {
            out.push(Finding::error("adr-format", &f, 1, "missing `Date: YYYY-MM-DD` line"));
        }
        for section in ["## Status", "## Context", "## Options considered", "## Decision", "## Consequences"] {
            if !text.lines().any(|l| l.trim() == section) {
                out.push(Finding::error("adr-format", &f, 1, format!("missing `{section}` section")));
            }
        }
        if !text.lines().any(|l| l.starts_with("Cost:")) {
            out.push(Finding::error("adr-format", &f, 1, "Consequences must end with a line beginning `Cost:`"));
        }
    }
    out
}

// --- 21 layer-order -------------------------------------------------------

fn layer_order(cfg: &Config, root: &Path) -> Vec<Finding> {
    let mut out = Vec::new();
    if cfg.layers.order.is_empty() {
        return out;
    }
    let crates = workspace_crates(root);
    for (name, manifest, deps) in &crates {
        let class = class_of_manifest(cfg, root, manifest);
        if class != "business" && class != "main" {
            continue;
        }
        let rel = Path::new(manifest)
            .strip_prefix(root)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| manifest.clone());
        let Some(layer) = cfg.layer_of(name) else {
            out.push(Finding::error("layer-order", &rel, 1, format!("crate `{name}` is in no layer; an undeclared module is one nobody placed")));
            continue;
        };
        let allowed = cfg.allowed_layers(layer);
        for dep in deps {
            let Some(dep_layer) = cfg.layer_of(dep) else { continue }; // external
            if !allowed.iter().any(|l| l == dep_layer) {
                out.push(Finding::error(
                    "layer-order",
                    &rel,
                    1,
                    format!("crate `{name}` (layer {layer}) may not depend on `{dep}` (layer {dep_layer})"),
                ));
            }
        }
    }
    out
}

// --- 22 stub-orphans ------------------------------------------------------

fn stub_orphans(cfg: &Config, root: &Path, files: &[String]) -> Vec<Finding> {
    let mut out = Vec::new();
    let re = Regex::new(r"\b(todo|unimplemented)!\s*\(").unwrap();
    for f in files {
        let class = classify(cfg, f);
        if class != Some("business") && class != Some("main") {
            continue;
        }
        let Some(text) = read(root, f) else { continue };
        for (i, line) in lines(&text).iter().enumerate() {
            if re.is_match(line) {
                out.push(Finding::error(
                    "stub-orphans",
                    f,
                    i + 1,
                    "a `todo!`/`unimplemented!` stub is left in business code; the story should pass against a real module",
                ));
            }
        }
    }
    out
}
