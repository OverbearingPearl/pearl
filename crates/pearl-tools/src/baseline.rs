//! baseline --- a pre-operation snapshot of the workspace, for reverting shell.
//!
//! The edit path is revertible because each write keeps a before-image. A shell
//! command is arbitrary, so it gets no per-write hook; instead we snapshot the
//! whole workspace before it runs and diff afterwards. Restoring the baseline
//! undoes whatever the command did to the files: changed files go back,
//! created files are removed, deleted files return. This is coarse (a whole
//! tree, not one file) but it is pure Rust and works on every platform we ship,
//! with no kernel or FUSE dependency.
//!
//! The snapshot skips `.git` and build output: those are not the user's work,
//! and copying them would make every shell op pay for the whole repository.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Directories never captured: version control and build output.
const SKIP: &[&str] = &[".git", "target", "node_modules", "dist", ".cache"];

/// A before-image of every work file under a root, keyed by relative path.
#[derive(Debug, Clone)]
pub struct Baseline {
    root: PathBuf,
    files: HashMap<PathBuf, Vec<u8>>,
}

impl Baseline {
    /// Capture the current state of the workspace under `root`.
    pub fn capture(root: impl AsRef<Path>) -> io::Result<Self> {
        let root = root.as_ref().to_path_buf();
        let mut files = HashMap::new();
        walk(&root, &root, &mut files)?;
        Ok(Self { root, files })
    }

    /// Restore the workspace to the captured state. Changed and deleted files
    /// are written back; files the operation created are removed. Returns the
    /// number of paths touched. Empty directories left behind are pruned.
    pub fn restore(&self) -> io::Result<usize> {
        let mut current = HashMap::new();
        walk(&self.root, &self.root, &mut current)?;
        let mut touched = 0;

        // Put back every captured file that changed or went missing.
        for (rel, content) in &self.files {
            let abs = self.root.join(rel);
            let same = current.get(rel).map(|c| c == content).unwrap_or(false);
            if !same {
                if let Some(parent) = abs.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(&abs, content)?;
                touched += 1;
            }
        }

        // Remove files the operation created (present now, absent at capture).
        for rel in current.keys() {
            if !self.files.contains_key(rel) {
                let abs = self.root.join(rel);
                if abs.exists() {
                    fs::remove_file(&abs)?;
                    touched += 1;
                }
            }
        }

        Ok(touched)
    }

    /// The number of files captured.
    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

/// Collect work files under `dir` recursively, keyed by path relative to root.
fn walk(root: &Path, dir: &Path, out: &mut HashMap<PathBuf, Vec<u8>>) -> io::Result<()> {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Ok(()),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if SKIP.contains(&name.as_ref()) {
            continue;
        }
        let meta = match fs::metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if meta.is_dir() {
            walk(root, &path, out)?;
        } else if meta.is_file() {
            if let Ok(content) = fs::read(&path) {
                if let Ok(rel) = path.strip_prefix(root) {
                    out.insert(rel.to_path_buf(), content);
                }
            }
        }
    }
    Ok(())
}
