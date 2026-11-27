//! fs --- read, edit, and write inside a root, with edits you can take back.
//!
//! Every write is proved only where a snapshot can take it back: the first edit
//! to a file saves its original bytes, and `revert` restores them. Edits are
//! anchored by text, not line numbers, so they survive the file shifting under
//! them. Paths are confined to the root; the boundary is a copy, not a cage.

use std::collections::HashMap;
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::images::{Images, Kept};
use crate::tool::ToolError;

/// A workspace rooted at a directory, keeping snapshots for revert.
#[derive(Debug)]
pub struct Workspace {
    root: PathBuf,
    /// What was there before the first change to each file, or that it did not
    /// exist. Kept as an id when there is a store to wait in: a session that
    /// writes a large file does not keep a copy of it to the end.
    snapshots: HashMap<PathBuf, Kept>,
    /// Where a snapshot waits instead of in memory, when the caller has one.
    images: Option<Arc<Images>>,
}

/// A run of lines from one file, and what was left behind.
#[derive(Debug)]
pub struct Window {
    /// The lines asked for, without their line endings.
    pub lines: Vec<String>,
    /// How many lines the file has, so "there is more" is a number.
    pub total: usize,
    /// The line the window starts at, counting from one.
    pub from: usize,
    /// Whether the byte budget stopped the window before the line count did.
    pub bytes: bool,
}

/// What one read may put in the conversation. A tool result is appended to the
/// log and replayed into every request after it, so a budget here is paid once
/// and saved on every round that follows.
const READ_BYTES: usize = 64 * 1024;

impl Workspace {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
            snapshots: HashMap::new(),
            // An undo of a write is the same promise as an undo of a command,
            // so out of the box it waits on disk too. A store that cannot be
            // opened falls back to holding the bytes, which still undoes.
            images: Images::open().ok().map(Arc::new),
        }
    }

    /// Keep the snapshots on disk instead of in memory, in this store.
    pub fn with_images(mut self, images: Arc<Images>) -> Self {
        self.images = Some(images);
        self
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Resolve a relative path inside the root, refusing escapes.
    fn resolve(&self, rel: &str) -> Result<PathBuf, ToolError> {
        let joined = self.root.join(rel);
        let canonical = joined
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| self.root.clone());
        // Reject any component that climbs out of the root.
        let mut depth: i32 = 0;
        for comp in Path::new(rel).components() {
            match comp {
                std::path::Component::ParentDir => {
                    depth -= 1;
                    if depth < 0 {
                        return Err(ToolError::Rejected(format!("path escapes the root: {rel}")));
                    }
                }
                std::path::Component::Normal(_) => depth += 1,
                std::path::Component::RootDir | std::path::Component::Prefix(_) => {
                    return Err(ToolError::Rejected(format!(
                        "absolute path not allowed: {rel}"
                    )));
                }
                _ => {}
            }
        }
        let _ = canonical;
        Ok(joined)
    }

    fn snapshot(&mut self, path: &Path) -> Result<(), ToolError> {
        if !self.snapshots.contains_key(path) {
            // Bytes, not text: a file that is not readable as UTF-8 is still a
            // file the human may have written, and an undo of it must work.
            let original = if path.exists() {
                Some(std::fs::read(path)?)
            } else {
                None
            };
            let kept = Kept::save(self.images.as_deref(), original);
            self.snapshots.insert(path.to_path_buf(), kept);
        }
        Ok(())
    }

    /// Read a file inside the root.
    pub fn read(&self, rel: &str) -> Result<String, ToolError> {
        let path = self.resolve(rel)?;
        Ok(std::fs::read_to_string(path)?)
    }

    /// Read a run of lines from a file, and say how long the file is.
    ///
    /// The window is what is held: a file is streamed a line at a time and
    /// only the asked-for lines are kept, so reading twenty lines of a large
    /// file does not put the file in the conversation, the log, and every
    /// request after it. The count is carried on to the end because a reader
    /// who is told "there is more" without being told how much cannot decide
    /// whether to keep reading.
    pub fn read_window(
        &self,
        rel: &str,
        offset: usize,
        limit: usize,
    ) -> Result<Window, ToolError> {
        let path = self.resolve(rel)?;
        let file = std::fs::File::open(&path)?;
        let mut reader = std::io::BufReader::new(file);
        let from = offset.max(1);
        let mut lines: Vec<String> = Vec::new();
        let mut total = 0usize;
        let mut held = 0usize;
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::InvalidData => {
                    return Err(ToolError::Rejected(format!(
                        "{rel} is not text; this reads text files"
                    )));
                }
                Err(e) => return Err(ToolError::Io(e)),
            }
            total += 1;
            if total < from || lines.len() >= limit || held >= READ_BYTES {
                continue;
            }
            while line.ends_with('\n') || line.ends_with('\r') {
                line.pop();
            }
            held += line.len();
            lines.push(line);
        }
        Ok(Window {
            lines,
            total,
            from,
            bytes: held >= READ_BYTES,
        })
    }

    /// Replace a unique anchor text with new text, snapshotting for revert.
    pub fn edit(&mut self, rel: &str, anchor: &str, replacement: &str) -> Result<(), ToolError> {
        let path = self.resolve(rel)?;
        let content = std::fs::read_to_string(&path)?;
        let count = content.matches(anchor).count();
        if count == 0 {
            return Err(ToolError::AnchorNotFound);
        }
        if count > 1 {
            return Err(ToolError::AnchorAmbiguous);
        }
        self.snapshot(&path)?;
        let updated = content.replacen(anchor, replacement, 1);
        std::fs::write(&path, updated)?;
        Ok(())
    }

    /// Write a whole file, snapshotting its prior state for revert.
    pub fn write(&mut self, rel: &str, content: &str) -> Result<(), ToolError> {
        let path = self.resolve(rel)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        self.snapshot(&path)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    /// Restore one file to its pre-edit state.
    pub fn revert(&mut self, rel: &str) -> Result<(), ToolError> {
        let path = self.resolve(rel)?;
        if let Some(kept) = self.snapshots.remove(&path) {
            kept.restore(&path, self.images.as_deref())?;
        }
        Ok(())
    }

    /// Restore every touched file.
    pub fn revert_all(&mut self) -> Result<(), ToolError> {
        let paths: Vec<PathBuf> = self.snapshots.keys().cloned().collect();
        for path in paths {
            if let Some(kept) = self.snapshots.remove(&path) {
                kept.restore(&path, self.images.as_deref())?;
            }
        }
        Ok(())
    }
}
