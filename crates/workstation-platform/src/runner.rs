//! Only launches our own stdin-gated collector. There is no public arbitrary-command runner.
use crate::{Error, Result};
use std::{
    io::{Read, Write},
    path::Path,
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};
use workstation_core::{WorkerRequest, MAX_WIRE_BYTES};

pub struct Output {
    pub bytes: Vec<u8>,
    pub exit_code: i32,
}

#[cfg(unix)]
pub(crate) struct UnixGroup(u32);
#[cfg(unix)]
impl UnixGroup {
    fn terminate(&self) {
        unsafe extern "C" {
            fn kill(pid: i32, signal: i32) -> i32;
        }
        // SAFETY: negative PID targets the private process group created for our worker only.
        unsafe {
            kill(-(self.0 as i32), 9);
        }
    }
}
#[cfg(unix)]
impl Drop for UnixGroup {
    fn drop(&mut self) {
        self.terminate();
    }
}

pub(crate) enum Guard {
    #[cfg(windows)]
    Windows(crate::windows::Containment),
    #[cfg(unix)]
    Unix(UnixGroup),
}
impl Guard {
    pub(crate) fn attach(child: &Child) -> Result<Self> {
        #[cfg(windows)]
        {
            Ok(Self::Windows(crate::windows::Containment::attach(child)?))
        }
        #[cfg(unix)]
        {
            Ok(Self::Unix(UnixGroup(child.id())))
        }
    }
    pub(crate) fn terminate(&self) {
        match self {
            #[cfg(windows)]
            Self::Windows(j) => j.terminate(),
            #[cfg(unix)]
            Self::Unix(g) => g.terminate(),
        }
    }
}

fn reader(
    mut stream: impl Read + Send + 'static,
    cap: usize,
    overflow: Arc<AtomicBool>,
) -> thread::JoinHandle<std::io::Result<Vec<u8>>> {
    thread::spawn(move || {
        let mut kept = Vec::new();
        let mut buf = [0u8; 8192];
        loop {
            let n = stream.read(&mut buf)?;
            if n == 0 {
                break;
            }
            let remain = cap.saturating_sub(kept.len());
            kept.extend_from_slice(&buf[..n.min(remain)]);
            if n > remain {
                overflow.store(true, Ordering::SeqCst);
            }
        }
        Ok(kept)
    })
}

pub fn collect(
    executable: &Path,
    cwd: &Path,
    request: &WorkerRequest,
    timeout: Duration,
) -> Result<Output> {
    if !executable.is_absolute() || !cwd.is_absolute() {
        return Err(Error::new("RUNNER_ABSOLUTE_PATH_REQUIRED"));
    }
    let input = serde_json::to_vec(request).map_err(|_| Error::new("REQUEST_SERIALIZE"))?;
    if input.len() > 64 * 1024 {
        return Err(Error::new("REQUEST_TOO_LARGE"));
    }
    let mut cmd = Command::new(executable);
    cmd.arg("__collector")
        .current_dir(cwd)
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for key in ["SystemRoot", "WINDIR", "ComSpec"] {
        if let Some(value) = std::env::var_os(key) {
            cmd.env(key, value);
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    let mut child = cmd.spawn().map_err(|_| Error::new("WORKER_SPAWN_FAILED"))?;
    // The trusted worker blocks on stdin until the parent assigns containment.
    let guard = match Guard::attach(&child) {
        Ok(g) => g,
        Err(e) => {
            let _ = child.kill();
            let _ = reap_bounded(&mut child, Duration::from_millis(750));
            return Err(e);
        }
    };
    let overflow = Arc::new(AtomicBool::new(false));
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| Error::new("WORKER_PIPE_MISSING"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| Error::new("WORKER_PIPE_MISSING"))?;
    let out_reader = reader(stdout, MAX_WIRE_BYTES, overflow.clone());
    let err_reader = reader(stderr, 16 * 1024, overflow.clone());
    let start = Instant::now();
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| Error::new("WORKER_PIPE_MISSING"))?;
    let input_writer = thread::spawn(move || stdin.write_all(&input));
    let mut failure = None;
    let mut code = -1;
    while failure.is_none() {
        if overflow.load(Ordering::SeqCst) {
            failure = Some("WORKER_OUTPUT_LIMIT");
            break;
        }
        if start.elapsed() >= timeout {
            failure = Some("WORKER_TIMEOUT");
            break;
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                code = status.code().unwrap_or(-1);
                break;
            }
            Ok(None) => thread::sleep(Duration::from_millis(10)),
            Err(_) => {
                failure = Some("WORKER_WAIT_FAILED");
                break;
            }
        }
    }
    // Also clean up descendants that survived a normally exited worker.
    guard.terminate();
    drop(guard);
    let cleanup_deadline = Instant::now() + Duration::from_millis(750);
    if !reap_bounded(&mut child, Duration::from_millis(750)) {
        return Err(Error::new("WORKER_CLEANUP_UNCONFIRMED"));
    }
    // Never join indefinitely if a pipe is retained by a stuck kernel operation.
    while !(input_writer.is_finished() && out_reader.is_finished() && err_reader.is_finished()) {
        if Instant::now() >= cleanup_deadline {
            return Err(Error::new("WORKER_CLEANUP_UNCONFIRMED"));
        }
        thread::sleep(Duration::from_millis(5));
    }
    let input_result = input_writer
        .join()
        .map_err(|_| Error::new("WORKER_WRITER_FAILED"))?;
    if failure.is_none() && input_result.is_err() {
        failure = Some("WORKER_INPUT_FAILED");
    }
    let bytes = out_reader
        .join()
        .map_err(|_| Error::new("WORKER_READER_FAILED"))?
        .map_err(|_| Error::new("WORKER_READ_FAILED"))?;
    let _discarded = err_reader
        .join()
        .map_err(|_| Error::new("WORKER_READER_FAILED"))?;
    if overflow.load(Ordering::SeqCst) {
        failure = Some("WORKER_OUTPUT_LIMIT");
    }
    if let Some(reason) = failure {
        return Err(Error::new(reason));
    }
    if code != 0 {
        return Err(Error::new("WORKER_FAILED"));
    }
    Ok(Output {
        bytes,
        exit_code: code,
    })
}

fn reap_bounded(child: &mut Child, budget: Duration) -> bool {
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return true,
            Err(_) => return false,
            Ok(None) => {}
        }
        if start.elapsed() >= budget {
            return false;
        }
        thread::sleep(Duration::from_millis(5));
    }
}
