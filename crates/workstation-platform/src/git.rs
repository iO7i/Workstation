use crate::{now, paths, Error, Result};
use std::{
    io::Read,
    process::{Command, Stdio},
};
use workstation_core::*;

/// Lossless NUL-delimited records; do not parse human output or quote-escaped paths.
pub fn parse_worktrees(bytes: &[u8]) -> Result<Vec<Worktree>> {
    if bytes.len() > 1024 * 1024 || (!bytes.is_empty() && !bytes.ends_with(&[0])) {
        return Err(Error::new("GIT_OUTPUT_INVALID"));
    }
    let mut result = Vec::new();
    let mut current: Option<Worktree> = None;
    for raw in bytes.split(|b| *b == 0) {
        if raw.is_empty() {
            if let Some(w) = current.take() {
                result.push(w);
            }
            if result.len() > 200 {
                return Err(Error::new("WORKTREE_COUNT_LIMIT"));
            }
            continue;
        }
        let line =
            std::str::from_utf8(raw).map_err(|_| Error::new("GIT_PATH_ENCODING_UNSUPPORTED"))?;
        if let Some(path) = line.strip_prefix("worktree ") {
            if current.is_some() {
                return Err(Error::new("GIT_RECORD_BOUNDARY_INVALID"));
            }
            current = Some(Worktree {
                path: path.into(),
                head: None,
                branch: None,
                detached: false,
                bare: false,
                locked: false,
                prunable: false,
                disposition: "protected".into(),
                content_coverage: Coverage::Unsupported,
            });
        } else {
            let w = current
                .as_mut()
                .ok_or_else(|| Error::new("GIT_RECORD_ORDER_INVALID"))?;
            if let Some(head) = line.strip_prefix("HEAD ") {
                if ![40, 64].contains(&head.len()) || !head.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return Err(Error::new("GIT_HEAD_INVALID"));
                }
                w.head = Some(head.into());
            } else if let Some(branch) = line.strip_prefix("branch ") {
                w.branch = Some(branch.into());
            } else if line == "detached" {
                w.detached = true;
            } else if line == "bare" {
                w.bare = true;
            } else if line == "locked" || line.starts_with("locked ") {
                w.locked = true;
            } else if line == "prunable" || line.starts_with("prunable ") {
                w.prunable = true;
            } else {
                return Err(Error::new("GIT_SCHEMA_UNSUPPORTED"));
            }
        }
    }
    if current.is_some() {
        return Err(Error::new("GIT_RECORD_INCOMPLETE"));
    }
    Ok(result)
}

/// This function MUST run inside our contained worker, never directly from the CLI.
pub fn collect(project: Project) -> WorkspaceObservation {
    let mut result = WorkspaceObservation {
        project_id: project.id,
        observed_at: now(),
        coverage: Coverage::Denied,
        worktrees: vec![],
        notes: vec!["REGISTRATION_ONLY_CONTENT_AND_SESSION_OWNERSHIP_UNKNOWN".into()],
    };
    if paths::directory(&project.path).is_err()
        || paths::local_existing(&project.git_executable).is_err()
    {
        result.notes.push("GIT_OR_REPOSITORY_UNAVAILABLE".into());
        return result;
    }
    #[cfg(windows)]
    if !project
        .git_executable
        .file_name()
        .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case("git.exe"))
    {
        result.notes.push("NATIVE_GIT_EXE_REQUIRED".into());
        return result;
    }
    let mut cmd = Command::new(&project.git_executable);
    cmd.env_clear()
        .current_dir(&project.path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    for key in ["SystemRoot", "WINDIR"] {
        if let Some(value) = std::env::var_os(key) {
            cmd.env(key, value);
        }
    }
    cmd.env("GIT_CONFIG_NOSYSTEM", "1")
        .env(
            "GIT_CONFIG_GLOBAL",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
        )
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_NO_LAZY_FETCH", "1")
        .env("GIT_PAGER", "")
        .env("GIT_PROTOCOL_FROM_USER", "0")
        .args([
            "--no-pager",
            "--no-optional-locks",
            "-c",
            "core.fsmonitor=false",
            "-c",
            "core.hooksPath=",
            "-c",
            "protocol.allow=never",
            "worktree",
            "list",
            "--porcelain",
            "-z",
        ]);
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(_) => {
            result.notes.push("GIT_START_FAILED".into());
            return result;
        }
    };
    let mut bytes = Vec::new();
    let read = match child.stdout.take() {
        Some(out) => out.take(1024 * 1024 + 1).read_to_end(&mut bytes),
        None => Err(std::io::Error::other("missing pipe")),
    };
    if read.is_err() || bytes.len() > 1024 * 1024 {
        let _ = child.kill();
        let _ = child.wait();
        result.coverage = Coverage::Partial;
        result.notes.push("GIT_READ_OR_OUTPUT_LIMIT".into());
        return result;
    }
    let status = child.wait();
    if !status.is_ok_and(|s| s.success()) {
        result.notes.push("GIT_OPERATION_FAILED".into());
        return result;
    }
    match parse_worktrees(&bytes) {
        Ok(trees) => {
            result.worktrees = trees;
            result.coverage = Coverage::Complete;
        }
        Err(e) => {
            result.coverage = Coverage::Unsupported;
            result.notes.push(e.code.into());
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_locked_unicode_and_spaces() {
        let b = format!(
            "worktree D:/مشروع name\0HEAD {}\0branch refs/heads/main\0locked busy\0\0",
            "a".repeat(40)
        );
        let w = parse_worktrees(b.as_bytes()).unwrap();
        assert_eq!(w.len(), 1);
        assert!(w[0].locked);
        assert_eq!(w[0].path, "D:/مشروع name");
        assert_eq!(w[0].disposition, "protected");
    }
    #[test]
    fn detached_work_is_protected() {
        let b = format!("worktree D:/x\0HEAD {}\0detached\0\0", "b".repeat(40));
        let w = parse_worktrees(b.as_bytes()).unwrap();
        assert!(w[0].detached);
        assert_eq!(w[0].content_coverage, Coverage::Unsupported);
    }
    #[test]
    fn rejects_truncated_stream() {
        assert!(parse_worktrees(b"worktree D:/x").is_err());
    }
    #[test]
    fn rejects_unknown_format() {
        assert!(parse_worktrees(b"worktree D:/x\0new-field x\0\0").is_err());
    }
    #[test]
    fn rejects_invalid_utf8() {
        assert!(parse_worktrees(b"worktree D:/\xff\0\0").is_err());
    }
    #[test]
    fn rejects_human_output() {
        assert!(parse_worktrees(b"D:/x abcd [main]\n").is_err());
    }
    #[test]
    fn bounded_malformed_inputs_never_panic() {
        let mut seed: u64 = 0x72a598;
        for length in 0..512 {
            let mut bytes = Vec::with_capacity(length);
            for _ in 0..length {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                bytes.push((seed >> 32) as u8);
            }
            let _ = parse_worktrees(&bytes);
        }
    }
}
