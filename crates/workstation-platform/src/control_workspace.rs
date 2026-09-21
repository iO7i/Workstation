//! Git metadata capture. Call only from the contained collector process.
//! No content cleanup, checkout, reset, commit, filter execution or fetch capability.
use crate::{control_store::epoch, paths, Error, Result};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
use workstation_core::{
    control::{self, WorkspaceStamp},
    Coverage, Project,
};
const CAP: usize = 1024 * 1024;
pub fn directory_identity(path: &Path) -> Result<String> {
    paths::directory(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let m = fs::metadata(path).map_err(|_| Error::new("DIRECTORY_ID_FAILED"))?;
        Ok(format!("{}:{}", m.dev(), m.ino()))
    }
    #[cfg(windows)]
    {
        use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle};
        use windows_sys::Win32::{Foundation::HANDLE, Storage::FileSystem::*};
        let file = fs::OpenOptions::new()
            .access_mode(0)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
            .open(path)
            .map_err(|_| Error::new("DIRECTORY_ID_FAILED"))?;
        let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
        if unsafe { GetFileInformationByHandle(file.as_raw_handle() as HANDLE, &mut info) } == 0 {
            return Err(Error::new("DIRECTORY_ID_FAILED"));
        }
        if info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(Error::new("DIRECTORY_REDIRECTED"));
        }
        Ok(format!(
            "{:08x}:{:08x}{:08x}",
            info.dwVolumeSerialNumber, info.nFileIndexHigh, info.nFileIndexLow
        ))
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = path;
        Err(Error::new("DIRECTORY_ID_UNSUPPORTED"))
    }
}
pub(crate) fn command(
    project: &Project,
    path: &Path,
    args: &[&str],
    allow_one: bool,
) -> Result<Vec<u8>> {
    paths::directory(path)?;
    paths::local_existing(&project.git_executable)?;
    #[cfg(windows)]
    if !project
        .git_executable
        .file_name()
        .is_some_and(|v| v.to_string_lossy().eq_ignore_ascii_case("git.exe"))
    {
        return Err(Error::new("NATIVE_GIT_EXE_REQUIRED"));
    }
    let mut cmd = Command::new(&project.git_executable);
    cmd.current_dir(path)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    for k in ["SystemRoot", "WINDIR"] {
        if let Some(v) = std::env::var_os(k) {
            cmd.env(k, v);
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
        .env("GIT_PROTOCOL_FROM_USER", "0")
        .args([
            "--no-pager",
            "--no-optional-locks",
            "-c",
            "core.fsmonitor=false",
            "-c",
            "core.hooksPath=",
            "-c",
            "core.untrackedCache=false",
            "-c",
            "core.preloadIndex=false",
            "-c",
            "protocol.allow=never",
            "-c",
            "diff.external=",
            "-c",
            "submodule.recurse=false",
        ])
        .args(args);
    let mut child = cmd.spawn().map_err(|_| Error::new("GIT_START_FAILED"))?;
    let mut bytes = Vec::new();
    let result = child
        .stdout
        .take()
        .ok_or_else(|| Error::new("GIT_PIPE_MISSING"))?
        .take(CAP as u64 + 1)
        .read_to_end(&mut bytes);
    if result.is_err() || bytes.len() > CAP {
        let _ = child.kill();
        return Err(Error::new("GIT_OUTPUT_LIMIT"));
    }
    // Any wait here is contained by the parent collector deadline/job, not a CLI wait.
    let status = child.wait().map_err(|_| Error::new("GIT_WAIT_FAILED"))?;
    if !status.success() && !(allow_one && status.code() == Some(1)) {
        return Err(Error::new("GIT_QUERY_FAILED"));
    }
    Ok(bytes)
}
fn utf8(bytes: &[u8]) -> Result<&str> {
    std::str::from_utf8(bytes).map_err(|_| Error::new("GIT_PATH_ENCODING_UNSUPPORTED"))
}
fn same_path(a: &Path, b: &Path) -> bool {
    #[cfg(windows)]
    {
        a.to_string_lossy()
            .replace('\\', "/")
            .eq_ignore_ascii_case(&b.to_string_lossy().replace('\\', "/"))
    }
    #[cfg(not(windows))]
    {
        a == b
    }
}
/// Returns relative porcelain v1 paths; rename's second path is not a new status record.
pub fn status_paths(bytes: &[u8]) -> Result<(bool, u64, u64, Vec<String>)> {
    if !bytes.is_empty() && !bytes.ends_with(&[0]) {
        return Err(Error::new("STATUS_TRUNCATED"));
    }
    let mut records = bytes.split(|b| *b == 0).filter(|x| !x.is_empty());
    let mut dirty = false;
    let (mut untracked, mut ignored) = (0, 0);
    let mut paths = Vec::new();
    while let Some(record) = records.next() {
        if record.len() < 4
            || record[2] != b' '
            || !record[..2].iter().all(|b| b" MADRCU?!T".contains(b))
        {
            return Err(Error::new("STATUS_SCHEMA_UNSUPPORTED"));
        }
        let code = &record[..2];
        let p = utf8(&record[3..])?;
        validate_relative(p)?;
        paths.push(p.into());
        if code == b"!!" {
            ignored += 1;
        } else {
            dirty = true;
            if code == b"??" {
                untracked += 1;
            }
        }
        if code.contains(&b'R') || code.contains(&b'C') {
            let old = records
                .next()
                .ok_or_else(|| Error::new("STATUS_RENAME_TRUNCATED"))?;
            let p = utf8(old)?;
            validate_relative(p)?;
            paths.push(p.into());
        }
        if paths.len() > 10_000 {
            return Err(Error::new("STATUS_PATH_LIMIT"));
        }
    }
    Ok((dirty, untracked, ignored, paths))
}
fn validate_relative(s: &str) -> Result<()> {
    if s.is_empty()
        || Path::new(s).is_absolute()
        || s.contains('\\')
        || s.contains(':')
        || s.split('/').any(|x| x == ".." || x == ".")
    {
        return Err(Error::new("STATUS_PATH_UNSAFE"));
    }
    Ok(())
}
/// Inspect `git ls-files -v -z` without opening contents. Lowercase tags mean
/// assume-unchanged; S/s means skip-worktree. Both invalidate a clean-state claim.
fn index_visibility_blockers(bytes: &[u8]) -> Result<Vec<String>> {
    if !bytes.is_empty() && !bytes.ends_with(&[0]) {
        return Err(Error::new("INDEX_FLAGS_TRUNCATED"));
    }
    let mut blockers = Vec::new();
    for record in bytes.split(|b| *b == 0).filter(|r| !r.is_empty()) {
        if record.len() < 3 || record[1] != b' ' || !b"HSMRCK?hsmrck".contains(&record[0]) {
            return Err(Error::new("INDEX_FLAGS_UNSUPPORTED"));
        }
        if record[0].is_ascii_lowercase() {
            blockers.push("git_assume_unchanged_content_hidden".into());
        }
        if record[0].eq_ignore_ascii_case(&b'S') {
            blockers.push("git_skip_worktree_content_hidden".into());
        }
    }
    blockers.sort();
    blockers.dedup();
    Ok(blockers)
}

pub fn capture(project: Project, workspace_id: String, path: PathBuf) -> Result<WorkspaceStamp> {
    control::id(&workspace_id).map_err(Error::new)?;
    paths::directory(&path)?;
    let initial = directory_identity(&path)?;
    let registered = crate::git::collect(project.clone());
    if !registered.coverage.is_complete() {
        return Err(Error::new("GIT_MEMBERSHIP_UNKNOWN"));
    }
    let selected = registered
        .worktrees
        .iter()
        .find(|w| {
            let candidate = Path::new(&w.path);
            same_path(candidate, &path)
                || directory_identity(candidate).is_ok_and(|identity| identity == initial)
        })
        .ok_or_else(|| Error::new("WORKSPACE_NOT_REGISTERED_WITH_GIT"))?;
    if selected.bare || selected.prunable {
        return Err(Error::new("WORKSPACE_UNUSABLE"));
    }
    let head = selected
        .head
        .clone()
        .ok_or_else(|| Error::new("WORKSPACE_HEAD_UNKNOWN"))?;
    let mut stamp = WorkspaceStamp {
        project_id: project.id.clone(),
        workspace_id,
        path: path
            .to_str()
            .ok_or_else(|| Error::new("PATH_ENCODING_UNSUPPORTED"))?
            .into(),
        directory_identity: initial.clone(),
        head: head.clone(),
        branch: selected.branch.clone(),
        status_digest: String::new(),
        dirty: None,
        untracked: None,
        ignored: None,
        local_only_commits: None,
        blockers: vec![],
        observed_at: epoch(),
        coverage: Coverage::Partial,
    };
    if selected.locked {
        stamp.blockers.push("git_worktree_locked".into());
    }
    match fs::symlink_metadata(path.join(".gitmodules")) {
        Ok(_) => stamp.blockers.push("submodule_state_not_certified".into()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => stamp.blockers.push("submodule_marker_unreadable".into()),
    }
    let initial_flags = command(&project, &path, &["ls-files", "-v", "-z"], false)?;
    stamp
        .blockers
        .extend(index_visibility_blockers(&initial_flags)?);
    for key in ["core.sparseCheckout", "index.sparse"] {
        let value = command(&project, &path, &["config", "--bool", "--get", key], true)?;
        if utf8(&value)?.trim() == "true" {
            stamp.blockers.push("sparse_checkout_not_certified".into());
        }
    }
    // A clean filter may execute during status. Inspect names only and skip status entirely.
    let filters = command(
        &project,
        &path,
        &[
            "config",
            "--name-only",
            "--get-regexp",
            r"^filter\..*\.(clean|process)$",
        ],
        true,
    )?;
    let status = if filters.is_empty() {
        command(
            &project,
            &path,
            &[
                "status",
                "--porcelain=v1",
                "-z",
                "--untracked-files=all",
                "--ignored=matching",
                "--ignore-submodules=all",
            ],
            false,
        )?
    } else {
        stamp
            .blockers
            .push("external_git_filter_status_skipped".into());
        Vec::new()
    };
    let mut digest = Sha256::new();
    digest.update(&status);
    if filters.is_empty() {
        let (dirty, untracked, ignored, files) = status_paths(&status)?;
        stamp.dirty = Some(dirty);
        stamp.untracked = Some(untracked);
        stamp.ignored = Some(ignored);
        for rel in files {
            let file = path.join(&rel);
            digest.update(rel.as_bytes());
            match fs::symlink_metadata(&file) {
                Ok(m) => {
                    if paths::is_redirect_or_placeholder(&m) {
                        stamp.blockers.push("redirected_changed_path".into());
                        continue;
                    }
                    if let Some(parent) = file.parent() {
                        if paths::directory(parent).is_err() {
                            stamp
                                .blockers
                                .push("changed_path_ancestor_unverified".into());
                            continue;
                        }
                    }
                    digest.update(m.len().to_le_bytes());
                    if let Ok(time) = m.modified().and_then(|x| {
                        x.duration_since(std::time::UNIX_EPOCH)
                            .map_err(std::io::Error::other)
                    }) {
                        digest.update(time.as_nanos().to_le_bytes());
                    } else {
                        stamp.blockers.push("changed_path_time_unavailable".into());
                    }
                    if m.is_dir() {
                        stamp
                            .blockers
                            .push("untracked_or_ignored_directory_not_fully_fingerprinted".into());
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => digest.update(b"missing"),
                Err(_) => stamp
                    .blockers
                    .push("changed_path_metadata_unreadable".into()),
            }
        }
    }
    let local = command(
        &project,
        &path,
        &["rev-list", "--count", "HEAD", "--not", "--remotes"],
        false,
    )?;
    stamp.local_only_commits = utf8(&local)?.trim().parse().ok();
    if stamp.local_only_commits.is_none() {
        stamp.blockers.push("local_commit_count_unknown".into());
    }
    for name in [
        "MERGE_HEAD",
        "CHERRY_PICK_HEAD",
        "REVERT_HEAD",
        "rebase-merge",
        "rebase-apply",
        "BISECT_LOG",
    ] {
        let p = command(
            &project,
            &path,
            &["rev-parse", "--path-format=absolute", "--git-path", name],
            false,
        )?;
        let p = PathBuf::from(utf8(&p)?.trim());
        match fs::symlink_metadata(p) {
            Ok(_) => stamp.blockers.push(format!("git_operation_{name}")),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => stamp
                .blockers
                .push(format!("git_operation_{name}_unreadable")),
        }
    }
    let final_head = command(&project, &path, &["rev-parse", "--verify", "HEAD"], false)?;
    if utf8(&final_head)?.trim() != head || directory_identity(&path)? != initial {
        return Err(Error::new("WORKSPACE_CHANGED_DURING_OBSERVATION"));
    }
    if filters.is_empty() {
        let end_status = command(
            &project,
            &path,
            &[
                "status",
                "--porcelain=v1",
                "-z",
                "--untracked-files=all",
                "--ignored=matching",
                "--ignore-submodules=all",
            ],
            false,
        )?;
        if end_status != status {
            return Err(Error::new("STATUS_CHANGED_DURING_OBSERVATION"));
        }
    }
    let final_flags = command(&project, &path, &["ls-files", "-v", "-z"], false)?;
    if final_flags != initial_flags {
        return Err(Error::new("INDEX_FLAGS_CHANGED_DURING_OBSERVATION"));
    }
    digest.update(&initial_flags);
    stamp.status_digest = format!("{:x}", digest.finalize());
    stamp.blockers.sort();
    stamp.blockers.dedup();
    stamp.coverage = if stamp.blockers.is_empty() {
        Coverage::Complete
    } else {
        Coverage::Partial
    };
    // Metadata fingerprint, not a byte-content digest. Same-size/same-mtime edits may evade it.
    // Handoff recipient MUST inspect and test the working tree. No cleanup permission follows.
    Ok(stamp)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ordinary_index_has_no_hidden_state() {
        assert!(index_visibility_blockers(b"H tracked.txt\0")
            .unwrap()
            .is_empty());
    }
    #[test]
    fn assume_unchanged_blocks_clean_claim() {
        assert_eq!(
            index_visibility_blockers(b"h tracked.txt\0").unwrap(),
            vec!["git_assume_unchanged_content_hidden"]
        );
    }
    #[test]
    fn skip_worktree_blocks_clean_claim() {
        assert_eq!(
            index_visibility_blockers(b"S tracked.txt\0").unwrap(),
            vec!["git_skip_worktree_content_hidden"]
        );
    }
    #[test]
    fn combined_index_flags_both_block() {
        assert_eq!(
            index_visibility_blockers(b"s tracked.txt\0").unwrap().len(),
            2
        );
    }
    #[test]
    fn truncated_index_flags_are_unknown() {
        assert!(index_visibility_blockers(b"H tracked.txt").is_err());
    }
    #[test]
    fn ignored_not_clean_disposable() {
        let (d, u, i, _) = status_paths(b"!! .env\0?? note.txt\0").unwrap();
        assert!(d);
        assert_eq!((u, i), (1, 1));
    }
    #[test]
    fn rename_has_two_paths() {
        let (_, _, _, p) = status_paths(b"R  new.txt\0old.txt\0").unwrap();
        assert_eq!(p.len(), 2);
    }
    #[test]
    fn traversal_rejected() {
        assert!(status_paths(b"?? ../secret\0").is_err());
    }
    #[test]
    fn truncated_rejected() {
        assert!(status_paths(b" M file").is_err());
    }
    #[test]
    fn secret_content_not_read() {
        let (_, _, i, p) = status_paths(b"!! .env\0").unwrap();
        assert_eq!(i, 1);
        assert_eq!(p[0], ".env");
    }
}
