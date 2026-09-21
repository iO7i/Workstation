use crate::{now, paths};
use std::{
    fs,
    path::Path,
    time::{Duration, Instant},
};
use workstation_core::*;

pub fn unavailable(root: Root, coverage: Coverage, code: &str) -> StorageObservation {
    StorageObservation {
        root,
        observed_at: now(),
        coverage,
        logical_entry_bytes: None,
        allocated_bytes: None,
        files: 0,
        directories: 0,
        visited_entries: 0,
        skipped_links_or_placeholders: 0,
        io_errors: 0,
        notes: vec![code.into()],
        delta_since_previous_complete_bytes: None,
    }
}

/// Metadata only: never opens user file contents. Hard-link entries are intentionally not deduplicated.
/// The supervisor imposes a hard wall-clock bound by containing this worker process.
pub fn scan(root: Root, budget: ScanBudget) -> StorageObservation {
    if paths::directory(&root.path).is_err() {
        return unavailable(root, Coverage::Denied, "ROOT_UNAVAILABLE_OR_REDIRECTED");
    }
    let budget = ScanBudget {
        deadline_ms: budget.deadline_ms.min(120_000),
        max_entries: budget.max_entries.min(100_000),
        max_depth: budget.max_depth.min(64),
    };
    let start = Instant::now();
    let mut s = unavailable(
        root.clone(),
        Coverage::Complete,
        "ENTRY_LOGICAL_BYTES_NOT_ALLOCATION",
    );
    s.logical_entry_bytes = Some(0);
    let mut pending = vec![(root.path.clone(), 0u32)];
    'walk: while let Some((dir, depth)) = pending.pop() {
        if start.elapsed() >= Duration::from_millis(budget.deadline_ms) {
            s.coverage = Coverage::TimedOut;
            s.notes.push("TIME_BUDGET".into());
            break;
        }
        // Recheck immediately before traversal; do not follow known redirects/placeholders.
        match fs::symlink_metadata(&dir) {
            Ok(m) if !paths::is_redirect_or_placeholder(&m) && m.is_dir() => {}
            _ => {
                s.coverage = Coverage::Partial;
                s.skipped_links_or_placeholders += 1;
                continue;
            }
        }
        let entries = match fs::read_dir(&dir) {
            Ok(v) => v,
            Err(_) => {
                s.io_errors += 1;
                s.coverage = Coverage::Partial;
                continue;
            }
        };
        s.directories += 1;
        for entry in entries {
            if start.elapsed() >= Duration::from_millis(budget.deadline_ms) {
                s.coverage = Coverage::TimedOut;
                s.notes.push("TIME_BUDGET".into());
                break 'walk;
            }
            if s.visited_entries >= budget.max_entries {
                s.coverage = Coverage::Partial;
                s.notes.push("ENTRY_BUDGET".into());
                break 'walk;
            }
            s.visited_entries += 1;
            let entry = match entry {
                Ok(e) => e,
                Err(_) => {
                    s.io_errors += 1;
                    s.coverage = Coverage::Partial;
                    continue;
                }
            };
            let m = match fs::symlink_metadata(entry.path()) {
                Ok(m) => m,
                Err(_) => {
                    s.io_errors += 1;
                    s.coverage = Coverage::Partial;
                    continue;
                }
            };
            if paths::is_redirect_or_placeholder(&m) {
                s.skipped_links_or_placeholders += 1;
                s.coverage = Coverage::Partial;
                continue;
            }
            if m.is_file() {
                match s.logical_entry_bytes.and_then(|v| v.checked_add(m.len())) {
                    Some(n) => {
                        s.logical_entry_bytes = Some(n);
                        s.files += 1;
                    }
                    None => {
                        s.logical_entry_bytes = None;
                        s.coverage = Coverage::Partial;
                        s.notes.push("BYTE_OVERFLOW".into());
                        break 'walk;
                    }
                }
            } else if m.is_dir() {
                if depth >= budget.max_depth {
                    s.coverage = Coverage::Partial;
                    s.notes.push("DEPTH_BUDGET".into());
                } else if pending.len() >= 2048 {
                    s.coverage = Coverage::Partial;
                    s.notes.push("PENDING_DIRECTORY_BUDGET".into());
                } else {
                    pending.push((entry.path(), depth + 1));
                }
            } else {
                s.coverage = Coverage::Partial;
                s.skipped_links_or_placeholders += 1;
            }
        }
    }
    s.notes.sort();
    s.notes.dedup();
    s
}

pub fn discover() -> Vec<Root> {
    let mut out = Vec::new();
    let mut add = |id: &str, path: std::path::PathBuf, kind: RootKind| {
        // These are candidates only; no registration or claim about an application's active home.
        if paths::directory(&path).is_ok() {
            out.push(Root {
                id: id.into(),
                path,
                kind,
            });
        }
    };
    if let Some(home) = std::env::var_os("USERPROFILE") {
        let p = std::path::PathBuf::from(home);
        add("codex-default", p.join(".codex"), RootKind::Codex);
        add("claude-default", p.join(".claude"), RootKind::Claude);
        add(
            "codex-workspaces",
            p.join("Documents").join("Codex"),
            RootKind::Workspace,
        );
    }
    if let Some(home) = std::env::var_os("CODEX_HOME") {
        add("codex-shell-override", home.into(), RootKind::Codex);
    }
    if let Some(home) = std::env::var_os("LOCALAPPDATA") {
        let p = std::path::PathBuf::from(home);
        add(
            "codex-runtime-candidate",
            p.join("OpenAI").join("Codex"),
            RootKind::Codex,
        );
        add("docker-local-candidate", p.join("Docker"), RootKind::Docker);
    }
    if let Some(home) = std::env::var_os("APPDATA") {
        let p = std::path::PathBuf::from(home);
        add("cursor-candidate", p.join("Cursor"), RootKind::Cursor);
        add("vscode-candidate", p.join("Code"), RootKind::Vscode);
    }
    out
}

pub fn is_plain_file(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|m| m.is_file() && !paths::is_redirect_or_placeholder(&m))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn root(p: &Path) -> Root {
        Root {
            id: "test".into(),
            path: p.into(),
            kind: RootKind::Other,
        }
    }
    #[test]
    fn sizes_metadata_without_reading_file_contents() {
        let t = tempfile::tempdir().unwrap();
        fs::write(t.path().join("a"), b"secret-canary").unwrap();
        let s = scan(root(t.path()), ScanBudget::deep());
        assert_eq!(s.logical_entry_bytes, Some(13));
        assert_eq!(s.coverage, Coverage::Complete);
        assert!(!serde_json::to_string(&s).unwrap().contains("secret-canary"));
    }
    #[test]
    fn budget_makes_partial_not_complete() {
        let t = tempfile::tempdir().unwrap();
        for i in 0..5 {
            fs::write(t.path().join(i.to_string()), b"x").unwrap();
        }
        let s = scan(
            root(t.path()),
            ScanBudget {
                deadline_ms: 1000,
                max_entries: 2,
                max_depth: 8,
            },
        );
        assert_eq!(s.coverage, Coverage::Partial);
        assert_eq!(s.visited_entries, 2);
    }
    #[test]
    fn zero_deadline_times_out() {
        let t = tempfile::tempdir().unwrap();
        let s = scan(
            root(t.path()),
            ScanBudget {
                deadline_ms: 0,
                max_entries: 20,
                max_depth: 8,
            },
        );
        assert_eq!(s.coverage, Coverage::TimedOut);
    }
    #[test]
    fn missing_root_is_not_zero_bytes() {
        let t = tempfile::tempdir().unwrap();
        let s = scan(root(&t.path().join("missing")), ScanBudget::normal());
        assert_eq!(s.logical_entry_bytes, None);
        assert_eq!(s.coverage, Coverage::Denied);
    }
    #[cfg(unix)]
    #[test]
    fn no_symlink_traversal() {
        let t = tempfile::tempdir().unwrap();
        let external = tempfile::tempdir().unwrap();
        fs::write(external.path().join("private"), vec![b'x'; 800]).unwrap();
        std::os::unix::fs::symlink(external.path(), t.path().join("escape")).unwrap();
        let s = scan(root(t.path()), ScanBudget::deep());
        assert_eq!(s.logical_entry_bytes, Some(0));
        assert_eq!(s.skipped_links_or_placeholders, 1);
        assert_eq!(s.coverage, Coverage::Partial);
    }
}
