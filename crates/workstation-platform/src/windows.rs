use workstation_core::*;

#[cfg(windows)]
mod native {
    use super::*;
    use crate::{now, Error, Result};
    use std::{
        mem::{size_of, zeroed},
        os::windows::{ffi::OsStrExt, io::AsRawHandle},
        path::Path,
        process::Child,
        ptr,
    };
    use windows_sys::Win32::{
        Foundation::{
            CloseHandle, GetLastError, ERROR_NO_MORE_FILES, FILETIME, HANDLE, INVALID_HANDLE_VALUE,
        },
        Storage::FileSystem::{
            GetDiskFreeSpaceExW, GetDriveTypeW, GetVolumeInformationW, GetVolumePathNameW,
        },
        System::{
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
                TH32CS_SNAPPROCESS,
            },
            JobObjects::{
                AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
                SetInformationJobObject, TerminateJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
                JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            },
            ProcessStatus::{
                GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX,
            },
            Threading::{GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION},
        },
    };
    fn wide(p: &Path) -> Vec<u16> {
        p.as_os_str().encode_wide().chain(Some(0)).collect()
    }
    fn text(w: &[u16]) -> String {
        String::from_utf16_lossy(&w[..w.iter().position(|x| *x == 0).unwrap_or(w.len())])
    }
    struct Owned(HANDLE);
    impl Drop for Owned {
        fn drop(&mut self) {
            // SAFETY: only valid, uniquely-owned Win32 handles are stored in Owned.
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
    pub struct Containment {
        job: Owned,
    }
    impl Containment {
        pub fn attach(child: &Child) -> Result<Self> {
            // SAFETY: null security attributes create a non-inherited job; all pointers are valid for the call.
            unsafe {
                let h = CreateJobObjectW(ptr::null(), ptr::null());
                if h.is_null() {
                    return Err(Error::new("JOB_CREATE_FAILED"));
                }
                let job = Owned(h);
                let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = zeroed();
                limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                if SetInformationJobObject(
                    job.0,
                    JobObjectExtendedLimitInformation,
                    &limits as *const _ as *const _,
                    size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                ) == 0
                {
                    return Err(Error::new("JOB_CONFIG_FAILED"));
                }
                if AssignProcessToJobObject(job.0, child.as_raw_handle() as HANDLE) == 0 {
                    return Err(Error::new("JOB_ASSIGN_FAILED"));
                }
                Ok(Self { job })
            }
        }
        pub fn terminate(&self) {
            // SAFETY: this job contains only our stdin-gated worker and its descendants.
            unsafe {
                TerminateJobObject(self.job.0, 6);
            }
        }
    }
    pub fn restrict_home_acl(path: &Path) -> Result<()> {
        use windows_sys::Win32::{
            Foundation::LocalFree,
            Security::Authorization::{
                ConvertStringSecurityDescriptorToSecurityDescriptorW, SetNamedSecurityInfoW,
                SE_FILE_OBJECT,
            },
            Security::{
                GetSecurityDescriptorDacl, DACL_SECURITY_INFORMATION,
                PROTECTED_DACL_SECURITY_INFORMATION,
            },
        };
        let sddl: Vec<u16> = "D:P(A;OICI;FA;;;OW)(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)"
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let mut name = wide(path);
        // SAFETY: the converted descriptor owns the ACL until after SetNamedSecurityInfoW returns.
        unsafe {
            let mut descriptor = ptr::null_mut();
            if ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                1,
                &mut descriptor,
                ptr::null_mut(),
            ) == 0
            {
                return Err(Error::new("HOME_ACL_DESCRIPTOR_FAILED"));
            }
            let mut present = 0;
            let mut defaulted = 0;
            let mut acl = ptr::null_mut();
            let valid =
                GetSecurityDescriptorDacl(descriptor, &mut present, &mut acl, &mut defaulted);
            let status = if valid != 0 && present != 0 && !acl.is_null() {
                SetNamedSecurityInfoW(
                    name.as_mut_ptr(),
                    SE_FILE_OBJECT,
                    DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    acl,
                    ptr::null_mut(),
                )
            } else {
                1
            };
            LocalFree(descriptor);
            if status != 0 {
                return Err(Error::new("HOME_ACL_APPLY_FAILED"));
            }
        }
        Ok(())
    }
    pub fn validate_local_ntfs(path: &Path) -> Result<()> {
        let p = wide(path);
        let mut volume = [0u16; 32768];
        let mut fs_name = [0u16; 32];
        // SAFETY: fixed output buffers and NUL-terminated input; unused outputs are null.
        unsafe {
            if GetVolumePathNameW(p.as_ptr(), volume.as_mut_ptr(), volume.len() as u32) == 0 {
                return Err(Error::new("VOLUME_UNAVAILABLE"));
            }
            if GetDriveTypeW(volume.as_ptr()) != 3 {
                return Err(Error::new("FIXED_LOCAL_DISK_REQUIRED"));
            }
            if GetVolumeInformationW(
                volume.as_ptr(),
                ptr::null_mut(),
                0,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                fs_name.as_mut_ptr(),
                fs_name.len() as u32,
            ) == 0
                || text(&fs_name) != "NTFS"
            {
                return Err(Error::new("NTFS_REQUIRED_FOR_R0"));
            }
        }
        Ok(())
    }
    pub fn disk(path: &Path) -> DiskObservation {
        let mut result = DiskObservation {
            target: path.to_string_lossy().into_owned(),
            coverage: Coverage::Denied,
            available_bytes: None,
            total_bytes: None,
        };
        if validate_local_ntfs(path).is_err() {
            return result;
        }
        let p = wide(path);
        let mut available = 0u64;
        let mut total = 0u64;
        let mut free = 0u64;
        // SAFETY: valid NUL-terminated path and properly sized u64 outputs.
        if unsafe { GetDiskFreeSpaceExW(p.as_ptr(), &mut available, &mut total, &mut free) } != 0 {
            result.coverage = Coverage::Complete;
            result.available_bytes = Some(available);
            result.total_bytes = Some(total);
        }
        result
    }
    fn ft(t: FILETIME) -> u64 {
        (u64::from(t.dwHighDateTime) << 32) | u64::from(t.dwLowDateTime)
    }
    pub fn processes() -> ProcessSnapshot {
        let mut result = ProcessSnapshot {
            observed_at: now(),
            coverage: Coverage::Complete,
            enumerated_count: 0,
            selected: Vec::new(),
            notes: vec!["NAME_FILTERED_INVENTORY_NOT_SESSION_OWNERSHIP".into()],
        };
        // SAFETY: ToolHelp APIs receive their required initialized structure size and valid buffers.
        unsafe {
            let h = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if h == INVALID_HANDLE_VALUE {
                result.coverage = Coverage::Denied;
                return result;
            }
            let snapshot = Owned(h);
            let mut entry: PROCESSENTRY32W = zeroed();
            entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
            let mut ok = Process32FirstW(snapshot.0, &mut entry);
            while ok != 0 {
                result.enumerated_count += 1;
                if result.enumerated_count > 20_000 {
                    result.coverage = Coverage::Partial;
                    result.notes.push("PROCESS_ENUMERATION_CAP".into());
                    break;
                }
                let name = text(&entry.szExeFile).to_ascii_lowercase();
                const NAMES: &[&str] = &[
                    "codex.exe",
                    "claude.exe",
                    "cursor.exe",
                    "code.exe",
                    "chatgpt.exe",
                    "docker desktop.exe",
                    "com.docker.backend.exe",
                    "node.exe",
                    "node_repl.exe",
                    "python.exe",
                    "python3.exe",
                    "pwsh.exe",
                    "powershell.exe",
                    "windsurf.exe",
                    "grok.exe",
                    "gemini.exe",
                ];
                if NAMES.contains(&name.as_str()) {
                    if result.selected.len() >= 1000 {
                        result.coverage = Coverage::Partial;
                        result.notes.push("PROCESS_SELECTION_CAP".into());
                        break;
                    }
                    let mut p = ProcessObservation {
                        pid: entry.th32ProcessID,
                        parent_pid_observed: entry.th32ParentProcessID,
                        birth_filetime_100ns: None,
                        executable_name: name,
                        working_set_bytes: None,
                        private_commit_bytes: None,
                        coverage: Coverage::Denied,
                        ownership: "unknown".into(),
                        disposition: "protected".into(),
                    };
                    let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, p.pid);
                    if !h.is_null() {
                        let process = Owned(h);
                        let mut birth: FILETIME = zeroed();
                        let mut exit: FILETIME = zeroed();
                        let mut kernel: FILETIME = zeroed();
                        let mut user: FILETIME = zeroed();
                        if GetProcessTimes(process.0, &mut birth, &mut exit, &mut kernel, &mut user)
                            != 0
                        {
                            p.birth_filetime_100ns = Some(ft(birth));
                        }
                        let mut mem: PROCESS_MEMORY_COUNTERS_EX = zeroed();
                        mem.cb = size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
                        if GetProcessMemoryInfo(
                            process.0,
                            &mut mem as *mut _ as *mut PROCESS_MEMORY_COUNTERS,
                            mem.cb,
                        ) != 0
                        {
                            p.working_set_bytes = Some(mem.WorkingSetSize as u64);
                            p.private_commit_bytes = Some(mem.PrivateUsage as u64);
                        }
                        p.coverage = if p.birth_filetime_100ns.is_some()
                            && p.private_commit_bytes.is_some()
                        {
                            Coverage::Complete
                        } else {
                            Coverage::Partial
                        };
                    }
                    if !p.coverage.is_complete() {
                        result.coverage = Coverage::Partial;
                    }
                    result.selected.push(p);
                }
                ok = Process32NextW(snapshot.0, &mut entry);
            }
            if ok == 0 && GetLastError() != ERROR_NO_MORE_FILES {
                result.coverage = Coverage::Partial;
                result.notes.push("ENUMERATION_ENDED_WITH_ERROR".into());
            }
        }
        result
    }
}
#[cfg(windows)]
pub use native::*;

#[cfg(not(windows))]
pub fn processes() -> ProcessSnapshot {
    ProcessSnapshot {
        observed_at: crate::now(),
        coverage: Coverage::Unsupported,
        enumerated_count: 0,
        selected: vec![],
        notes: vec!["WINDOWS_COLLECTOR_NOT_EXECUTED_ON_THIS_HOST".into()],
    }
}
#[cfg(not(windows))]
pub fn disk(path: &std::path::Path) -> DiskObservation {
    DiskObservation {
        target: path.to_string_lossy().into_owned(),
        coverage: Coverage::Unsupported,
        available_bytes: None,
        total_bytes: None,
    }
}
