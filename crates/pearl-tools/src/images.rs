//! images --- before-image bytes on disk, so the ledger holds ids, not files.
//!
//! A shell op keeps a before-image of every work file, and a session runs
//! hundreds of them. Held in memory that is the workspace multiplied by the
//! number of commands: one 142 MB workspace and 761 ops measured 23.68 GB. The
//! bytes are almost all identical from one op to the next, so they are written
//! once, by content, and every op keeps only the ids of the files it saw.
//!
//! Nothing about the promise changes: every op is still revertible, each op is
//! still independent, and the bytes that come back are the bytes that were
//! there. What changes is where they wait (ADR 13).
//!
//! The store lives in the system's temporary directory, beside nothing the
//! human owns, and is removed when the last holder goes. A blob that the
//! system cleaned away is an honest refusal, not a half-restored tree.

use std::fs;
use std::hash::{DefaultHasher, Hasher};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// One file's bytes, named by what they are. Small enough to keep one per
/// captured path per op: the id and the length, and nothing else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Image {
    id: u128,
    len: u64,
}

impl Image {
    /// The id of these bytes, without writing them anywhere.
    pub fn of(bytes: &[u8]) -> Self {
        Self {
            id: fingerprint(bytes),
            len: bytes.len() as u64,
        }
    }

    /// Whether `bytes` are exactly what this image names.
    pub fn matches(&self, bytes: &[u8]) -> bool {
        bytes.len() as u64 == self.len && fingerprint(bytes) == self.id
    }

    fn name(&self) -> String {
        format!("{:032x}", self.id)
    }
}

/// A 128-bit fingerprint from the standard library's hasher, run twice over
/// different inputs. No dependency for a number, and a collision would be
/// caught on read rather than restored as the wrong file.
fn fingerprint(bytes: &[u8]) -> u128 {
    let mut first = DefaultHasher::new();
    first.write(bytes);
    let mut second = DefaultHasher::new();
    second.write_u8(0xff);
    second.write(bytes);
    ((first.finish() as u128) << 64) | second.finish() as u128
}

static STORES: AtomicU64 = AtomicU64::new(0);

/// A directory of blobs, shared by every op that keeps one. Removed when the
/// last holder drops, so a finished session leaves nothing behind.
#[derive(Debug)]
pub struct Images {
    dir: PathBuf,
}

impl Images {
    /// A store of its own. The directory is created on the first write, so a
    /// run that never captures anything leaves no trace.
    pub fn open() -> io::Result<Self> {
        let n = STORES.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("pearl-undo-{}-{n}", std::process::id()));
        Ok(Self { dir })
    }

    /// Keep these bytes, returning what names them. Bytes already held are not
    /// written again: a hundred ops over an unchanged tree cost one copy.
    pub fn put(&self, bytes: &[u8]) -> io::Result<Image> {
        let image = Image::of(bytes);
        let path = self.dir.join(image.name());
        if path.exists() {
            return Ok(image);
        }
        fs::create_dir_all(&self.dir)?;
        // Written beside its name and moved into place, so a reader never sees
        // half a file and two ops writing the same bytes cannot collide.
        let temp = self.dir.join(format!("{}.part", image.name()));
        fs::write(&temp, bytes)?;
        fs::rename(&temp, &path)?;
        Ok(image)
    }

    /// The bytes this image names. An image whose blob is gone, or whose bytes
    /// are not the ones recorded, is an error the caller must report: taking
    /// an op back is all or nothing.
    pub fn get(&self, image: Image) -> io::Result<Vec<u8>> {
        let path = self.dir.join(image.name());
        let bytes = fs::read(&path).map_err(|e| {
            io::Error::new(
                e.kind(),
                format!(
                    "the before-image for this op is no longer on disk ({}): {e}",
                    path.display()
                ),
            )
        })?;
        if !image.matches(&bytes) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "the before-image on disk is not the bytes it was recorded as",
            ));
        }
        Ok(bytes)
    }
}

impl Drop for Images {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

/// One file's state before a change, as much of it as the holder must keep.
///
/// A shell op keeps one of these per path, and a write keeps one for the file
/// it replaced, so this is the shape of the whole memory bill: an id when the
/// bytes can wait on disk, the bytes themselves only when there is nowhere
/// else to put them.
#[derive(Debug, Clone)]
pub(crate) enum Kept {
    /// The file did not exist: taking the change back means removing it.
    Nothing,
    /// The bytes wait in a store.
    At(Image),
    /// No store, or its disk was full, so the bytes are held here. Undo still
    /// works; it costs what it used to cost.
    Held(Vec<u8>),
}

impl Kept {
    /// Save what was there, in the store when there is one.
    pub(crate) fn save(store: Option<&Images>, before: Option<Vec<u8>>) -> Self {
        match before {
            None => Kept::Nothing,
            Some(bytes) => match store {
                Some(store) => match store.put(&bytes) {
                    Ok(image) => Kept::At(image),
                    // The store is the cheap way, not the only way: a full
                    // disk may not turn into an undo that does not work.
                    Err(_) => Kept::Held(bytes),
                },
                None => Kept::Held(bytes),
            },
        }
    }

    /// Put it back, or say why it cannot be.
    pub(crate) fn restore(&self, path: &Path, store: Option<&Images>) -> io::Result<()> {
        match self {
            Kept::Nothing => {
                if path.exists() {
                    fs::remove_file(path)?;
                }
                Ok(())
            }
            Kept::Held(bytes) => fs::write(path, bytes),
            Kept::At(image) => match store {
                Some(store) => fs::write(path, store.get(*image)?),
                None => Err(io::Error::other(
                    "the before-image for this change is no longer on disk",
                )),
            },
        }
    }
}
