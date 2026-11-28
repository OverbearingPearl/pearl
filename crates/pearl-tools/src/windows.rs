//! windows --- an AppContainer profile inside a Job Object.
//!
//! Windows has no Seatbelt and no Landlock, but it has two primitives that
//! together make a sandbox: an AppContainer profile gives the process a unique
//! low-privilege SID that touches nothing it was not granted, and a Job Object
//! with kill-on-close takes the whole process tree down when the handle closes.
//! The approach follows Microsoft's MIT-licensed MXC `job_object.rs`; the FFI
//! ours, against `windows-sys`.

use std::io;

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
use windows_sys::Win32::Security::Authorization::ConvertStringSidToSidW;
use windows_sys::Win32::Foundation::{HANDLE_FLAG_INHERIT, SetHandleInformation};
use windows_sys::Win32::Security::Authorization::{
    EXPLICIT_ACCESS_W, GRANT_ACCESS, GetNamedSecurityInfoW, SE_FILE_OBJECT, SetEntriesInAclW,
    SetNamedSecurityInfoW, TRUSTEE_IS_SID, TRUSTEE_IS_UNKNOWN, TRUSTEE_W,
};
use windows_sys::Win32::Security::Isolation::{
    CreateAppContainerProfile, DeriveAppContainerSidFromAppContainerName,
};
use windows_sys::Win32::Security::{
    DACL_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION, PSID, SECURITY_CAPABILITIES,
    SID_AND_ATTRIBUTES, SUB_CONTAINERS_AND_OBJECTS_INHERIT,
};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
    SetInformationJobObject,
};
use windows_sys::Win32::System::Threading::{
    CreateProcessW, EXTENDED_STARTUPINFO_PRESENT, GetExitCodeProcess, INFINITE,
    InitializeProcThreadAttributeList, PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES,
    PROCESS_INFORMATION, STARTF_USESTDHANDLES, STARTUPINFOEXW, TerminateProcess,
    UpdateProcThreadAttribute, WaitForSingleObject,
};

use super::Policy;

/// A confined process: its handle and the Job Object that owns its tree.
pub struct WinChild {
    process: HANDLE,
    job: HANDLE,
    pid: u32,
}

impl WinChild {
    pub fn id(&self) -> u32 {
        self.pid
    }

    /// Wait for the process to exit.
    pub fn wait(&mut self) -> io::Result<()> {
        unsafe {
            let rc = WaitForSingleObject(self.process, INFINITE);
            if rc != WAIT_OBJECT_0 {
                return Err(io::Error::other(format!("WaitForSingleObject: {rc}")));
            }
        }
        Ok(())
    }

    /// Kill the process; the Job Object takes its children with it.
    pub fn kill(&mut self) -> io::Result<()> {
        unsafe {
            if TerminateProcess(self.process, 1) == 0 {
                return Err(io::Error::last_os_error());
            }
        }
        Ok(())
    }

    /// The exit code, once the process has exited.
    pub fn exit_code(&self) -> io::Result<Option<u32>> {
        unsafe {
            let mut code: u32 = 0;
            if GetExitCodeProcess(self.process, &mut code) == 0 {
                return Err(io::Error::last_os_error());
            }
            // STILL_ACTIVE (259) means it has not exited.
            if code == 259 {
                Ok(None)
            } else {
                Ok(Some(code))
            }
        }
    }
}

impl Drop for WinChild {
    fn drop(&mut self) {
        unsafe {
            // Closing the job handle fires kill-on-close for any live children.
            CloseHandle(self.job);
            CloseHandle(self.process);
        }
    }
}

/// A wide, null-terminated string for the Win32 W APIs.
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn last_error() -> io::Error {
    io::Error::last_os_error()
}

/// Spawn `program` confined to an AppContainer profile and a Job Object.
/// A trace of the sandbox's own start-up, when `PEARL_SANDBOX_TRACE` names a
/// file: which step of a session's first command costs the seconds the human
/// waits for. Empty, the sandbox writes nothing.
#[cfg(windows)]
fn trace(step: &str, started: std::time::Instant) {
    let Ok(path) = std::env::var("PEARL_SANDBOX_TRACE") else {
        return;
    };
    use std::io::Write;
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(file, "{step}: {:?}", started.elapsed());
    }
}

pub fn spawn_confined(
    program: &str,
    args: &[String],
    policy: Policy,
    root: &str,
    sink: super::Sink<'_>,
) -> io::Result<WinChild> {
    let started = std::time::Instant::now();
    // The capture crosses as an inherited handle: asking the shell to open
    // the file itself put a quoted path inside a quoted argument, and a
    // command that never parsed wrote nothing at all.
    let capture = match sink {
        super::Sink::File(path) => Some(std::fs::File::create(path)?),
        _ => None,
    };
    unsafe {
        // A Job Object that kills the whole tree when its handle closes.
        let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if job.is_null() {
            return Err(last_error());
        }
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        if SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &limits as *const _ as *const _,
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        ) == 0
        {
            CloseHandle(job);
            return Err(last_error());
        }

        trace("job object", started);
        // An AppContainer profile: a unique SID that touches nothing ungranted.
        // One SID per workspace for the whole process: making or deriving it
        // per command was most of what a Windows run spent its time on.
        let sid = match sid_for(root) {
            Ok(sid) => sid,
            Err(e) => {
                CloseHandle(job);
                return Err(e);
            }
        };

        trace("profile and sid", started);
        // Reads everywhere, as on macOS and Linux: the same policy must mean
        // the same thing on every OS. An AppContainer touches nothing it was
        // not granted, so parity costs an ACE on each drive root -- see the
        // record on what that writes to the machine.
        // A drive root is the one place where writing a descriptor is not a
        // write: the ACE has to reach the objects already there, so setting
        // it walks the whole volume. `C:\` already grants ALL APPLICATION
        // PACKAGES read, which every container has, so the drives the
        // workspace is not on are left alone; the workspace's own drive is
        // reached through the ancestors of what is granted below.
        for drive in read_roots(root) {
            if std::path::Path::new(&drive).parent().is_some() {
                let _ = granted_path(sid, &drive, false, false);
            }
        }
        // The programs themselves. An AppContainer touches nothing it was not
        // granted, and a drive root's ACE is written protected so it does not
        // walk a drive's worth of objects: nothing below a drive root is
        // reached by that grant. The loader needs to read `msys-2.0.dll`
        // beside `cat.exe` and `git.exe`, and `cargo` lives in the user's own
        // profile, so the trees the programs stand in are granted here, with
        // inheritance, once per profile.
        trace("drive reads", started);
        // Nothing more is granted here: the capability above makes the trees
        // a program stands in readable, which is what the hand-written ACEs
        // were buying at the price of a propagated write per tree.
        trace("program reads", started);
        // Grant the workspace to the SID so a WorkspaceWrite command can write.
        // The null device is a promise of every policy (ADR 22): a command
        // that throws its errors away writes nothing, and failing it teaches
        // the model to stop redirecting. Its DACL is the device's, not ours,
        // so this is an attempt: a machine that refuses costs nothing.
        for null in ["\\\\.\\NUL", "NUL"] {
            let _ = granted_path(sid, null, true, false);
        }
        if policy == Policy::WorkspaceWrite {
            // Inheriting, so a file that was in the workspace before the
            // command ran is writable too and not only the ones it creates.
            let _ = granted_path(sid, root, true, true);
            for dir in crate::sandbox::scratch_dirs() {
                let _ = granted_path(sid, &dir.to_string_lossy(), true, true);
            }
        }

        trace("workspace writes", started);
        // `ALL APPLICATION PACKAGES`: every container has it, and it is what
        // makes the system's own trees -- `System32`, `Program Files`, and
        // everything on PATH -- readable without an ACE per tree. Granting
        // those by hand costs minutes of a first command, because an
        // inheriting ACE is propagated to every object already there.
        let mut all_packages: PSID = std::ptr::null_mut();
        let named: Vec<u16> = "S-1-15-2-1\0".encode_utf16().collect();
        if ConvertStringSidToSidW(named.as_ptr(), &mut all_packages) == 0 {
            return Err(std::io::Error::last_os_error());
        }
        let mut capability_sids = [SID_AND_ATTRIBUTES {
            Sid: all_packages,
            Attributes: 0,
        }];
        let mut caps = SECURITY_CAPABILITIES {
            AppContainerSid: sid,
            Capabilities: capability_sids.as_mut_ptr(),
            CapabilityCount: capability_sids.len() as u32,
            Reserved: 0,
        };

        // STARTUPINFOEX carrying the security capabilities attribute.
        let mut size: usize = 0;
        InitializeProcThreadAttributeList(std::ptr::null_mut(), 1, 0, &mut size);
        let mut buf = vec![0u8; size];
        let attr_list = buf.as_mut_ptr() as *mut _;
        if InitializeProcThreadAttributeList(attr_list, 1, 0, &mut size) == 0 {
            CloseHandle(job);
            return Err(last_error());
        }
        if UpdateProcThreadAttribute(
            attr_list,
            0,
            PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES as usize,
            &mut caps as *mut _ as *const _,
            std::mem::size_of::<SECURITY_CAPABILITIES>(),
            std::ptr::null_mut(),
            std::ptr::null(),
        ) == 0
        {
            CloseHandle(job);
            return Err(last_error());
        }

        let mut si: STARTUPINFOEXW = std::mem::zeroed();
        si.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;
        si.lpAttributeList = attr_list;
        let mut inherit = 0;
        if let Some(file) = &capture {
            use std::os::windows::io::AsRawHandle;
            let handle = file.as_raw_handle() as HANDLE;
            let _ = SetHandleInformation(handle, HANDLE_FLAG_INHERIT as u32, HANDLE_FLAG_INHERIT);
            si.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
            si.StartupInfo.hStdOutput = handle;
            si.StartupInfo.hStdError = handle;
            inherit = 1;
        }

        let mut command_line = wide(&build_command_line(program, args));
        let cwd = wide(root);
        let mut pi: PROCESS_INFORMATION = std::mem::zeroed();
        let ok = CreateProcessW(
            std::ptr::null(),
            command_line.as_mut_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            inherit,
            EXTENDED_STARTUPINFO_PRESENT,
            std::ptr::null(),
            cwd.as_ptr(),
            &si.StartupInfo,
            &mut pi,
        );
        // The SID is cached for the process, so it is never freed here: the
        // next command reuses this pointer, and a freed one corrupted the heap
        // under the second confinement (0xC0000374).
        if ok == 0 {
            CloseHandle(job);
            return Err(last_error());
        }
        AssignProcessToJobObject(job, pi.hProcess);
        CloseHandle(pi.hThread);
        Ok(WinChild {
            process: pi.hProcess,
            job,
            pid: pi.dwProcessId,
        })
    }
}

/// A stable profile name derived from the workspace root.
/// Let the confined child write the two files its own report needs.
///
/// On macOS and Linux the parent opens the capture and hands the child the
/// handle, so a read-only workspace still produces a report. An AppContainer
/// child inherits no handle and redirects for itself: under a policy that
/// grants no write, the redirect is itself refused and every command comes
/// back as a bare exit code with no words at all. The op's script and its
/// capture are ours and not the human's tree, so they are writable under
/// every policy, for the same reason the null device is.
/// What has already been granted, so the ACL is written once per path.
fn granted() -> &'static std::sync::Mutex<std::collections::HashSet<String>> {
    static GRANTED: std::sync::OnceLock<std::sync::Mutex<std::collections::HashSet<String>>> =
        std::sync::OnceLock::new();
    GRANTED.get_or_init(|| std::sync::Mutex::new(std::collections::HashSet::new()))
}

/// The AppContainer SID for `root`, made or derived once per process.
///
/// Creating a profile writes to the registry and every ACL write is a system
/// call: doing either on every command made a Windows run of the suite take
/// minutes longer than the work it measured.
fn sid_for(root: &str) -> io::Result<*mut core::ffi::c_void> {
    static SIDS: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, usize>>> =
        std::sync::OnceLock::new();
    let cache = SIDS.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    if let Some(found) = cache.lock().unwrap().get(root) {
        return Ok(*found as *mut core::ffi::c_void);
    }
    // SAFETY: the name is null-terminated; a null SID is checked before use.
    unsafe {
        let name = wide(&profile_name(root));
        let mut sid = std::ptr::null_mut();
        let hr = CreateAppContainerProfile(
            name.as_ptr(),
            name.as_ptr(),
            name.as_ptr(),
            std::ptr::null(),
            0,
            &mut sid,
        );
        if hr < 0 || sid.is_null() {
            let hr = DeriveAppContainerSidFromAppContainerName(name.as_ptr(), &mut sid);
            if hr < 0 || sid.is_null() {
                return Err(io::Error::other(format!(
                    "AppContainer sid failed: {hr:#x}"
                )));
            }
        }
        // The SID outlives every command: it is cached, never freed.
        cache.lock().unwrap().insert(root.to_string(), sid as usize);
        Ok(sid)
    }
}

pub fn permit_report(root: &str, paths: &[&std::path::Path]) -> io::Result<()> {
    let sid = sid_for(root)?;
    let _ = granted_path(sid, root, false, false);
    for path in paths {
        // Our own two files are read and written by the child: the script has
        // to be opened, the report has to be made.
        let _ = granted_path(sid, &path.to_string_lossy(), false, false);
        let _ = granted_path(sid, &path.to_string_lossy(), true, false);
    }
    Ok(())
}

/// Grant once per path: a second identical ACL write changes nothing and
/// costs a system call per command.
fn granted_path(
    sid: *mut core::ffi::c_void,
    path: &str,
    write: bool,
    inherit: bool,
) -> io::Result<()> {
    // Keyed by SID: two workspaces are two AppContainers, and a grant one
    // profile holds means nothing to the other.
    let key = format!("{sid:p}:{write}:{inherit}:{path}");
    // One ACL write at a time. Replacing a directory's DACL leaves a window
    // in which the object has the new-but-incomplete list, and a command
    // spawning in that window is told "Access is denied" for a path that was
    // granted a moment earlier -- which is exactly the flake the temp-log
    // story kept showing. The lock is held across the grant, not across the
    // command.
    static WRITING: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _writing = WRITING.lock().unwrap();
    if granted().lock().unwrap().contains(&key) {
        return Ok(());
    }
    let written = grant_path(sid, path, write, inherit);
    // Remembered only once the machine took it: a grant that failed is tried
    // again by the next command instead of being believed forever.
    if written.is_ok() {
        granted().lock().unwrap().insert(key);
    }
    written
}

fn profile_name(root: &str) -> String {
    let hash = root
        .bytes()
        .fold(0u64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u64));
    format!("pearl.sandbox.{hash:016x}")
}

/// Build a command line for CreateProcessW (argv[0] plus arguments).
fn build_command_line(program: &str, args: &[String]) -> String {
    let mut out = quote(program);
    for a in args {
        out.push(' ');
        out.push_str(&quote(a));
    }
    out
}

/// Quote one argument the way `CommandLineToArgvW` parses it: wrap in quotes
/// when it is empty or holds a blank, double the backslashes that sit before a
/// quote, and double trailing backslashes so they cannot escape the closer.
fn quote(s: &str) -> String {
    if !s.is_empty() && !s.contains([' ', '\t', '\n', '\u{b}', '"']) {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    let mut backslashes = 0usize;
    for c in s.chars() {
        match c {
            '\\' => {
                backslashes += 1;
                out.push('\\');
            }
            '"' => {
                for _ in 0..=backslashes {
                    out.push('\\');
                }
                backslashes = 0;
                out.push('"');
            }
            _ => {
                backslashes = 0;
                out.push(c);
            }
        }
    }
    for _ in 0..backslashes {
        out.push('\\');
    }
    out.push('"');
    out
}

/// Grant the AppContainer SID change access to a path (best effort).
/// The drive roots a confined child may read: the workspace's own drive and
/// the system drive, which is where the toolchain lives.
fn read_roots(root: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut push = |p: String| {
        if p.len() >= 2 && p.as_bytes()[1] == b':' {
            let drive = format!("{}\\", &p[..2]);
            if !out.contains(&drive) {
                out.push(drive);
            }
        }
    };
    push(root.to_string());
    if let Ok(sys) = std::env::var("SystemDrive") {
        push(sys);
    }
    out
}

/// The directories the programs a command may name stand in: every entry on
/// PATH, the three Git directories that hold an executable or the library
/// beside it, Node, and the Rust toolchain's own `bin` directories. Leaves,
/// not trees: an inheritable ACE is written to every object under the path it
/// is set on, and granting `Git` or `.rustup` whole cost minutes per launch.

fn grant_path(
    sid: *mut core::ffi::c_void,
    root: &str,
    write: bool,
    inherit: bool,
) -> io::Result<()> {
    // SAFETY: the SID comes from a live AppContainer profile; the wide path is
    // null-terminated; the ACL pointers are checked before use.
    unsafe {
        let path = wide(root);
        let mut dacl = std::ptr::null_mut();
        let mut sd = std::ptr::null_mut();
        let rc = GetNamedSecurityInfoW(
            path.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut dacl,
            std::ptr::null_mut(),
            &mut sd,
        );
        if rc != 0 {
            return Err(io::Error::other(format!("GetNamedSecurityInfo: {rc}")));
        }
        let mut trustee: TRUSTEE_W = std::mem::zeroed();
        trustee.TrusteeForm = TRUSTEE_IS_SID;
        trustee.TrusteeType = TRUSTEE_IS_UNKNOWN;
        trustee.ptstrName = sid as *mut u16;
        let mut access: EXPLICIT_ACCESS_W = std::mem::zeroed();
        // FILE_ALL_ACCESS, or FILE_GENERIC_READ when the grant is read-only:
        // a drive root is granted so paths resolve and files can be read,
        // never so the tree can be changed.
        access.grfAccessPermissions = if write { 0x001F_01FF } else { 0x0012_0089 };
        access.grfAccessMode = GRANT_ACCESS;
        access.grfInheritance = SUB_CONTAINERS_AND_OBJECTS_INHERIT;
        access.Trustee = trustee;
        let mut new_dacl = std::ptr::null_mut();
        let rc = SetEntriesInAclW(1, &access, dacl, &mut new_dacl);
        if rc != 0 {
            return Err(io::Error::other(format!("SetEntriesInAcl: {rc}")));
        }
        // A protected descriptor does not inherit, which is what keeps the
        // drive roots' grant from walking a drive. A tree the child must read
        // through is written unprotected so the ACE travels to its children.
        let info = if inherit {
            DACL_SECURITY_INFORMATION
        } else {
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION
        };
        let rc = SetNamedSecurityInfoW(
            path.as_ptr(),
            SE_FILE_OBJECT,
            info,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            new_dacl,
            std::ptr::null_mut(),
        );
        if rc != 0 {
            return Err(io::Error::other(format!("SetNamedSecurityInfo: {rc}")));
        }
        Ok(())
    }
}
