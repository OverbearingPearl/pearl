//! fs --- read, edit, and write inside a root, with edits you can take back.
//!
//! Every write is proved only where a snapshot can take it back: the first edit
//! to a file saves its original bytes, and `revert` restores them. Edits are
//! anchored by text, not line numbers, so they survive the file shifting under
//! them. Paths are confined to the root; the boundary is a copy, not a cage.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::tool::ToolError;

/// A workspace rooted at a directory, keeping snapshots for revert.
#[derive(Debug)]
pub struct Workspace {
    root: PathBuf,
    /// The original bytes of each touched file, or `None` if it did not exist.
    snapshots: HashMap<PathBuf, Option<String>>,
}

impl Workspace {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self { root: root.as_ref().to_path_buf(), snapshots: HashMap::new() }
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
                    return Err(ToolError::Rejected(format!("absolute path not allowed: {rel}")));
                }
                _ => {}
            }
        }
        let _ = canonical;
        Ok(joined)
    }

    fn snapshot(&mut self, path: &Path) -> Result<(), ToolError> {
        if !self.snapshots.contains_key(path) {
            let original = if path.exists() { Some(std::fs::read_to_string(path)?) } else { None };
            self.snapshots.insert(path.to_path_buf(), original);
        }
        Ok(())
    }

    /// Read a file inside the root.
    pub fn read(&self, rel: &str) -> Result<String, ToolError> {
        let path = self.resolve(rel)?;
        Ok(std::fs::read_to_string(path)?)
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
        if let Some(original) = self.snapshots.remove(&path) {
            match original {
                Some(bytes) => std::fs::write(&path, bytes)?,
                None => {
                    if path.exists() {
                        std::fs::remove_file(&path)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Restore every touched file.
    pub fn revert_all(&mut self) -> Result<(), ToolError> {
        let paths: Vec<PathBuf> = self.snapshots.keys().cloned().collect();
        for path in paths {
            if let Some(original) = self.snapshots.remove(&path) {
                match original {
                    Some(bytes) => std::fs::write(&path, bytes)?,
                    None => {
                        if path.exists() {
                            std::fs::remove_file(&path)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
