//! sandbox --- confinement for the shell, native on every platform we ship.
//!
//! The boundary is a copy, not a cage: a policy confines what a command may
//! touch, but the command still runs as a process the user can see and kill.
//! macOS has Seatbelt (`sandbox-exec`); Linux has Landlock; Windows has an
//! AppContainer profile inside a Job Object. Each is the OS's own primitive,
//! not a container we reinvent.

use std::io;
use std::path::Path;

#[cfg(target_os = "linux")]
#[path = "linux.rs"]
mod linux;
#[cfg(target_os = "windows")]
#[path = "windows.rs"]
mod windows;

/// What a command is allowed to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Policy {
    /// Read anything, write nothing.
    ReadOnly,
    /// Read anything, write only under the workspace root.
    WorkspaceWrite,
    /// No confinement; the command runs as the user.
    Full,
}

/// Whether this platform confines a command natively.
pub fn native_available() -> bool {
    cfg!(any(
        target_os = "macos",
        target_os = "linux",
        target_os = "windows"
    ))
}

/// A spawned child, whether the OS gave us a `std` handle or our own.
pub enum ConfinedChild {
    Std(std::process::Child),
    #[cfg(target_os = "windows")]
    Win(windows::WinChild),
}

impl ConfinedChild {
    /// The process id.
    pub fn id(&self) -> u32 {
        match self {
            Self::Std(c) => c.id(),
            #[cfg(target_os = "windows")]
            Self::Win(c) => c.id(),
        }
    }

    /// Wait for the process to exit.
    pub fn wait(&mut self) -> io::Result<()> {
        match self {
            Self::Std(c) => c.wait().map(|_| ()),
            #[cfg(target_os = "windows")]
            Self::Win(c) => c.wait(),
        }
    }

    /// Kill the process and, where the OS supports it, its tree.
    pub fn kill(&mut self) -> io::Result<()> {
        match self {
            Self::Std(c) => c.kill(),
            #[cfg(target_os = "windows")]
            Self::Win(c) => c.kill(),
        }
    }

    /// Wait for exit and return the exit code, where the OS reports one.
    pub fn wait_code(&mut self) -> io::Result<Option<i32>> {
        match self {
            Self::Std(c) => c.wait().map(|s| s.code()),
            #[cfg(target_os = "windows")]
            Self::Win(c) => {
                c.wait()?;
                c.exit_code().map(|code| code.map(|c| c as i32))
            }
        }
    }
}

/// Where a child's output goes.
///
/// A program that draws on the terminal owns every byte on it, because the
/// console's model of the screen is what it last drew there. Anything else
/// that writes to the terminal writes over that model, and the next frame
/// leaves it alone: half a line from a shell's complaint, a neighbour's
/// numbers, a border that will not come back until the screen is cleared. So a
/// child never gets the terminal. It writes to the capture the tool reads, or
/// nowhere.
#[derive(Debug, Clone, Copy)]
pub enum Sink<'a> {
    /// A file the caller reads afterwards. Opened before the shell parses, so
    /// a command the shell cannot read has its complaint land there too,
    /// instead of on the frame.
    File(&'a Path),
    /// Nothing: the caller wants the exit code and no words.
    None,
}

/// Give the child its stdio, and never the terminal.
fn stdio(cmd: &mut std::process::Command, sink: Sink<'_>) -> io::Result<()> {
    cmd.stdin(std::process::Stdio::null());
    match sink {
        Sink::None => {
            cmd.stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null());
        }
        Sink::File(path) => {
            let said = std::fs::File::create(path)?;
            let complained = said.try_clone()?;
            cmd.stdout(said).stderr(complained);
        }
    }
    Ok(())
}

/// Make the op's own two files writable under every policy.
///
/// A report the sandbox can refuse is no report: the human would read an exit
/// code and nothing else. Where the child inherits a handle this is already
/// true and there is nothing to do.
pub fn permit_report(root: &str, paths: &[&std::path::Path]) -> io::Result<()> {
    #[cfg(target_os = "windows")]
    return windows::permit_report(root, paths);
    #[allow(unreachable_code)]
    {
        let _ = (root, paths);
        Ok(())
    }
}

/// Spawn `program` under `policy`, confined to writes under `root` where the
/// policy allows them. Returns the running child the caller can wait on.
pub fn spawn(
    program: &str,
    args: &[String],
    policy: Policy,
    root: &str,
    sink: Sink<'_>,
) -> io::Result<ConfinedChild> {
    if policy == Policy::Full {
        let mut cmd = std::process::Command::new(program);
        cmd.args(args).current_dir(root);
        stdio(&mut cmd, sink)?;
        return cmd.spawn().map(ConfinedChild::Std);
    }
    #[cfg(target_os = "macos")]
    {
        // Seatbelt matches resolved paths (/var -> /private/var); the profile
        // must name the canonical root or the subpath will not match.
        let resolved = std::fs::canonicalize(root).unwrap_or_else(|_| root.into());
        let root_str = resolved.to_string_lossy();
        let profile = seatbelt_profile(policy, &root_str);
        let mut cmd = std::process::Command::new("sandbox-exec");
        cmd.arg("-p")
            .arg(profile)
            .arg(program)
            .args(args)
            .current_dir(&resolved);
        stdio(&mut cmd, sink)?;
        return cmd.spawn().map(ConfinedChild::Std);
    }
    #[cfg(target_os = "windows")]
    {
        // CreateProcessW is called with bInheritHandles = 0 and no
        // STARTF_USESTDHANDLES, so an AppContainer child inherits no std
        // handle at all and cannot reach the glass. Its capture is the batch
        // file's own redirect, made inside the caller -- so the capture is
        // ours to make writable, whatever the policy says about the tree.
        if let Sink::File(path) = sink {
            let _ = std::fs::File::create(path);
            let _ = windows::permit_report(root, &[path]);
        }
        return windows::spawn_confined(program, args, policy, root).map(ConfinedChild::Win);
    }
    #[cfg(target_os = "linux")]
    {
        return linux::spawn_confined(program, args, policy, root, sink).map(ConfinedChild::Std);
    }
    #[allow(unreachable_code)]
    {
        let _ = (policy, root);
        let mut cmd = std::process::Command::new(program);
        cmd.args(args).current_dir(root);
        stdio(&mut cmd, sink)?;
        cmd.spawn().map(ConfinedChild::Std)
    }
}

/// The words the OS uses when it refuses a write on the sandbox's behalf.
/// A refusal is text on stderr and nothing else -- no exit code is reserved
/// for it -- so the recogniser reads the words the OS itself uses.
const REFUSALS: &[&str] = &[
    // Seatbelt on macOS, and the same errno through a POSIX shell on Linux.
    "Operation not permitted",
    // Landlock on Linux, and a read-only mount.
    "Permission denied",
    "Read-only file system",
    // AppContainer on Windows.
    "Access is denied",
];

/// The phrase that shows the OS refused a write, when `policy` could have
/// refused one. `Full` confines nothing, so a refusal there came from the
/// filesystem and is the command's own business.
///
/// Words alone are not evidence: a file that talks about refusals, read out
/// by a command, prints the same words. The caller passes only a command that
/// failed, and even then the words count only in the shape the shell prints
/// them in -- `sh: /etc/x: Operation not permitted`, or `sh: x.txt: ...` for a
/// relative name -- a program, a path, and the phrase closing the line. Prose
/// that mentions a refusal does not end a line that way, and a line with no
/// path in it is a sentence about refusals rather than one.
///
/// This is a reading of the command's output, not a syscall audit: Seatbelt
/// and Landlock do not hand back the rule that said no, and `(debug deny)`
/// prints nothing on current macOS. The notice says what was refused and what
/// the policy allows, which is true of the denial the line names.
pub fn refusal(policy: Policy, output: &str) -> Option<&'static str> {
    if policy == Policy::Full {
        return None;
    }
    output.lines().find_map(diagnostic)
}

/// Whether a tool's own error carries a refusal.
///
/// The file tools fail inside this process, so their words come from our own
/// call and never from a file the task read: no shape is needed to tell them
/// from prose.
pub fn refused(policy: Policy, words: &str) -> Option<&'static str> {
    if policy == Policy::Full {
        return None;
    }
    REFUSALS.iter().copied().find(|said| words.contains(said))
}

/// The shell's report of one denial: a program's name, the path it was told
/// to write, and the OS's phrase at the end. `None` for any other line.
fn diagnostic(line: &str) -> Option<&'static str> {
    // `cmd` reports a denial as the phrase alone -- no program, no path -- so
    // on Windows a line that is nothing but the phrase is the report itself.
    // It is read only for a command that failed, which is what keeps a file
    // about the sandbox from being mistaken for one.
    #[cfg(windows)]
    if let Some(said) = REFUSALS
        .iter()
        .copied()
        .find(|said| line.trim().trim_end_matches('.') == said.trim_end_matches('.'))
    {
        return Some(said);
    }
    // A shell may close its sentence with a period or a quote; the phrase is
    // still the OS's, and the reader should not care which.
    let line = line.trim_end().trim_end_matches(['.', '\'', '"']);
    let said = REFUSALS
        .iter()
        .copied()
        .find(|said| line.ends_with(&format!(": {said}")))?;
    let head = line[..line.len() - said.len()]
        .trim_end()
        .strip_suffix(':')?;
    // The path is the last word the program named: every shell writes the
    // phrase after it, and what comes before can be a line number
    // (`sh: 1: cannot create inside.txt: Read-only file system`). A word is a
    // path when it carries a dot or a slash, which is what separates the
    // shell's report from a sentence about one.
    let path = head.split_whitespace().next_back()?;
    if !(path.contains('.') || path.contains('/')) {
        return None;
    }
    Some(said)
}

/// The directories a confined child may always write to, besides the
/// workspace. An agent that cannot write a scratch file cannot use the shell
/// for more than one-liners: a script, a log, a compiler's intermediate all
/// need somewhere to go that is not the human's tree.
///
/// Both spellings are named on macOS, where `/tmp` and `/var` are symlinks into
/// `/private`: a rule matches the path the process actually names, so granting
/// only one spelling leaves the other denied.
pub fn scratch_dirs() -> Vec<std::path::PathBuf> {
    let mut out: Vec<std::path::PathBuf> = Vec::new();
    let mut push = |p: std::path::PathBuf| {
        if let Ok(real) = std::fs::canonicalize(&p) {
            if !out.contains(&real) {
                out.push(real);
            }
        }
        if p.is_dir() && !out.contains(&p) {
            out.push(p);
        }
    };
    push(std::env::temp_dir());
    #[cfg(unix)]
    {
        push(std::path::PathBuf::from("/tmp"));
        // Not all of /var/folders: TMPDIR is the per-user, per-session leaf
        // under it, and granting the whole tree would hand over every other
        // application's scratch as well.
    }
    out.into_iter().filter(|p| p.is_dir()).collect()
}

/// A Seatbelt profile for the policy. Deny by default; allow reads broadly and
/// writes only under the root for `WorkspaceWrite`.
#[cfg(target_os = "macos")]
fn seatbelt_profile(policy: Policy, root: &str) -> String {
    let mut profile = String::from("(version 1)(deny default)(allow process*)(allow file-read*)");
    if policy == Policy::WorkspaceWrite {
        profile.push_str(&format!("(allow file-write* (subpath \"{root}\"))"));
        for dir in scratch_dirs() {
            let d = dir.to_string_lossy();
            profile.push_str(&format!("(allow file-write* (subpath \"{d}\"))"));
        }
    }
    // The null device writes nothing, and `2>/dev/null` is the commonest
    // redirect a shell command carries: refusing it fails work that could not
    // have damaged anything, and does so with a message about /dev/null rather
    // than about the command. The same goes for the descriptors a shell opens
    // for process substitution. This is a write the policy allows on purpose,
    // for every policy, because there is nothing on the other end of it.
    profile.push_str(
        "(allow file-write* (literal \"/dev/null\")(literal \"/dev/zero\") \
         (literal \"/dev/stdout\")(literal \"/dev/stderr\")(subpath \"/dev/fd\"))",
    );
    profile.push_str("(allow sysctl-read)(allow mach-lookup)");
    profile
}
