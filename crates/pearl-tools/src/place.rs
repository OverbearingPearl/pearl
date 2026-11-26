//! place --- the ground the work stands on: where, under what, on which branch.
//!
//! Three facts the human must not have to guess: which directory the agent's
//! writes land in, what the sandbox will allow there, and which branch a commit
//! would land on. Getting any of them wrong is the kind of mistake that is only
//! noticed after the files have changed.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::sandbox::Policy;

/// Where the work is, what it is allowed to do, and what it is standing on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Place {
    pub path: PathBuf,
    pub policy: Policy,
    pub branch: Option<String>,
    /// Files git reports as changed. `None` when git was not asked or did not
    /// answer -- an unknown count is not a clean tree.
    pub dirty: Option<usize>,
}

impl Place {
    /// Read the place. `dirty` asks git, so a caller that refreshes often
    /// should pass `false` and ask for it only now and then.
    pub fn read(root: &Path, policy: Policy, ask_git: bool) -> Self {
        let branch = branch_of(root);
        let dirty = if ask_git && branch.is_some() {
            dirty_count(root)
        } else {
            None
        };
        Self {
            path: root.to_path_buf(),
            policy,
            branch,
            dirty,
        }
    }

    /// A short line: `~/proj  git:main*3  sandbox:write`.
    pub fn describe(&self) -> String {
        self.items().join("  ")
    }

    /// The ground as whole items. The sandbox is a directory, so it is named
    /// as one: `write:~/proj` says both where the work is and where writes may
    /// land, and a policy that allows nothing says so in words.
    pub fn items(&self) -> Vec<String> {
        let here = shorten(&self.path);
        let mut out = match self.policy {
            // The scratch roots are writable too, and a boundary that names
            // only the workspace would be a smaller promise than the real one.
            Policy::WorkspaceWrite => vec![format!("write:{here}+tmp")],
            Policy::ReadOnly => vec![here.clone(), String::from("read-only")],
            Policy::Full => vec![here.clone(), String::from("full-access")],
        };
        if let Some(b) = &self.branch {
            let mark = match self.dirty {
                Some(0) => String::new(),
                Some(n) => format!("*{n}"),
                None => String::from("*?"),
            };
            out.push(format!("git:{b}{mark}"));
        }
        out
    }
}

/// What the policy allows, in the words of the promise rather than the type.
pub fn policy_name(policy: Policy) -> &'static str {
    match policy {
        Policy::ReadOnly => "read-only",
        Policy::WorkspaceWrite => "workspace-write",
        Policy::Full => "full-access",
    }
}

/// `$HOME` as `~`, so a deep path does not spend the row on a prefix everyone
/// already knows.
fn shorten(path: &Path) -> String {
    let text = path.to_string_lossy();
    if let Ok(home) = std::env::var("HOME") {
        if let Some(rest) = text.strip_prefix(&home) {
            return format!("~{rest}");
        }
    }
    text.into_owned()
}

/// The branch HEAD names, read from the file rather than by asking git: this
/// is read every refresh and a subprocess per frame is not a reading, it is a
/// cost. A detached HEAD gives the short sha, because that is what it is.
fn branch_of(root: &Path) -> Option<String> {
    let git = git_dir(root)?;
    let head = std::fs::read_to_string(git.join("HEAD")).ok()?;
    let head = head.trim();
    if let Some(r) = head.strip_prefix("ref: ") {
        return Some(r.rsplit('/').next()?.to_string());
    }
    if head.len() >= 7 {
        return Some(head[..7].to_string());
    }
    None
}

/// Where the repository's files are. `.git` is a directory in a normal clone
/// and a file naming one in a worktree or a submodule.
fn git_dir(root: &Path) -> Option<PathBuf> {
    let dot = root.join(".git");
    if dot.is_dir() {
        return Some(dot);
    }
    let text = std::fs::read_to_string(&dot).ok()?;
    let target = text.trim().strip_prefix("gitdir: ")?;
    let path = PathBuf::from(target);
    Some(if path.is_absolute() {
        path
    } else {
        root.join(path)
    })
}

/// How many paths git reports changed. Untracked files count: a file the agent
/// created is a change to the tree even before anyone adds it.
fn dirty_count(root: &Path) -> Option<usize> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("status")
        .arg("--porcelain")
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter(|l| !l.trim().is_empty())
            .count(),
    )
}
