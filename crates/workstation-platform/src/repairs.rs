//! Narrow offline quarantine: no deletion, no global WSL shutdown, no application kill.
//! Candidate recipes are not certified. Explicit plan approval + release acknowledgement required.
use crate::{
    control_store::{epoch, sha},
    paths,
    storage::Store,
    Error, Result,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};
use workstation_core::{effects::*, integrations::Integration};
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SocketEntry {
    name: String,
    attributes: u32,
    reparse_tag: u32,
    bytes: u64,
    modified: u64,
}
fn observed_error(path: &Path, kind: RepairKind) -> Result<(String, i64)> {
    paths::local_existing(path)?;
    let m = fs::metadata(path).map_err(|_| Error::new("ERROR_FILE_UNREADABLE"))?;
    if !m.is_file() || m.len() > 1024 * 1024 {
        return Err(Error::new("ERROR_FILE_LIMIT"));
    }
    let at = m
        .modified()
        .map_err(|_| Error::new("ERROR_FILE_TIME"))?
        .duration_since(UNIX_EPOCH)
        .map_err(|_| Error::new("ERROR_FILE_TIME"))?
        .as_secs() as i64;
    let now = epoch();
    if at > now + 5 || now - at > 300 {
        return Err(Error::new("STALE_ERROR_FILE"));
    }
    let mut b = vec![];
    fs::File::open(path)
        .map_err(|_| Error::new("ERROR_FILE_UNREADABLE"))?
        .take(1024 * 1024 + 1)
        .read_to_end(&mut b)
        .map_err(|_| Error::new("ERROR_FILE_UNREADABLE"))?;
    if b.len() > 1024 * 1024 {
        return Err(Error::new("ERROR_FILE_LIMIT"));
    }
    let text = String::from_utf8_lossy(&b).to_lowercase();
    let matches = match kind {
        RepairKind::CodexAttachmentRegistry => {
            text.contains("pasted-text") && (text.contains("json") || text.contains("attach"))
        }
        _ => {
            text.contains(".sock")
                && (text.contains("cannot be accessed") || text.contains("error_cant_access_file"))
        }
    };
    if !matches {
        return Err(Error::new("CURRENT_FAILURE_FINGERPRINT_NOT_MATCHED"));
    }
    Ok((sha(&b), at))
}
fn stopped(kind: RepairKind) -> Result<()> {
    #[cfg(not(windows))]
    {
        let _ = kind;
        return Err(Error::new("REPAIR_WINDOWS_REQUIRED"));
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::{
            Foundation::{CloseHandle, GetLastError, ERROR_NO_MORE_FILES, INVALID_HANDLE_VALUE},
            System::Diagnostics::ToolHelp::*,
        };
        // Enumerate every process name, not just the regular health collector's selected subset.
        unsafe {
            let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snap == INVALID_HANDLE_VALUE {
                return Err(Error::new("APPLICATION_STOP_COVERAGE_INCOMPLETE"));
            }
            let mut row: PROCESSENTRY32W = std::mem::zeroed();
            row.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            let mut ok = Process32FirstW(snap, &mut row);
            let mut found = false;
            let mut count = 0;
            while ok != 0 {
                count += 1;
                if count > 50000 {
                    CloseHandle(snap);
                    return Err(Error::new("APPLICATION_STOP_COVERAGE_INCOMPLETE"));
                }
                let n = row
                    .szExeFile
                    .iter()
                    .position(|c| *c == 0)
                    .unwrap_or(row.szExeFile.len());
                let name = String::from_utf16_lossy(&row.szExeFile[..n]).to_lowercase();
                found |= match kind {
                    RepairKind::CodexAttachmentRegistry => {
                        name.contains("codex") || name == "chatgpt.exe" || name == "node_repl.exe"
                    }
                    _ => name.contains("docker"),
                };
                ok = Process32NextW(snap, &mut row);
            }
            let last = GetLastError();
            CloseHandle(snap);
            if last != ERROR_NO_MORE_FILES {
                return Err(Error::new("APPLICATION_STOP_COVERAGE_INCOMPLETE"));
            }
            if found {
                return Err(Error::new("STOP_APPLICATION_AND_RELATED_SERVICE_MANUALLY"));
            }
        }
        Ok(())
    }
}
fn target(root: &Path, kind: RepairKind) -> Result<PathBuf> {
    paths::directory(root)?;
    match kind {
        RepairKind::CodexAttachmentRegistry => {
            let expected = std::env::var_os("CODEX_HOME")
                .map(PathBuf::from)
                .or_else(|| {
                    std::env::var_os("USERPROFILE").map(|p| PathBuf::from(p).join(".codex"))
                })
                .ok_or(Error::new("CODEX_HOME_UNAVAILABLE"))?;
            if root != expected {
                return Err(Error::new("CODEX_HOME_NOT_EFFECTIVE_ENVIRONMENT"));
            }
            Ok(root
                .join("attachments")
                .join("pasted-text-attachments.json"))
        }
        RepairKind::DockerRuntime => {
            let local =
                std::env::var_os("LOCALAPPDATA").ok_or(Error::new("LOCALAPPDATA_MISSING"))?;
            let expected = PathBuf::from(local).join("Docker");
            if root != expected {
                return Err(Error::new("DOCKER_RUNTIME_ROOT_NOT_DEFAULT_LOCAL_PROFILE"));
            }
            Ok(root.join("run"))
        }
        RepairKind::DockerSecretsRuntime => {
            let local =
                std::env::var_os("LOCALAPPDATA").ok_or(Error::new("LOCALAPPDATA_MISSING"))?;
            if root != Path::new(&local) {
                return Err(Error::new("DOCKER_SECRETS_ROOT_NOT_LOCAL_PROFILE"));
            }
            Ok(root.join("docker-secrets-engine"))
        }
    }
}
#[cfg(windows)]
fn socket_entries(path: &Path) -> Result<Vec<SocketEntry>> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::{
        Foundation::{GetLastError, ERROR_NO_MORE_FILES, INVALID_HANDLE_VALUE},
        Storage::FileSystem::*,
    };
    let mut pattern: Vec<u16> = path.join("*").as_os_str().encode_wide().collect();
    pattern.push(0);
    struct Find(windows_sys::Win32::Foundation::HANDLE);
    impl Drop for Find {
        fn drop(&mut self) {
            unsafe {
                FindClose(self.0);
            }
        }
    }
    let mut data: WIN32_FIND_DATAW = unsafe { std::mem::zeroed() };
    let h = unsafe { FindFirstFileW(pattern.as_ptr(), &mut data) };
    if h == INVALID_HANDLE_VALUE {
        return Err(Error::new("SOCKET_CONTAINER_ENUMERATION_FAILED"));
    }
    let h = Find(h);
    let mut rows = vec![];
    loop {
        let len = data
            .cFileName
            .iter()
            .position(|x| *x == 0)
            .unwrap_or(data.cFileName.len());
        let name = String::from_utf16(&data.cFileName[..len])
            .map_err(|_| Error::new("SOCKET_NAME_ENCODING"))?;
        if name != "." && name != ".." {
            let size = (u64::from(data.nFileSizeHigh) << 32) | u64::from(data.nFileSizeLow);
            let attributes = data.dwFileAttributes;
            let tag = data.dwReserved0;
            if rows.len() >= 64
                || size != 0
                || attributes & FILE_ATTRIBUTE_DIRECTORY != 0
                || (!name.ends_with(".sock") && !name.ends_with(".sock.stale"))
                || name.contains(['/', '\\'])
            {
                return Err(Error::new("RUNTIME_CONTAINS_NONSOCKET_DATA"));
            }
            if attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 && tag != 0x80000023 {
                return Err(Error::new("RUNTIME_REPARSE_TAG_NOT_AF_UNIX"));
            }
            rows.push(SocketEntry {
                name,
                attributes,
                reparse_tag: tag,
                bytes: size,
                modified: (u64::from(data.ftLastWriteTime.dwHighDateTime) << 32)
                    | u64::from(data.ftLastWriteTime.dwLowDateTime),
            });
        }
        if unsafe { FindNextFileW(h.0, &mut data) } == 0 {
            if unsafe { GetLastError() } != ERROR_NO_MORE_FILES {
                return Err(Error::new("SOCKET_ENUMERATION_INCOMPLETE"));
            }
            break;
        }
    }
    if rows.is_empty() {
        return Err(Error::new("NO_RUNTIME_SOCKET_TO_QUARANTINE"));
    }
    rows.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(rows)
}
#[cfg(not(windows))]
fn socket_entries(_: &Path) -> Result<Vec<SocketEntry>> {
    Err(Error::new("REPAIR_WINDOWS_REQUIRED"))
}
fn target_digest(path: &Path, kind: RepairKind) -> Result<String> {
    if kind == RepairKind::CodexAttachmentRegistry {
        paths::local_existing(path)?;
        let m = fs::metadata(path).map_err(|_| Error::new("REGISTRY_UNAVAILABLE"))?;
        if !m.is_file() || m.len() > 1024 * 1024 {
            return Err(Error::new("REGISTRY_SIZE_NOT_ELIGIBLE"));
        }
        let mut b = vec![];
        fs::File::open(path)
            .map_err(|_| Error::new("REGISTRY_READ"))?
            .take(1024 * 1024 + 1)
            .read_to_end(&mut b)
            .map_err(|_| Error::new("REGISTRY_READ"))?;
        if b.len() > 1024 * 1024 {
            return Err(Error::new("REGISTRY_SIZE_NOT_ELIGIBLE"));
        }
        if serde_json::from_slice::<Value>(&b).is_ok() {
            return Err(Error::new("VALID_REGISTRY_NOT_AUTOMATICALLY_DISPOSABLE"));
        }
        Ok(sha(&b))
    } else {
        paths::directory(path)?;
        Ok(sha(&serde_json::to_vec(&socket_entries(path)?)
            .map_err(|_| Error::new("RUNTIME_DIGEST_ENCODING"))?))
    }
}
pub fn inspect(
    root: &Path,
    kind: RepairKind,
    error: &Path,
    version: &str,
) -> Result<QuarantineTarget> {
    stopped(kind)?;
    let t = target(root, kind)?;
    let (digest, at) = observed_error(error, kind)?;
    let content = target_digest(&t, kind)?;
    let identity = crate::external::path_identity(&t)?;
    Ok(QuarantineTarget {
        kind,
        root: root.to_str().ok_or(Error::new("PATH_ENCODING"))?.into(),
        target: t.to_str().ok_or(Error::new("PATH_ENCODING"))?.into(),
        identity,
        content_digest: content,
        source_version: version.into(),
        source_error_path: error.to_str().ok_or(Error::new("PATH_ENCODING"))?.into(),
        source_error_sha256: digest,
        error_observed_at: at,
        original_exists: true,
    })
}
pub fn quarantine(store: &mut Store, run: &str, t: &QuarantineTarget) -> Result<Value> {
    let fresh = inspect(
        Path::new(&t.root),
        t.kind,
        Path::new(&t.source_error_path),
        &t.source_version,
    )?;
    if serde_json::to_vec(&fresh).ok() != serde_json::to_vec(t).ok() {
        return Err(Error::new("REPAIR_TARGET_CHANGED"));
    }
    let original = Path::new(&t.target);
    let name = original
        .file_name()
        .ok_or(Error::new("TARGET_NAME"))?
        .to_string_lossy();
    let backup = original.with_file_name(format!("{name}.workstation-quarantine-{run}"));
    if paths::entry_exists(&backup)? {
        return Err(Error::new("QUARANTINE_ALREADY_EXISTS"));
    }
    store.effect_step(run,"quarantine_exact_runtime_entry","intent",&json!({"original":original,"quarantine":backup,"identity":t.identity,"digest":t.content_digest,"payloads":"not_touched"}))?;
    // One same-volume rename; never cross-volume copy/delete a reparse point.
    fs::rename(original, &backup).map_err(|_| Error::new("QUARANTINE_RENAME_FAILED"))?;
    if paths::entry_exists(original)? || crate::external::path_identity(&backup)? != t.identity {
        return Err(Error::new("QUARANTINE_POSTCONDITION_UNCERTAIN"));
    }
    let out = json!({"original":original,"quarantine":backup,"quarantine_identity":t.identity,"source_kind":t.kind,"content_digest":t.content_digest,"restorable":true,"registry_or_runtime_only":true,"root_cause_fixed":false,"application_started":false,"postcondition":"entry_isolated_not_application_recovery_certified"});
    store.effect_step(run, "quarantine_exact_runtime_entry", "completed", &out)?;
    Ok(out)
}
pub fn restore(store: &mut Store, run: &str, prior: &Value) -> Result<Value> {
    let o = prior
        .get("original")
        .and_then(Value::as_str)
        .ok_or(Error::new("RESTORE_RECEIPT_INVALID"))?;
    let q = prior
        .get("quarantine")
        .and_then(Value::as_str)
        .ok_or(Error::new("RESTORE_RECEIPT_INVALID"))?;
    let kind: RepairKind = serde_json::from_value(
        prior
            .get("source_kind")
            .cloned()
            .ok_or(Error::new("RESTORE_KIND"))?,
    )
    .map_err(|_| Error::new("RESTORE_KIND"))?;
    stopped(kind)?;
    let original = Path::new(o);
    let backup = Path::new(q);
    paths::local_existing(backup)?;
    if original.parent() != backup.parent()
        || !backup
            .file_name()
            .and_then(|x| x.to_str())
            .is_some_and(|n| n.contains(".workstation-quarantine-"))
    {
        return Err(Error::new("RESTORE_PATH_BOUNDARY"));
    }
    paths::validate_lexical(original)?;
    if paths::entry_exists(original)? {
        return Err(Error::new("RESTORE_WOULD_OVERWRITE_NEW_STATE"));
    }
    if crate::external::path_identity(backup)?
        != prior
            .get("quarantine_identity")
            .and_then(Value::as_str)
            .ok_or(Error::new("RESTORE_IDENTITY"))?
    {
        return Err(Error::new("RESTORE_BACKUP_CHANGED"));
    }
    let digest = target_digest(backup, kind)?;
    if prior.get("content_digest").and_then(Value::as_str) != Some(digest.as_str()) {
        return Err(Error::new("RESTORE_CONTENT_CHANGED"));
    }
    store.effect_step(
        run,
        "restore_quarantine",
        "intent",
        &json!({"original":original,"quarantine":backup}),
    )?;
    fs::rename(backup, original).map_err(|_| Error::new("RESTORE_RENAME_FAILED"))?;
    if paths::entry_exists(backup)?
        || crate::external::path_identity(original)?
            != prior
                .get("quarantine_identity")
                .and_then(Value::as_str)
                .ok_or(Error::new("RESTORE_IDENTITY"))?
        || target_digest(original, kind)? != digest
    {
        return Err(Error::new("RESTORE_POSTCONDITION_UNCERTAIN"));
    }
    let result = json!({"restored":true,"application_repaired":false,"overwritten":false});
    store.effect_step(run, "restore_quarantine", "completed", &result)?;
    Ok(result)
}

/// Derive only the installed Docker Desktop GUI next to its pinned bundled CLI.
pub fn desktop_identity(i: &Integration) -> Result<(String, String)> {
    if i.adapter != workstation_core::integrations::Adapter::Docker {
        return Err(Error::new("DOCKER_ADAPTER_REQUIRED"));
    }
    let cli = Path::new(&i.executable);
    if cli.file_name().and_then(|x| x.to_str()) != Some("docker.exe")
        || cli
            .parent()
            .and_then(Path::file_name)
            .and_then(|x| x.to_str())
            != Some("bin")
        || cli
            .parent()
            .and_then(Path::parent)
            .and_then(Path::file_name)
            .and_then(|x| x.to_str())
            != Some("resources")
    {
        return Err(Error::new("BUNDLED_DOCKER_CLI_REQUIRED"));
    }
    let desktop = cli
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .ok_or(Error::new("DESKTOP_ROOT"))?
        .join("Docker Desktop.exe");
    let hash = crate::external::hash_file(&desktop)?;
    Ok((
        desktop.to_str().ok_or(Error::new("PATH_ENCODING"))?.into(),
        hash,
    ))
}
/// A persistent GUI launch is intentionally NOT treated as an ephemeral probe.
/// No existing application is killed and no broad job is terminated on return.
pub fn start_docker(
    store: &mut Store,
    run: &str,
    i: &Integration,
    desktop: &str,
    digest: &str,
) -> Result<Value> {
    #[cfg(not(windows))]
    {
        let _ = (store, run, i, desktop, digest);
        Err(Error::new("DOCKER_WINDOWS_REQUIRED"))
    }
    #[cfg(windows)]
    {
        use std::{
            process::{Command, Stdio},
            time::{Duration, Instant},
        };
        let (actual, hash) = desktop_identity(i)?;
        if actual != desktop || hash != digest {
            return Err(Error::new("DESKTOP_BINARY_CHANGED"));
        }
        // Do not create a second launch if the local engine is already healthy.
        if let Ok(v) = crate::adapters::docker_local(store, i) {
            return Ok(json!({"already_healthy":true,"engine":v,"new_process":false}));
        }
        stopped(RepairKind::DockerRuntime)?;
        store.effect_step(run,"start_exact_docker_desktop","intent",&json!({"executable":desktop,"sha256":digest,"persistent_application":true,"restarts":0}))?;
        let env = crate::tasks::environment(store, i)?;
        let mut child = Command::new(desktop)
            .current_dir(
                Path::new(desktop)
                    .parent()
                    .ok_or(Error::new("DESKTOP_PARENT"))?,
            )
            .env_clear()
            .envs(env)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| Error::new("DESKTOP_START_FAILED"))?;
        let pid = child.id();
        store.effect_step(run,"start_exact_docker_desktop","completed",&json!({"pid":pid,"pid_alone_never_stop_authority":true,"application_intentionally_persistent":true}))?;
        let deadline = Instant::now() + Duration::from_secs(90);
        loop {
            if let Ok(v) = crate::adapters::docker_local(store, i) {
                return Ok(
                    json!({"launched_pid":pid,"engine":v,"root_cause_fixed":false,"certified":false}),
                );
            }
            // Launcher exit does not prove Desktop exited. Keep polling only the explicit local API.
            let _ = child.try_wait();
            if Instant::now() >= deadline {
                return Err(Error::new("DOCKER_API_NOT_READY_INSPECT_LAUNCH_RECEIPT"));
            }
            std::thread::sleep(Duration::from_secs(2));
        }
    }
}
