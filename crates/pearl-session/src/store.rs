//! store --- the append-only session log, one writer, resumable.
//!
//! A session file has one writer. Entries are appended as JSON lines and read
//! back in order; nothing is rewritten in place, so a kill and reopen finds the
//! history exactly where it stopped.

use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use thiserror::Error;

use crate::entry::Entry;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("malformed log line {line}: {message}")]
    Malformed { line: usize, message: String },
    #[error("nothing to resume in {path}: no launch has been recorded there")]
    Nothing { path: PathBuf },
    #[error(
        "one window per directory: {path} is in use by another PEARL. Close it, or remove {lock} if it is really gone"
    )]
    Busy { path: PathBuf, lock: PathBuf },
}

/// What a claim says about the process that took it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Holder {
    /// The process is there: the claim is live and the directory is taken.
    Runs,
    /// The process is gone: a window killed by its terminal, whose claim the
    /// next window may clear.
    Gone,
    /// Nothing here can say. The claim stands, because a guess that it does
    /// not is the guess that costs a running window its log.
    Unknown,
}

/// The claim one process holds on a session file.
///
/// A log with one writer is what lets a launch hand the queue back on the next
/// start: two processes restoring the same list would run the same tasks twice
/// and charge for both. The claim is a file beside the log, created exclusively
/// and removed when the last handle closes. A process that dies without
/// cleaning up leaves it behind, so the words name the file to remove.
#[derive(Debug)]
struct Lease {
    path: PathBuf,
}

impl Drop for Lease {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

impl Lease {
    fn take(lock: PathBuf, log: &Path) -> Result<Arc<Self>> {
        Self::take_once(lock, log, 0)
    }

    /// What the process named by a claim is doing.
    ///
    /// `kill -0` answers for a zombie too, and a window closed by its terminal
    /// manager is exactly the case this is for, so the process's state is read:
    /// a dead one is a `Z`, and one that no longer exists prints nothing.
    ///
    /// The three answers are not two. A probe that fails says nothing about
    /// the process -- a machine may have no `ps`, and a confinement may
    /// refuse to run one -- and reading that failure as "gone" cleared the
    /// claim of a window that was running, which is this file's one promise
    /// gone quietly. So an unreadable claim and an unanswerable probe are
    /// their own answer, and the claim stands.
    fn holder(pid: &str) -> Holder {
        // Our own claim is a pid and nothing else, so a line that is not
        // digits was left by something that did not finish writing.
        if pid.is_empty() || !pid.bytes().all(|byte| byte.is_ascii_digit()) {
            return Holder::Unknown;
        }
        // `ps` is not every platform's question to ask. Where it is not, the
        // claim stands: a window that cannot be checked is still somebody's,
        // and the words name the file to remove once the human knows it is
        // not.
        #[cfg(windows)]
        let seen = std::process::Command::new("tasklist")
            .args(["/NH", "/FI", &format!("PID eq {pid}")])
            .output();
        #[cfg(not(windows))]
        let seen = std::process::Command::new("ps")
            .args(["-o", "stat=", "-p", pid])
            .stderr(std::process::Stdio::null())
            .output();
        let out = match seen {
            Ok(out) => out,
            Err(_) => return Holder::Unknown,
        };
        let stat = String::from_utf8_lossy(&out.stdout);
        let stat = stat.trim();
        #[cfg(windows)]
        {
            // `tasklist` prints one line per match and the words "No tasks"
            // when there is none.
            return if stat.contains(pid) {
                Holder::Runs
            } else if stat.is_empty() || stat.contains("No tasks") {
                Holder::Gone
            } else {
                Holder::Unknown
            };
        }
        #[allow(unreachable_code)]
        if stat.is_empty() {
            // "No such process" is an empty line and status 1. Any other
            // status is `ps` complaining about the question, not answering
            // it.
            return if out.status.code() == Some(1) {
                Holder::Gone
            } else {
                Holder::Unknown
            };
        }
        if stat.starts_with('Z') {
            Holder::Gone
        } else {
            Holder::Runs
        }
    }

    fn take_once(lock: PathBuf, log: &Path, tries: usize) -> Result<Arc<Self>> {
        match OpenOptions::new().write(true).create_new(true).open(&lock) {
            Ok(mut file) => {
                let _ = writeln!(file, "{}", std::process::id());
                Ok(Arc::new(Self { path: lock }))
            }
            // A window killed by its terminal leaves the claim behind. The
            // claim names the process that took it, so a claim whose process is
            // gone is cleared rather than made the human's problem -- and only
            // one that is gone: a claim nobody can read, and a claim nobody
            // here can check, is still somebody's, and the words say which
            // file to remove once the human knows it is not.
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists && tries == 0 => {
                let holder = std::fs::read_to_string(&lock).unwrap_or_default();
                if Self::holder(holder.trim()) == Holder::Gone {
                    let _ = std::fs::remove_file(&lock);
                    Self::take_once(lock, log, 1)
                } else {
                    Err(StoreError::Busy {
                        path: log.to_path_buf(),
                        lock,
                    })
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Err(StoreError::Busy {
                path: log.to_path_buf(),
                lock,
            }),
            Err(e) => Err(e.into()),
        }
    }
}

type Result<T> = std::result::Result<T, StoreError>;

/// An append-only JSONL log at a path.
#[derive(Debug, Clone)]
pub struct SessionStore {
    path: PathBuf,
    /// Shared by every clone: the runner and the queue's watcher both append,
    /// and two counters started from the same total would write the same
    /// sequence twice.
    next_seq: Arc<Mutex<u64>>,
    /// The topic the next turn belongs to. Set by the runner before it asks a
    /// task for words, so no append call has to carry it: every turn written
    /// while this is set is stamped with it.
    topic: Option<String>,
    /// The task the next turn is spoken for. Stamped like the topic, and read
    /// by the view: a turn belongs to a task, and the task's place on the list
    /// decides which conversation reads it.
    task: Option<String>,
    /// Held by every clone and released when the last one goes: the log's one
    /// writer, and the reason a default launch may restore the last list.
    _lease: Arc<Lease>,
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
        let next_seq = if path.exists() {
            Self::read_all(&path)?.len() as u64
        } else {
            0
        };
        OpenOptions::new().create(true).append(true).open(&path)?;
        let lock = path.with_extension("lock");
        let lease = Lease::take(lock, &path)?;
        Ok(Self {
            path,
            next_seq: Arc::new(Mutex::new(next_seq)),
            topic: None,
            task: None,
            _lease: lease,
        })
    }

    /// Append one entry, returning the sequence number it was written at.
    pub fn append(&mut self, role: crate::entry::Role, content: impl Into<String>) -> Result<u64> {
        let seq = self.take_seq();
        self.write(Entry::new(seq, role, content))
    }

    /// Append the queue as it stands, for a resume to restore (ADR 29).
    pub fn append_queue(&mut self, snapshot: impl Into<String>) -> Result<u64> {
        let seq = self.take_seq();
        self.write(Entry::new(seq, crate::entry::Role::Queue, snapshot))
    }

    /// The next sequence number, taken under the shared lock.
    fn take_seq(&mut self) -> u64 {
        let mut next = self
            .next_seq
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let seq = *next;
        *next += 1;
        seq
    }

    /// The newest queue snapshot of the conversation the last launch began.
    ///
    /// A snapshot belongs to the run that wrote it. Reading the newest one in
    /// the file would hand a fresh launch the list from before its own launch
    /// mark -- the one thing a fresh launch promises not to do.
    pub fn last_queue(&self) -> Result<Option<String>> {
        let entries = self.read()?;
        let began = entries
            .iter()
            .rposition(|entry| entry.role == crate::entry::Role::Launch)
            .map(|at| at + 1)
            .unwrap_or(0);
        Ok(entries
            .into_iter()
            .skip(began)
            .filter(|entry| entry.role == crate::entry::Role::Queue)
            .next_back()
            .map(|entry| entry.content))
    }

    /// Append an assistant turn that asked for tools, with the calls it asked
    /// for, so a replay can answer them.
    pub fn append_asked(
        &mut self,
        calls: impl Into<String>,
        content: impl Into<String>,
    ) -> Result<u64> {
        let seq = self.take_seq();
        self.write(Entry::asked(seq, calls, content))
    }

    /// Append a tool turn, saying which call it answers.
    pub fn append_answered(
        &mut self,
        call: impl Into<String>,
        content: impl Into<String>,
    ) -> Result<u64> {
        let seq = self.take_seq();
        self.write(Entry::answered(seq, call, content))
    }

    /// Append the summary of a folded range, recording the range it covers so
    /// a resumed session can show the view without paying for it again.
    pub fn append_compaction(
        &mut self,
        content: impl Into<String>,
        covers: crate::entry::Coverage,
    ) -> Result<u64> {
        let seq = self.take_seq();
        self.write(Entry::compaction(seq, content, covers))
    }

    /// Append the mark that a conversation begins here.
    ///
    /// A launch writes one before its first request, so the view a model is
    /// shown starts at this run and not at the oldest thing the file holds.
    /// `--resume` is the one launch that writes none.
    pub fn append_launch(&mut self) -> Result<u64> {
        let seq = self.take_seq();
        self.write(Entry::new(seq, crate::entry::Role::Launch, ""))
    }

    /// Write one record and advance the sequence.
    /// Work from now on belongs to `topic`: the id of the topic, or `None`
    /// for the shared line.
    pub fn in_topic(&mut self, topic: Option<String>) {
        self.topic = topic;
    }

    /// Work from now on is spoken for `task`, the id of the row it is done for.
    pub fn in_task(&mut self, task: Option<String>) {
        self.task = task;
    }

    fn write(&mut self, mut entry: Entry) -> Result<u64> {
        if entry.topic.is_none()
            && !matches!(
                entry.role,
                crate::entry::Role::Queue | crate::entry::Role::Launch
            )
        {
            entry.topic = self.topic.clone();
            entry.task = self.task.clone();
        }
        let seq = entry.seq;
        let mut file = OpenOptions::new().append(true).open(&self.path)?;
        let line = serde_json::to_string(&entry).map_err(|e| StoreError::Malformed {
            line: seq as usize,
            message: e.to_string(),
        })?;
        writeln!(file, "{line}")?;
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
        self.path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."))
    }
}
