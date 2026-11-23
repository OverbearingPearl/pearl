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
        let profile = seatbelt_profile(policy, root);
        return std::process::Command::new("sandbox-exec")
            .arg("-p")
            .arg(profile)
            .arg(program)
            .args(args)
            .current_dir(root)
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

/// A Seatbelt profile for the policy. Deny by default; allow reads broadly and
/// writes only under the root for `WorkspaceWrite`.
#[cfg(target_os = "macos")]
fn seatbelt_profile(policy: Policy, root: &str) -> String {
    let mut profile = String::from("(version 1)(deny default)(allow process*)(allow file-read*)");
    if policy == Policy::WorkspaceWrite {
        profile.push_str(&format!("(allow file-write* (subpath \"{root}\"))"));
    }
    profile.push_str("(allow sysctl-read)(allow mach-lookup)");
    profile
}
