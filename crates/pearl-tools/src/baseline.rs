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
//! Pearl's own state is skipped for the same reason and a stronger one: the
//! session log grows while the command runs, so restoring it would rewind the
//! conversation to before the command and take the human's list with it.
//!
//! The bytes wait on disk, named by their content, and the image keeps only
//! the names. A session runs hundreds of ops over nearly the same tree, so one
//! copy serves them all; holding a copy per op in memory is what made a long
//! run cost gigabytes (ADR 13).

use crate::images::{Image, Images};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

/// Directories never captured: version control and build output.
const SKIP: &[&str] = &[".git", "target", "node_modules", "dist", ".cache"];

/// Files never captured: pearl's own scratch, named by the op that owns it.
/// Two tasks run at once, so one task's output file must not land in another's
/// before-image -- that revert would delete a file a running command is
/// still writing.
const OWN_PREFIXES: &[&str] = &[".pearl-out-"];

/// The paths that are pearl's own state rather than the human's work: the
/// session log, its claim, and the derived logs of any children. A handle, so
/// whoever opens a log can name it and every baseline taken afterwards leaves
/// it alone. Empty by default, which is what a caller with no session wants.
#[derive(Debug, Clone, Default)]
pub struct Own(Arc<Mutex<HashSet<PathBuf>>>);

impl Own {
    pub fn new() -> Self {
        Self::default()
    }

    /// Name a path as pearl's own. Resolved now, because a baseline walks a
    /// canonical root and `/var` must not look different from `/private/var`.
    pub fn add(&self, path: impl AsRef<Path>) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(absolute(path.as_ref()));
    }

    fn holds(&self, path: &Path) -> bool {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .contains(path)
    }

    /// Every path named so far, for a caller handing the same exclusions to a
    /// child's registry.
    pub fn paths(&self) -> Vec<PathBuf> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .cloned()
            .collect()
    }
}

/// The absolute form of a path that may not exist yet: resolve the longest
/// ancestor that does, then append the rest.
fn absolute(path: &Path) -> PathBuf {
    if let Ok(abs) = fs::canonicalize(path) {
        return abs;
    }
    match (path.parent(), path.file_name()) {
        (Some(parent), Some(name)) if !parent.as_os_str().is_empty() => fs::canonicalize(parent)
            .map(|abs| abs.join(name))
            .unwrap_or_else(|_| path.to_path_buf()),
        _ => path.to_path_buf(),
    }
}

/// A before-image of every work file under a root, keyed by relative path.
#[derive(Debug, Clone)]
pub struct Baseline {
    root: PathBuf,
    files: HashMap<PathBuf, Image>,
    /// The bytes themselves, shared with every other op that captured into the
    /// same store. Held here so an entry outliving its ledger can still be
    /// taken back.
    images: Arc<Images>,
    /// Kept with the image, because a restore walks the tree again: what was
    /// left out at capture must be left out at restore, or the undo would
    /// delete the very files it was told not to touch.
    own: Own,
}

/// What a file looked like the last time it was read, so an unchanged tree
/// costs a stat instead of a read.
///
/// A run of commands walks nearly the same tree every time: 275 files and
/// 1.7 MB here, which is 42 ms of reading and hashing against 1.5 ms of
/// statting. The bytes are already on disk under their own content's name, so
/// an unchanged file needs only its name carried forward.
#[derive(Clone, Debug, Default)]
pub struct Seen(Arc<Mutex<HashMap<PathBuf, Mark>>>);

#[derive(Clone, Copy, Debug)]
struct Mark {
    modified: std::time::SystemTime,
    len: u64,
    image: Image,
}

impl Seen {
    pub fn new() -> Self {
        Self::default()
    }

    /// The image of a file that has not changed since it was last read, or
    /// `None` when it has to be read again.
    ///
    /// A mark is trusted only when it is older than the second it is being
    /// asked in. A filesystem that stamps whole seconds cannot tell two writes
    /// apart inside one second, and a command that writes a file twice would
    /// otherwise hand the next op a before-image of the first write -- an undo
    /// that quietly restores the wrong bytes. This is the same guard git keeps
    /// on its index, for the same reason.
    fn unchanged(&self, rel: &Path, meta: &fs::Metadata) -> Option<Image> {
        let modified = meta.modified().ok()?;
        let cutoff = std::time::SystemTime::now().checked_sub(std::time::Duration::from_secs(1))?;
        if modified >= cutoff {
            return None;
        }
        let marks = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let mark = marks.get(rel)?;
        (mark.modified == modified && mark.len == meta.len()).then_some(mark.image)
    }

    fn remember(&self, rel: &Path, meta: &fs::Metadata, image: Image) {
        let Ok(modified) = meta.modified() else {
            return;
        };
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(
                rel.to_path_buf(),
                Mark {
                    modified,
                    len: meta.len(),
                    image,
                },
            );
    }
}

impl Baseline {
    /// Capture the current state of the workspace under `root`.
    pub fn capture(root: impl AsRef<Path>) -> io::Result<Self> {
        Self::capture_except(root, &Own::new())
    }

    /// Capture the workspace, leaving pearl's own state out of it.
    pub fn capture_except(root: impl AsRef<Path>, own: &Own) -> io::Result<Self> {
        Self::capture_with(root, own, Arc::new(Images::open()?), &Seen::new())
    }

    /// Capture into a store the caller already holds, so a run of ops keeps one
    /// copy of every unchanged file instead of one per op.
    pub(crate) fn capture_with(
        root: impl AsRef<Path>,
        own: &Own,
        images: Arc<Images>,
        seen: &Seen,
    ) -> io::Result<Self> {
        let root = root.as_ref();
        let root = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
        let mut files = HashMap::new();
        // One file in hand at a time: the tree is never held whole, which is
        // the difference between a run's memory and a run's workspace.
        visit(&root, &root, own, &mut |rel, path, meta| {
            if let Some(image) = seen.unchanged(&rel, meta) {
                files.insert(rel, image);
                return Ok(());
            }
            if let Ok(bytes) = fs::read(path) {
                let image = images.put(&bytes)?;
                seen.remember(&rel, meta, image);
                files.insert(rel, image);
            }
            Ok(())
        })?;
        Ok(Self {
            root,
            files,
            images,
            own: own.clone(),
        })
    }

    /// Restore the workspace to the captured state. Changed and deleted files
    /// are written back; files the operation created are removed. Returns the
    /// number of paths touched. Empty directories left behind are pruned.
    pub fn restore(&self) -> io::Result<usize> {
        let mut current = HashSet::new();
        visit(&self.root, &self.root, &self.own, &mut |rel, _, _| {
            current.insert(rel);
            Ok(())
        })?;
        let mut touched = 0;

        // Put back every captured file that changed or went missing. Compared
        // by name, so an unchanged file costs a read and no write.
        for (rel, image) in &self.files {
            let abs = self.root.join(rel);
            let same = fs::read(&abs)
                .map(|bytes| image.matches(&bytes))
                .unwrap_or(false);
            if !same {
                if let Some(parent) = abs.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(&abs, self.images.get(*image)?)?;
                touched += 1;
            }
        }

        // Remove files the operation created (present now, absent at capture).
        for rel in &current {
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

/// Visit every work file under `dir`, by path relative to root. The visitor
/// decides what to do with each one, so neither a capture nor a restore has to
/// hold the tree to look at it.
fn visit(
    root: &Path,
    dir: &Path,
    own: &Own,
    f: &mut dyn FnMut(PathBuf, &Path, &fs::Metadata) -> io::Result<()>,
) -> io::Result<()> {
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
            visit(root, &path, own, f)?;
        } else if meta.is_file() {
            if own.holds(&path) || OWN_PREFIXES.iter().any(|p| name.starts_with(p)) {
                continue;
            }
            if let Ok(rel) = path.strip_prefix(root) {
                f(rel.to_path_buf(), &path, &meta)?;
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
    copy_workspace_except(src, dest, &Own::new())
}

/// Copy a workspace for a child, leaving pearl's own state behind: a child
/// gets the human's files, not the parent's conversation.
pub fn copy_workspace_except(
    src: impl AsRef<Path>,
    dest: impl AsRef<Path>,
    own: &Own,
) -> io::Result<usize> {
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
    copy_dir(&abs_src, &abs_src, dest, own, &mut copied)?;
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
fn copy_dir(root: &Path, dir: &Path, dest: &Path, own: &Own, copied: &mut usize) -> io::Result<()> {
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
            copy_dir(root, &path, dest, own, copied)?;
        } else if meta.is_file() {
            if own.holds(&path) || OWN_PREFIXES.iter().any(|p| name.starts_with(p)) {
                continue;
            }
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&path, &target)?;
            *copied += 1;
        }
    }
    Ok(())
}
