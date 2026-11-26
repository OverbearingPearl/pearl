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
use windows_sys::Win32::Security::Authorization::{
    EXPLICIT_ACCESS_W, GRANT_ACCESS, GetNamedSecurityInfoW, SE_FILE_OBJECT, SetEntriesInAclW,
    SetNamedSecurityInfoW, TRUSTEE_IS_SID, TRUSTEE_IS_UNKNOWN, TRUSTEE_W,
};
use windows_sys::Win32::Security::Isolation::{
    CreateAppContainerProfile, DeriveAppContainerSidFromAppContainerName,
};
use windows_sys::Win32::Security::{
    DACL_SECURITY_INFORMATION, FreeSid, PROTECTED_DACL_SECURITY_INFORMATION, SECURITY_CAPABILITIES,
    SUB_CONTAINERS_AND_OBJECTS_INHERIT,
};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
    SetInformationJobObject,
};
use windows_sys::Win32::System::Threading::{
    CreateProcessW, EXTENDED_STARTUPINFO_PRESENT, GetExitCodeProcess, INFINITE,
    InitializeProcThreadAttributeList, PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES,
    PROCESS_INFORMATION, STARTUPINFOEXW, TerminateProcess, UpdateProcThreadAttribute,
    WaitForSingleObject,
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
pub fn spawn_confined(
    program: &str,
    args: &[String],
    policy: Policy,
    root: &str,
) -> io::Result<WinChild> {
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

        // An AppContainer profile: a unique SID that touches nothing ungranted.
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
        // A negative HRESULT means it failed; reuse the profile if it exists.
        if hr < 0 {
            let hr2 = DeriveAppContainerSidFromAppContainerName(name.as_ptr(), &mut sid);
            if hr2 < 0 || sid.is_null() {
                CloseHandle(job);
                return Err(io::Error::other(format!(
                    "AppContainer profile failed: {hr:#x}"
                )));
            }
        }

        // Reads everywhere, as on macOS and Linux: the same policy must mean
        // the same thing on every OS. An AppContainer touches nothing it was
        // not granted, so parity costs an ACE on each drive root -- see the
        // record on what that writes to the machine.
        for drive in read_roots(root) {
            let _ = grant_path(sid, &drive, false);
        }
        // Grant the workspace to the SID so a WorkspaceWrite command can write.
        if policy == Policy::WorkspaceWrite {
            let _ = grant_path(sid, root, true);
            for dir in crate::sandbox::scratch_dirs() {
                let _ = grant_path(sid, &dir.to_string_lossy(), true);
            }
        }

        let mut caps = SECURITY_CAPABILITIES {
            AppContainerSid: sid,
            Capabilities: std::ptr::null_mut(),
            CapabilityCount: 0,
            Reserved: 0,
        };

        // STARTUPINFOEX carrying the security capabilities attribute.
        let mut size: usize = 0;
        InitializeProcThreadAttributeList(std::ptr::null_mut(), 1, 0, &mut size);
        let mut buf = vec![0u8; size];
        let attr_list = buf.as_mut_ptr() as *mut _;
        if InitializeProcThreadAttributeList(attr_list, 1, 0, &mut size) == 0 {
            FreeSid(sid);
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
            FreeSid(sid);
            CloseHandle(job);
            return Err(last_error());
        }

        let mut si: STARTUPINFOEXW = std::mem::zeroed();
        si.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;
        si.lpAttributeList = attr_list;

        let mut command_line = wide(&build_command_line(program, args));
        let cwd = wide(root);
        let mut pi: PROCESS_INFORMATION = std::mem::zeroed();
        let ok = CreateProcessW(
            std::ptr::null(),
            command_line.as_mut_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            0,
            EXTENDED_STARTUPINFO_PRESENT,
            std::ptr::null(),
            cwd.as_ptr(),
            &si.StartupInfo,
            &mut pi,
        );
        FreeSid(sid);
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

fn grant_path(sid: *mut core::ffi::c_void, root: &str, write: bool) -> io::Result<()> {
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
        let rc = SetNamedSecurityInfoW(
            path.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
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
