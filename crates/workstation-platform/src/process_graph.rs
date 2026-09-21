//! Win32 process metadata collected only inside a bounded owned collector.
//! Paths/digests identify on-disk executables, not their loaded bytes or semantic sessions.
use crate::{Error, Result};
use workstation_core::{host_graph::*, integrations::Integration, ownership::*, Coverage};
#[cfg(windows)]
pub fn uptime_ms() -> Result<u64> {
    Ok(unsafe { windows_sys::Win32::System::SystemInformation::GetTickCount64() })
}
#[cfg(not(windows))]
pub fn uptime_ms() -> Result<u64> {
    Err(Error::new("HOST_PROCESS_GRAPH_WINDOWS_REQUIRED"))
}
#[cfg(windows)]
pub fn child_identity(child: &std::process::Child) -> Result<(u32, u64)> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::{
        Foundation::{FILETIME, HANDLE},
        System::Threading::GetProcessTimes,
    };
    let mut c: FILETIME = unsafe { std::mem::zeroed() };
    let (mut e, mut k, mut u) = (c, c, c);
    if unsafe {
        GetProcessTimes(
            child.as_raw_handle() as HANDLE,
            &mut c,
            &mut e,
            &mut k,
            &mut u,
        )
    } == 0
    {
        return Err(Error::new("OWNED_PROCESS_BIRTH_UNAVAILABLE"));
    }
    Ok((
        child.id(),
        u64::from(c.dwLowDateTime) | (u64::from(c.dwHighDateTime) << 32),
    ))
}
#[cfg(not(windows))]
pub fn child_identity(_child: &std::process::Child) -> Result<(u32, u64)> {
    Err(Error::new("OWNED_PROCESS_BIRTH_WINDOWS_REQUIRED"))
}
#[cfg(windows)]
pub fn collect(host: &str, boot: &str, profiles: &[Integration]) -> Result<HostGraph> {
    use std::{
        collections::BTreeMap,
        mem::{size_of, zeroed},
        path::Path,
    };
    use windows_sys::Win32::{
        Foundation::{
            CloseHandle, GetLastError, ERROR_NO_MORE_FILES, FILETIME, HANDLE, INVALID_HANDLE_VALUE,
        },
        System::{Diagnostics::ToolHelp::*, ProcessStatus::*, Threading::*},
    };
    struct H(HANDLE);
    impl Drop for H {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
    workstation_core::control::id(host).map_err(Error::new)?;
    workstation_core::control::id(boot).map_err(Error::new)?;
    if profiles.len() > 128 {
        return Err(Error::new("PROFILE_GRAPH_LIMIT"));
    }
    let mut result = HostGraph {
        observed_at: crate::control_store::epoch(),
        uptime_ms: Some(uptime_ms()?),
        coverage: Coverage::Complete,
        processes: vec![],
        executables: vec![],
        denied: 0,
        notices: vec![
            "No command lines, environment blocks or process memory contents were read.".into(),
            "Disk digest is not loaded-image proof. Session edges require independent provenance."
                .into(),
        ],
    };
    let raw = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if raw == INVALID_HANDLE_VALUE {
        return Err(Error::new("PROCESS_SNAPSHOT_DENIED"));
    }
    let snapshot = H(raw);
    let mut entry: PROCESSENTRY32W = unsafe { zeroed() };
    entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
    let mut ok = unsafe { Process32FirstW(snapshot.0, &mut entry) };
    let mut parents = BTreeMap::new();
    let mut hashes: BTreeMap<String, Option<String>> = BTreeMap::new();
    let ft = |v: FILETIME| u64::from(v.dwLowDateTime) | (u64::from(v.dwHighDateTime) << 32);
    while ok != 0 {
        if result.processes.len() >= 4096 {
            result.coverage = Coverage::Partial;
            result.notices.push("process_limit".into());
            break;
        }
        let pid = entry.th32ProcessID;
        if pid > 0 {
            let raw = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
            if raw.is_null() {
                result.denied += 1;
                result.coverage = Coverage::Partial;
            } else {
                let h = H(raw);
                let mut c: FILETIME = unsafe { zeroed() };
                let (mut e, mut k, mut u) = (c, c, c);
                if unsafe { GetProcessTimes(h.0, &mut c, &mut e, &mut k, &mut u) } != 0 {
                    let key = ProcessKey {
                        host: host.into(),
                        boot: boot.into(),
                        pid,
                        created: ft(c),
                    };
                    let mut name = vec![0u16; 32768];
                    let mut n = name.len() as u32;
                    let path =
                        if unsafe { QueryFullProcessImageNameW(h.0, 0, name.as_mut_ptr(), &mut n) }
                            != 0
                        {
                            String::from_utf16(&name[..n as usize]).ok()
                        } else {
                            None
                        };
                    let mut matched = vec![];
                    let mut digest = None;
                    let path_readable = path.is_some();
                    if let Some(path) = path {
                        let candidates: Vec<_> = profiles
                            .iter()
                            .filter(|p| {
                                p.executable
                                    .replace('\\', "/")
                                    .eq_ignore_ascii_case(&path.replace('\\', "/"))
                            })
                            .collect();
                        if !candidates.is_empty() {
                            digest = hashes
                                .entry(path.clone())
                                .or_insert_with(|| {
                                    crate::external::hash_file(Path::new(&path)).ok()
                                })
                                .clone();
                            for p in candidates {
                                if digest.as_deref() == Some(p.executable_sha256.as_str()) {
                                    matched.push(p.id.clone());
                                }
                            }
                        }
                        result.executables.push(ExecutableObservation {
                            process: key.clone(),
                            path,
                            disk_digest: digest.clone(),
                            matching_integrations: matched,
                            loaded_image_identity_verified: false,
                        });
                    }
                    let mut private = None;
                    let mut io_bytes = None;
                    let query =
                        unsafe { OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, 0, pid) };
                    if !query.is_null() {
                        let q = H(query);
                        let mut memory: PROCESS_MEMORY_COUNTERS_EX = unsafe { zeroed() };
                        memory.cb = size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
                        let (mut qc, mut qe, mut qk, mut qu) = (c, c, c, c);
                        let same_birth =
                            unsafe { GetProcessTimes(q.0, &mut qc, &mut qe, &mut qk, &mut qu) }
                                != 0
                                && ft(qc) == key.created;
                        if same_birth
                            && unsafe {
                                K32GetProcessMemoryInfo(
                                    q.0,
                                    (&mut memory as *mut PROCESS_MEMORY_COUNTERS_EX).cast(),
                                    size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32,
                                )
                            } != 0
                        {
                            private = Some(memory.PrivateUsage as u64);
                        }
                        let mut io: IO_COUNTERS = unsafe { zeroed() };
                        if same_birth && unsafe { GetProcessIoCounters(q.0, &mut io) } != 0 {
                            io_bytes = io
                                .ReadTransferCount
                                .checked_add(io.WriteTransferCount)
                                .and_then(|v| v.checked_add(io.OtherTransferCount));
                        }
                    }
                    parents.insert(pid, entry.th32ParentProcessID);
                    result.processes.push(ProcessFact {
                        key,
                        parent: None,
                        executable_digest: digest,
                        private_bytes: private,
                        cpu_ticks: Some(ft(k).saturating_add(ft(u))),
                        io_bytes,
                        coverage: if path_readable && private.is_some() && io_bytes.is_some() {
                            Coverage::Complete
                        } else {
                            Coverage::Partial
                        },
                    });
                } else {
                    result.denied += 1;
                    result.coverage = Coverage::Partial;
                }
            }
        }
        ok = unsafe { Process32NextW(snapshot.0, &mut entry) };
    }
    if ok == 0 && unsafe { GetLastError() } != ERROR_NO_MORE_FILES {
        result.coverage = Coverage::Partial;
    }
    let index: BTreeMap<_, _> = result
        .processes
        .iter()
        .map(|x| (x.key.pid, x.key.clone()))
        .collect();
    for p in &mut result.processes {
        if let Some(parent) = parents.get(&p.key.pid).and_then(|id| index.get(id)) {
            if parent.created <= p.key.created && parent != &p.key {
                p.parent = Some(parent.clone());
            } else {
                p.coverage = Coverage::Partial;
            }
        } else if parents.get(&p.key.pid).is_some_and(|id| *id != 0) {
            p.coverage = Coverage::Partial;
        }
    }
    if result
        .processes
        .iter()
        .any(|p| p.coverage != Coverage::Complete)
    {
        result.coverage = Coverage::Partial;
    }
    Ok(result)
}
#[cfg(not(windows))]
pub fn collect(_host: &str, _boot: &str, _profiles: &[Integration]) -> Result<HostGraph> {
    Ok(HostGraph {
        observed_at: crate::control_store::epoch(),
        uptime_ms: None,
        coverage: Coverage::Unsupported,
        processes: vec![],
        executables: vec![],
        denied: 0,
        notices: vec!["Native Windows collector not executed on this platform.".into()],
    })
}
