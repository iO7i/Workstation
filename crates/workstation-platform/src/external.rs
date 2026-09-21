//! Internal bounded subprocess transport for reviewed typed integrations/tasks.
//! No CLI/MCP accepts an arbitrary executable through this module directly.
use crate::{paths, runner::Guard, Error, Result};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    ffi::OsString,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender, TrySendError},
        Arc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

pub struct Launch {
    pub executable: PathBuf,
    pub expected_sha256: String,
    pub cwd: PathBuf,
    pub args: Vec<OsString>,
    pub env: BTreeMap<OsString, OsString>,
    pub timeout: Duration,
    pub output_limit: usize,
}
const PIN_FILE_LIMIT_BYTES: u64 = 1024 * 1024 * 1024;
pub fn hash_file(path: &Path) -> Result<String> {
    use sha2::{Digest, Sha256};
    paths::local_existing(path)?;
    let m = std::fs::symlink_metadata(path).map_err(|_| Error::new("FILE_UNAVAILABLE"))?;
    if !m.is_file() || m.len() > PIN_FILE_LIMIT_BYTES {
        return Err(Error::new("PIN_FILE_LIMIT"));
    }
    let mut f = std::fs::File::open(path).map_err(|_| Error::new("PIN_OPEN_FAILED"))?;
    let mut h = Sha256::new();
    let mut buf = [0; 65536];
    let mut count = 0;
    loop {
        let n = f
            .read(&mut buf)
            .map_err(|_| Error::new("PIN_READ_FAILED"))?;
        if n == 0 {
            break;
        }
        count += n;
        if count > PIN_FILE_LIMIT_BYTES as usize {
            return Err(Error::new("PIN_FILE_LIMIT"));
        }
        h.update(&buf[..n]);
    }
    Ok(format!("{:x}", h.finalize()))
}
fn system_environment() -> BTreeMap<OsString, OsString> {
    let mut e = BTreeMap::new();
    for key in ["SystemRoot", "WINDIR", "ComSpec", "SYSTEMDRIVE"] {
        if let Some(v) = std::env::var_os(key) {
            e.insert(key.into(), v);
        }
    }
    e
}
pub fn scoped_environment(
    home: &Path,
    inherit: &[String],
    configured: &BTreeMap<String, String>,
) -> Result<BTreeMap<OsString, OsString>> {
    let mut env = system_environment();
    let temp = home.join("scratch");
    if !temp.exists() {
        std::fs::create_dir(&temp).map_err(|_| Error::new("SCRATCH_CREATE_FAILED"))?;
    }
    paths::directory(&temp)?;
    for k in ["TEMP", "TMP"] {
        env.insert(k.into(), temp.as_os_str().into());
    }
    for key in inherit {
        if let Some(v) = std::env::var_os(key) {
            env.insert(key.into(), v);
        }
    }
    for (k, v) in configured {
        paths::directory(Path::new(v))?;
        env.insert(k.into(), v.into());
    }
    Ok(env)
}
enum Chunk {
    Out(Vec<u8>),
    Stderr(usize),
    OutEof,
    StderrEof,
    Fault,
}
fn output_reader(
    mut pipe: impl Read + Send + 'static,
    tx: SyncSender<Chunk>,
    overflow: Arc<AtomicBool>,
    stderr: bool,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            let msg = match pipe.read(&mut buf) {
                Ok(0) => {
                    if stderr {
                        Chunk::StderrEof
                    } else {
                        Chunk::OutEof
                    }
                }
                Ok(n) => {
                    if stderr {
                        Chunk::Stderr(n)
                    } else {
                        Chunk::Out(buf[..n].to_vec())
                    }
                }
                Err(_) => Chunk::Fault,
            };
            let final_msg = matches!(&msg, Chunk::OutEof | Chunk::StderrEof | Chunk::Fault);
            match tx.try_send(msg) {
                Ok(()) => {}
                Err(TrySendError::Full(_)) => {
                    overflow.store(true, Ordering::SeqCst);
                    break;
                }
                Err(TrySendError::Disconnected(_)) => break,
            }
            if final_msg {
                break;
            }
        }
    })
}
/// Running owns only its child/job. Drop never kills unrelated names, PIDs or WSL.
pub struct Running {
    child: Child,
    guard: Option<Guard>,
    rx: Receiver<Chunk>,
    input: Option<SyncSender<Vec<u8>>>,
    threads: Vec<JoinHandle<()>>,
    overflow: Arc<AtomicBool>,
    start: Instant,
    timeout: Duration,
    output_limit: usize,
    received: usize,
    stderr_bytes: usize,
    pending: Vec<u8>,
    out_eof: bool,
}
impl Running {
    pub fn identity(&self) -> Result<(u32, u64)> {
        crate::process_graph::child_identity(&self.child)
    }
    pub fn spawn(spec: Launch) -> Result<Self> {
        paths::directory(&spec.cwd)?;
        paths::local_existing(&spec.executable)?;
        if hash_file(&spec.executable)? != spec.expected_sha256 {
            return Err(Error::new("EXECUTABLE_CHANGED"));
        }
        if spec.timeout.is_zero()
            || spec.timeout > Duration::from_secs(3600)
            || spec.output_limit > 4 * 1024 * 1024
        {
            return Err(Error::new("TRANSPORT_BUDGET_INVALID"));
        }
        let mut cmd = Command::new(&spec.executable);
        cmd.args(&spec.args)
            .current_dir(&spec.cwd)
            .env_clear()
            .envs(&spec.env)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            cmd.process_group(0);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000004);
        } // NO_WINDOW | SUSPENDED
        let mut child = cmd
            .spawn()
            .map_err(|_| Error::new("EXTERNAL_SPAWN_FAILED"))?;
        let guard = match Guard::attach(&child) {
            Ok(g) => g,
            Err(e) => {
                let _ = child.kill();
                return Err(e);
            }
        };
        if let Err(e) = crate::durable_runtime::child_started(&child) {
            guard.terminate();
            let _ = child.kill();
            return Err(e);
        }
        #[cfg(windows)]
        if let Err(e) = resume_suspended(child.id()) {
            guard.terminate();
            let _ = child.kill();
            return Err(e);
        }
        let stdout = child
            .stdout
            .take()
            .ok_or(Error::new("EXTERNAL_PIPE_FAILED"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or(Error::new("EXTERNAL_PIPE_FAILED"))?;
        let mut stdin = child
            .stdin
            .take()
            .ok_or(Error::new("EXTERNAL_PIPE_FAILED"))?;
        let (tx, rx) = mpsc::sync_channel(128);
        let (itx, irx) = mpsc::sync_channel::<Vec<u8>>(4);
        let overflow = Arc::new(AtomicBool::new(false));
        let mut threads = vec![
            output_reader(stdout, tx.clone(), overflow.clone(), false),
            output_reader(stderr, tx, overflow.clone(), true),
        ];
        let writer_overflow = overflow.clone();
        threads.push(thread::spawn(move || {
            while let Ok(mut b) = irx.recv() {
                let failed = stdin.write_all(&b).and_then(|_| stdin.flush()).is_err();
                for v in &mut b {
                    unsafe {
                        std::ptr::write_volatile(v, 0);
                    }
                }
                if failed {
                    writer_overflow.store(true, Ordering::SeqCst);
                    break;
                }
            }
        }));
        Ok(Self {
            child,
            guard: Some(guard),
            rx,
            input: Some(itx),
            threads,
            overflow,
            start: Instant::now(),
            timeout: spec.timeout,
            output_limit: spec.output_limit,
            received: 0,
            stderr_bytes: 0,
            pending: vec![],
            out_eof: false,
        })
    }
    pub fn send(&self, bytes: Vec<u8>) -> Result<()> {
        if bytes.len() > 256 * 1024 + 4096 {
            return Err(Error::new("TRANSPORT_INPUT_LIMIT"));
        }
        self.input
            .as_ref()
            .ok_or(Error::new("TRANSPORT_INPUT_CLOSED"))?
            .try_send(bytes)
            .map_err(|_| Error::new("TRANSPORT_INPUT_BUSY_OR_CLOSED"))
    }
    pub fn send_json(&self, v: &Value) -> Result<()> {
        let mut b = serde_json::to_vec(v).map_err(|_| Error::new("RPC_ENCODING"))?;
        b.push(b'\n');
        self.send(b)
    }

    pub fn send_framed(&self, v: &Value) -> Result<()> {
        self.send(workstation_core::wire::encode(v).map_err(Error::new)?)
    }
    pub fn next_framed(&mut self) -> Result<Value> {
        loop {
            if let Some((n, v)) =
                workstation_core::wire::decode(&self.pending).map_err(Error::new)?
            {
                self.pending.drain(..n);
                return Ok(v);
            }
            if self.out_eof {
                return Err(Error::new("RPC_EOF_BEFORE_RESPONSE"));
            }
            self.pump()?;
        }
    }
    pub fn close_input(&mut self) {
        self.input.take();
    }
    fn pump(&mut self) -> Result<()> {
        if let Some(cancel) = crate::durable_runtime::tick()? {
            self.send_json(&cancel)?;
        }
        if !crate::durable_runtime::active() && self.start.elapsed() > self.timeout {
            return Err(Error::new("EXTERNAL_TIMEOUT"));
        }
        if self.overflow.load(Ordering::SeqCst) {
            return Err(Error::new("EXTERNAL_PIPE_BACKPRESSURE"));
        }
        match self.rx.recv_timeout(Duration::from_millis(25)) {
            Ok(Chunk::Out(v)) => {
                self.received += v.len();
                if self.received > self.output_limit {
                    return Err(Error::new("EXTERNAL_OUTPUT_LIMIT"));
                }
                self.pending.extend_from_slice(&v);
            }
            Ok(Chunk::Stderr(n)) => {
                self.stderr_bytes += n;
                if self.stderr_bytes > 256 * 1024 {
                    return Err(Error::new("EXTERNAL_STDERR_LIMIT"));
                }
            }
            Ok(Chunk::OutEof) => self.out_eof = true,
            Ok(Chunk::Fault) => return Err(Error::new("EXTERNAL_PIPE_READ_FAILED")),
            Ok(Chunk::StderrEof) | Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => self.out_eof = true,
        }
        Ok(())
    }
    pub fn next_json(&mut self) -> Result<Value> {
        loop {
            if let Some(n) = self.pending.iter().position(|b| *b == b'\n') {
                if n > 256 * 1024 {
                    return Err(Error::new("RPC_FRAME_LIMIT"));
                }
                let frame: Vec<u8> = self.pending.drain(..=n).collect();
                if frame.iter().all(u8::is_ascii_whitespace) {
                    continue;
                }
                return serde_json::from_slice(&frame).map_err(|_| Error::new("RPC_FRAME_INVALID"));
            }
            if self.pending.len() > 256 * 1024 {
                return Err(Error::new("RPC_FRAME_LIMIT"));
            }
            if self.out_eof {
                return Err(Error::new("RPC_EOF_BEFORE_RESPONSE"));
            }
            self.pump()?;
        }
    }
    pub fn finish(mut self) -> Result<Captured> {
        self.close_input();
        loop {
            self.pump()?;
            if self.out_eof {
                if let Some(s) = self
                    .child
                    .try_wait()
                    .map_err(|_| Error::new("EXTERNAL_WAIT_FAILED"))?
                {
                    let bytes = std::mem::take(&mut self.pending);
                    return Ok(Captured {
                        bytes: crate::vault::SecretBytes(bytes),
                        exit_code: s.code().unwrap_or(-1),
                        stderr_bytes: self.stderr_bytes,
                    });
                }
            }
        }
    }
}
impl Drop for Running {
    fn drop(&mut self) {
        self.input.take();
        if let Some(g) = self.guard.take() {
            g.terminate();
            drop(g);
        }
        let _ = self.child.kill();
        let until = Instant::now() + Duration::from_millis(750);
        while Instant::now() < until {
            if self.child.try_wait().ok().flatten().is_some()
                && self.threads.iter().all(JoinHandle::is_finished)
            {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        for t in self.threads.drain(..) {
            if t.is_finished() {
                let _ = t.join();
            }
        }
        for b in &mut self.pending {
            unsafe {
                std::ptr::write_volatile(b, 0);
            }
        }
    }
}
pub struct Captured {
    pub(crate) bytes: crate::vault::SecretBytes,
    pub exit_code: i32,
    pub stderr_bytes: usize,
}
pub fn one_shot(spec: Launch, input: Option<Vec<u8>>) -> Result<Captured> {
    let mut r = Running::spawn(spec)?;
    if let Some(b) = input {
        r.send(b)?;
    }
    r.close_input();
    r.finish()
}
#[cfg(windows)]
fn resume_suspended(pid: u32) -> Result<()> {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, INVALID_HANDLE_VALUE},
        System::{
            Diagnostics::ToolHelp::*,
            Threading::{OpenThread, ResumeThread, THREAD_SUSPEND_RESUME},
        },
    };
    // A newly created suspended process must have one primary thread. Ambiguity fails closed.
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return Err(Error::new("SUSPENDED_THREAD_SNAPSHOT"));
        }
        let mut entry: THREADENTRY32 = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<THREADENTRY32>() as u32;
        let mut found = vec![];
        let mut ok = Thread32First(snapshot, &mut entry);
        while ok != 0 {
            if entry.th32OwnerProcessID == pid {
                found.push(entry.th32ThreadID);
            }
            ok = Thread32Next(snapshot, &mut entry);
        }
        CloseHandle(snapshot);
        if found.len() != 1 {
            return Err(Error::new("SUSPENDED_THREAD_AMBIGUOUS"));
        }
        let t = OpenThread(THREAD_SUSPEND_RESUME, 0, found[0]);
        if t.is_null() {
            return Err(Error::new("SUSPENDED_THREAD_OPEN"));
        }
        let resumed = ResumeThread(t);
        CloseHandle(t);
        if resumed == u32::MAX {
            return Err(Error::new("SUSPENDED_THREAD_RESUME"));
        }
    }
    Ok(())
}

/// Stable local file-or-directory identity, excluding redirected components.
pub fn path_identity(path: &Path) -> Result<String> {
    paths::local_existing(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let m = std::fs::metadata(path).map_err(|_| Error::new("FILE_ID_FAILED"))?;
        Ok(format!("{}:{}", m.dev(), m.ino()))
    }
    #[cfg(windows)]
    {
        use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle};
        use windows_sys::Win32::{Foundation::HANDLE, Storage::FileSystem::*};
        let f = std::fs::OpenOptions::new()
            .access_mode(0)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
            .open(path)
            .map_err(|_| Error::new("FILE_ID_FAILED"))?;
        let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
        if unsafe { GetFileInformationByHandle(f.as_raw_handle() as HANDLE, &mut info) } == 0
            || info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
        {
            return Err(Error::new("FILE_ID_OR_REDIRECTION"));
        }
        Ok(format!(
            "{:08x}:{:08x}{:08x}",
            info.dwVolumeSerialNumber, info.nFileIndexHigh, info.nFileIndexLow
        ))
    }
    #[cfg(not(any(unix, windows)))]
    {
        Err(Error::new("FILE_ID_UNSUPPORTED"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn executable_pin_limit_accepts_large_agent_class_file() {
        let file = tempfile::NamedTempFile::new().unwrap();
        file.as_file().set_len(300 * 1024 * 1024).unwrap();
        let digest = hash_file(file.path()).unwrap();
        assert_eq!(digest.len(), 64);
    }

    #[test]
    fn executable_pin_limit_rejects_over_one_gib_without_reading() {
        let file = tempfile::NamedTempFile::new().unwrap();
        file.as_file().set_len(PIN_FILE_LIMIT_BYTES + 1).unwrap();
        assert_eq!(hash_file(file.path()).unwrap_err().code, "PIN_FILE_LIMIT");
    }
}
