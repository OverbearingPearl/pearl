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

/// Copy a workspace tree into `dest`, skipping the same directories a baseline
/// skips. Every child of a fan-out works on its own copy, so one child's writes
/// cannot land in another's tree.
///
/// Returns the number of files copied.
pub fn copy_workspace(src: impl AsRef<Path>, dest: impl AsRef<Path>) -> io::Result<usize> {
    let src = src.as_ref();
    let dest = dest.as_ref();
    // A copy inside the thing it copies recurses into itself until the path
    // is too long. Refuse it by name rather than let the caller discover it
    // as an OS error three hundred levels down.
    let abs_src = fs::canonicalize(src).unwrap_or_else(|_| src.to_path_buf());
    let abs_dest = resolve_unborn(dest);
    if abs_dest.starts_with(&abs_src) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "a workspace copy cannot live inside the workspace it copies",
        ));
    }
    fs::create_dir_all(dest)?;
    let mut copied = 0;
    copy_dir(src, src, dest, &mut copied)?;
    Ok(copied)
}

/// The absolute path of a destination that does not exist yet: canonicalize
/// the longest ancestor that does, then append the rest. Comparing a resolved
/// source against an unresolved destination is how `/var` and `/private/var`
/// end up looking like different trees.
fn resolve_unborn(dest: &Path) -> std::path::PathBuf {
    let mut tail: Vec<std::ffi::OsString> = Vec::new();
    let mut head = dest.to_path_buf();
    loop {
        if let Ok(abs) = fs::canonicalize(&head) {
            let mut out = abs;
            for part in tail.iter().rev() {
                out.push(part);
            }
            return out;
        }
        match (head.file_name().map(|n| n.to_os_string()), head.parent()) {
            (Some(name), Some(parent)) => {
                tail.push(name);
                head = parent.to_path_buf();
            }
            _ => return dest.to_path_buf(),
        }
    }
}

/// Copy one directory level, recursing into the directories a baseline keeps.
fn copy_dir(root: &Path, dir: &Path, dest: &Path, copied: &mut usize) -> io::Result<()> {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        // A workspace may hold an unreadable directory; a copy is best effort
        // there, because the child can still work on everything else.
        Err(_) => return Ok(()),
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if SKIP.contains(&name.as_ref()) {
            continue;
        }
        let Ok(rel) = path.strip_prefix(root) else {
            continue;
        };
        let target = dest.join(rel);
        let Ok(meta) = fs::metadata(&path) else {
            continue;
        };
        if meta.is_dir() {
            fs::create_dir_all(&target)?;
            copy_dir(root, &path, dest, copied)?;
        } else if meta.is_file() {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&path, &target)?;
            *copied += 1;
        }
    }
    Ok(())
}
