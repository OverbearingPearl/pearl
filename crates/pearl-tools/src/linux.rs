//! linux --- Landlock, the kernel's own unprivileged sandbox.
//!
//! Landlock lets a process restrict itself and every child it spawns, without
//! root. We build a ruleset in the child just before exec: reads are allowed
//! broadly, writes only under the workspace root. No container, no daemon.

use std::io;
use std::os::unix::process::CommandExt;
use std::process::Child;

use landlock::{
    ABI, Access, AccessFs, PathBeneath, PathFd, Ruleset, RulesetAttr, RulesetCreatedAttr,
};

use super::Policy;

/// Spawn `program` under a Landlock ruleset applied in the child before exec.
pub fn spawn_confined(
    program: &str,
    args: &[String],
    policy: Policy,
    root: &str,
    sink: super::Sink<'_>,
) -> io::Result<Child> {
    let abi = ABI::V3;
    let root = root.to_string();
    let mut command = std::process::Command::new(program);
    command.args(args).current_dir(&root);
    super::stdio(&mut command, sink)?;
    unsafe {
        command.pre_exec(move || {
            let ruleset = Ruleset::default()
                .handle_access(AccessFs::from_all(abi))
                .map_err(to_io)?
                .create()
                .map_err(to_io)?;
            // Reads everywhere.
            let ruleset = ruleset
                .add_rule(PathBeneath::new(
                    PathFd::new("/").map_err(to_io)?,
                    AccessFs::from_read(abi),
                ))
                .map_err(to_io)?;
            // The null device, for every policy: `2>/dev/null` writes nothing,
            // so refusing it fails a command that could not have changed
            // anything. The same goes for the descriptors a shell opens for
            // process substitution.
            let mut ruleset = ruleset;
            for device in [
                "/dev/null",
                "/dev/zero",
                "/dev/fd",
                "/dev/stdout",
                "/dev/stderr",
            ] {
                if let Ok(fd) = PathFd::new(device) {
                    ruleset = ruleset
                        .add_rule(PathBeneath::new(fd, AccessFs::from_all(abi)))
                        .map_err(to_io)?;
                }
            }
            // Writes only under the workspace root, when the policy allows.
            let ruleset = if policy == Policy::WorkspaceWrite {
                let mut r = ruleset
                    .add_rule(PathBeneath::new(
                        PathFd::new(&root).map_err(to_io)?,
                        AccessFs::from_all(abi),
                    ))
                    .map_err(to_io)?;
                // Scratch space: a script, a log, a compiler's intermediate.
                for dir in crate::sandbox::scratch_dirs() {
                    if let Ok(fd) = PathFd::new(&dir) {
                        r = r
                            .add_rule(PathBeneath::new(fd, AccessFs::from_all(abi)))
                            .map_err(to_io)?;
                    }
                }
                r
            } else {
                ruleset
            };
            ruleset.restrict_self().map_err(to_io)?;
            Ok(())
        });
    }
    command.spawn()
}

fn to_io(e: impl std::fmt::Display) -> io::Error {
    io::Error::other(e.to_string())
}
