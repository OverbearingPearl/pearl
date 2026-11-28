//! footprint --- what this process is holding, read from the OS.
//!
//! A long run keeps a log, and the log is read whole on every request. If
//! that grows into something the machine feels, the human should be able to see
//! it before the swap does. Each OS is asked directly: no dependency, and no
//! estimate standing in for a number the kernel already has.

/// Resident memory in bytes, or `None` where this OS is not asked yet.
pub fn rss_bytes() -> Option<u64> {
    imp::rss_bytes()
}

/// A reading for a status line: `42M`, `1.2G`. Whole megabytes below a
/// gigabyte, because a byte count on a status line is noise.
pub fn describe() -> Option<String> {
    let bytes = rss_bytes()?;
    let mb = bytes as f64 / (1024.0 * 1024.0);
    if mb >= 1024.0 {
        Some(format!("{:.1}G", mb / 1024.0))
    } else {
        Some(format!("{}M", mb.round() as u64))
    }
}

#[cfg(target_os = "linux")]
mod imp {
    /// `/proc/self/status` reports VmRSS in kB, which avoids assuming a page
    /// size the way statm would.
    pub fn rss_bytes() -> Option<u64> {
        let text = std::fs::read_to_string("/proc/self/status").ok()?;
        let line = text.lines().find(|l| l.starts_with("VmRSS:"))?;
        let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
        Some(kb * 1024)
    }
}

#[cfg(target_os = "macos")]
mod imp {
    // Declared here rather than pulled in: libSystem is already linked, and a
    // reading of one number does not earn a dependency.
    #[repr(C)]
    #[derive(Default)]
    struct MachTaskBasicInfo {
        virtual_size: u64,
        resident_size: u64,
        resident_size_max: u64,
        user_time: [u32; 2],
        system_time: [u32; 2],
        policy: i32,
        suspend_count: i32,
    }

    unsafe extern "C" {
        static mut mach_task_self_: u32;
        fn task_info(
            target: u32,
            flavor: u32,
            info: *mut MachTaskBasicInfo,
            count: *mut u32,
        ) -> i32;
    }

    // MACH_TASK_BASIC_INFO, not TASK_BASIC_INFO_64: the latter answers with
    // the 32-bit layout and a success code, so the read looks fine and every
    // field after the first is garbage.
    const MACH_TASK_BASIC_INFO: u32 = 20;

    pub fn rss_bytes() -> Option<u64> {
        let mut info = MachTaskBasicInfo::default();
        let mut count = (std::mem::size_of::<MachTaskBasicInfo>() / 4) as u32;
        // SAFETY: the pointer is to a live, correctly sized struct, the count
        // is its size in integers, and the task port is our own.
        let kr = unsafe {
            let port = *std::ptr::addr_of!(mach_task_self_);
            task_info(port, MACH_TASK_BASIC_INFO, &mut info, &mut count)
        };
        if kr != 0 {
            return None;
        }
        Some(info.resident_size)
    }
}

#[cfg(target_os = "windows")]
mod imp {
    use windows_sys::Win32::System::ProcessStatus::{
        GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcess;

    pub fn rss_bytes() -> Option<u64> {
        // windows-sys structs are plain C layouts without Default.
        let mut counters: PROCESS_MEMORY_COUNTERS = unsafe { std::mem::zeroed() };
        counters.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        // SAFETY: the struct is live and its cb field tells the API its size;
        // the handle is the current process pseudo-handle.
        let ok = unsafe { GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, counters.cb) };
        if ok == 0 {
            return None;
        }
        Some(counters.WorkingSetSize as u64)
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
mod imp {
    pub fn rss_bytes() -> Option<u64> {
        None
    }
}
