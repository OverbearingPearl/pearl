//! sandbox --- confinement for the shell, native on every platform we ship.
//!
//! The boundary is a copy, not a cage: a policy confines what a command may
//! touch, but the command still runs as a process the user can see and kill.
//! macOS has Seatbelt (`sandbox-exec`); Linux has Landlock; Windows has an
//! AppContainer profile inside a Job Object. Each is the OS's own primitive,
//! not a container we reinvent.

use std::io;

#[cfg(target_os = "windows")]
#[path = "windows.rs"]
mod windows;
#[cfg(target_os = "linux")]
#[path = "linux.rs"]
mod linux;

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
    cfg!(any(target_os = "macos", target_os = "linux", target_os = "windows"))
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

/// Spawn `program` under `policy`, confined to writes under `root` where the
/// policy allows them. Returns the running child the caller can wait on.
pub fn spawn(program: &str, args: &[String], policy: Policy, root: &str) -> io::Result<ConfinedChild> {
    if policy == Policy::Full {
        return std::process::Command::new(program)
            .args(args)
            .current_dir(root)
            .spawn()
            .map(ConfinedChild::Std);
    }
    #[cfg(target_os = "macos")]
    {
        // Seatbelt matches resolved paths (/var -> /private/var); the profile
        // must name the canonical root or the subpath will not match.
        let resolved = std::fs::canonicalize(root).unwrap_or_else(|_| root.into());
        let root_str = resolved.to_string_lossy();
        let profile = seatbelt_profile(policy, &root_str);
        return std::process::Command::new("sandbox-exec")
            .arg("-p")
            .arg(profile)
            .arg(program)
            .args(args)
            .current_dir(&resolved)
            .spawn()
            .map(ConfinedChild::Std);
    }
    #[cfg(target_os = "windows")]
    {
        return windows::spawn_confined(program, args, policy, root).map(ConfinedChild::Win);
    }
    #[cfg(target_os = "linux")]
    {
        return linux::spawn_confined(program, args, policy, root).map(ConfinedChild::Std);
    }
    #[allow(unreachable_code)]
    {
        let _ = (policy, root);
        std::process::Command::new(program).args(args).current_dir(root).spawn().map(ConfinedChild::Std)
    }
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
