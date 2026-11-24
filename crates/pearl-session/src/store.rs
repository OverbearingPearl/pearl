//! store --- the append-only session log, one writer, resumable.
//!
//! A session file has one writer. Entries are appended as JSON lines and read
//! back in order; nothing is rewritten in place, so a kill and reopen finds the
//! history exactly where it stopped.

use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::entry::Entry;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("malformed log line {line}: {message}")]
    Malformed { line: usize, message: String },
}

type Result<T> = std::result::Result<T, StoreError>;

/// An append-only JSONL log at a path.
#[derive(Debug, Clone)]
pub struct SessionStore {
    path: PathBuf,
    next_seq: u64,
}

impl SessionStore {
    /// Open (or create) the log at `path`, counting existing records so the
    /// next append continues the sequence rather than restarting it.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let next_seq = if path.exists() { Self::read_all(&path)?.len() as u64 } else { 0 };
        OpenOptions::new().create(true).append(true).open(&path)?;
        Ok(Self { path, next_seq })
    }

    /// Append one entry, returning the sequence number it was written at.
    pub fn append(&mut self, role: crate::entry::Role, content: impl Into<String>) -> Result<u64> {
        let seq = self.next_seq;
        self.write(Entry::new(seq, role, content))
    }

    /// Append an assistant turn that asked for tools, with the calls it asked
    /// for, so a replay can answer them.
    pub fn append_asked(
        &mut self,
        calls: impl Into<String>,
        content: impl Into<String>,
    ) -> Result<u64> {
        let seq = self.next_seq;
        self.write(Entry::asked(seq, calls, content))
    }

    /// Append a tool turn, saying which call it answers.
    pub fn append_answered(
        &mut self,
        call: impl Into<String>,
        content: impl Into<String>,
    ) -> Result<u64> {
        let seq = self.next_seq;
        self.write(Entry::answered(seq, call, content))
    }

    /// Append the summary of a folded range, recording the range it covers so
    /// a resumed session can show the view without paying for it again.
    pub fn append_compaction(
        &mut self,
        content: impl Into<String>,
        covers: crate::entry::Coverage,
    ) -> Result<u64> {
        let seq = self.next_seq;
        self.write(Entry::compaction(seq, content, covers))
    }

    /// Write one record and advance the sequence.
    fn write(&mut self, entry: Entry) -> Result<u64> {
        let seq = entry.seq;
        let mut file = OpenOptions::new().append(true).open(&self.path)?;
        let line = serde_json::to_string(&entry).map_err(|e| StoreError::Malformed {
            line: seq as usize,
            message: e.to_string(),
        })?;
        writeln!(file, "{line}")?;
        self.next_seq += 1;
        Ok(seq)
    }

    /// Read every record back in order.
    pub fn read(&self) -> Result<Vec<Entry>> {
        Self::read_all(&self.path)
    }

    fn read_all(path: &Path) -> Result<Vec<Entry>> {
        if !path.exists() {
            return Ok(vec![]);
        }
        let file = File::open(path)?;
        let mut out = Vec::new();
        for (i, line) in BufReader::new(file).lines().enumerate() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let entry: Entry = serde_json::from_str(&line).map_err(|e| StoreError::Malformed {
                line: i + 1,
                message: e.to_string(),
            })?;
            out.push(entry);
        }
        Ok(out)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The directory holding this session log; derived sessions live beside it.
    pub fn dir(&self) -> PathBuf {
        self.path.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from("."))
    }
}
