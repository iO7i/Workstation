//! Bounded adapter-aware metadata inspection, no transcript/credential values or vendor launch.
use crate::{external, paths, Error, Result};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use workstation_core::{
    integrations::{Adapter, Integration},
    Coverage,
};
fn entry(path: &Path) -> Value {
    if let Some(parent) = path.parent() {
        if paths::directory(parent).is_ok()
            && matches!(fs::symlink_metadata(path),Err(ref e) if e.kind()==std::io::ErrorKind::NotFound)
        {
            return json!({"path":path,"exists":false,"coverage":"complete"});
        }
    }
    match paths::local_existing(path) {
        Ok(_) => match fs::symlink_metadata(path) {
            Ok(m) => {
                json!({"path":path,"exists":true,"kind":if m.is_dir(){"directory"}else if m.is_file(){"file"}else{"special"},"logical_bytes_if_file":if m.is_file(){Some(m.len())}else{None},"coverage":"complete"})
            }
            Err(_) => json!({"path":path,"exists":null,"coverage":"partial"}),
        },
        Err(e) => json!({"path":path,"exists":null,"coverage":"partial","reason":e.code}),
    }
}
fn configured(i: &Integration, key: &str) -> Option<PathBuf> {
    i.configuration.get(key).map(PathBuf::from).or_else(|| {
        if i.inherit_env.iter().any(|k| k == key) {
            std::env::var_os(key).map(PathBuf::from)
        } else {
            None
        }
    })
}
pub fn inspect(i: &Integration) -> Result<Value> {
    i.validate().map_err(Error::new)?;
    let identity = external::hash_file(Path::new(&i.executable))?;
    let mut roots: Vec<(PathBuf, &str)> = vec![];
    let mut notices = vec![];
    match i.adapter {
        Adapter::Codex => {
            if let Some(p) = configured(i, "CODEX_HOME") {
                roots.push((p, "codex"));
            } else {
                notices.push("EFFECTIVE_CODEX_HOME_NOT_DECLARED");
            }
        }
        Adapter::Claude => {
            if let Some(p) = configured(i, "CLAUDE_CONFIG_DIR") {
                roots.push((p, "claude"));
            } else {
                notices.push("EFFECTIVE_CLAUDE_HOME_NOT_DECLARED");
            }
        }
        Adapter::CursorAcp => {
            if let Some(p) = configured(i, "APPDATA") {
                roots.push((p.join("Cursor"), "cursor"));
            } else {
                notices.push("EDITOR_USER_DATA_NOT_DECLARED");
            }
        }
        Adapter::Copilot => {
            if let Some(p) = configured(i, "COPILOT_HOME") {
                roots.push((p, "copilot-cli"));
            }
            if let Some(p) = configured(i, "APPDATA") {
                roots.push((p.join("Code"), "vscode"));
            }
            if roots.is_empty() {
                notices.push("COPILOT_CLI_AND_EDITOR_STATE_NOT_DECLARED");
            }
        }
        Adapter::GeminiAcp => {
            if let Some(p) = configured(i, "GEMINI_CLI_HOME") {
                roots.push((p.join(".gemini"), "gemini"));
            } else {
                notices.push("GEMINI_HOME_NOT_DECLARED");
            }
        }
        Adapter::OpenCode => {
            if let Some(p) = configured(i, "XDG_DATA_HOME") {
                roots.push((p.join("opencode"), "opencode"));
            } else {
                notices.push("OPENCODE_HOME_NOT_DECLARED");
            }
        }
        Adapter::Windsurf => {
            if let Some(p) = configured(i, "APPDATA") {
                roots.push((p.join("Windsurf"), "windsurf"));
            }
        }
        _ => notices.push("NO_PRIVATE_FORMAT_DIAGNOSTIC_ADAPTER"),
    }
    let mut state = vec![];
    let mut signals = BTreeMap::<String, Value>::new();
    if i.adapter == Adapter::Codex {
        if let Some(local) = configured(i, "LOCALAPPDATA") {
            signals.insert(
                "staging".into(),
                staging_metadata(&local.join("OpenAI").join("Codex").join("runtimes")),
            );
        }
    }
    for (root, kind) in roots {
        state.push(entry(&root));
        let subpaths: &[&str] = match kind {
            "codex" => &[
                "attachments/pasted-text-attachments.json",
                "sessions",
                "worktrees",
                "config.toml",
            ],
            "claude" => &["projects", "settings.json", "debug", "todos"],
            "cursor" | "vscode" | "windsurf" => &[
                "User/globalStorage/state.vscdb",
                "User/workspaceStorage",
                "logs",
            ],
            "copilot-cli" => &["session-state", "logs"],
            "gemini" => &["tmp", "settings.json"],
            "opencode" => &["log", "storage"],
            _ => &[],
        };
        for name in subpaths {
            let p = root.join(name);
            let e = entry(&p);
            if name.ends_with("state.vscdb") {
                signals.insert(
                    "state_db_logical_bytes".into(),
                    e.get("logical_bytes_if_file")
                        .cloned()
                        .unwrap_or(Value::Null),
                );
                signals.insert("state_db_path".into(), json!(p));
                signals.insert(
                    "state_root_identity".into(),
                    json!(crate::control_workspace::directory_identity(&root).ok()),
                );
                signals.insert(
                    "state_db_coverage".into(),
                    e.get("coverage").cloned().unwrap_or(json!("partial")),
                );
            }
            state.push(e);
        }
    }
    Ok(
        json!({"profile_id":i.id,"adapter":i.adapter,"declared_version":i.version_text,"executable_digest_matches":identity==i.executable_sha256,"executable_sha256":identity,"state_candidates":state,"numeric_signals":signals,"notices":notices,"observed_at":crate::control_store::epoch(),"coverage":Coverage::Partial,"session_ownership":"managed_protocol_bindings_and_hooks_are_separate","network_called":false,"transcripts_or_credentials_read":false,"repair_authorized":false,"limitations":["Candidate paths do not establish effective GUI data-dir overrides.","No SQLite user database opened; size alone is not corruption.","Full private-state formats and signatures remain version-specific and unsupported by default."]}),
    )
}

/// Explore only two container levels, then metadata beneath matching staging directories.
/// This is a size/count symptom, never proof that updates or migration are not active.
fn staging_metadata(root: &Path) -> Value {
    let started = Instant::now();
    let mut pending = vec![(root.to_path_buf(), 0usize, false)];
    if paths::directory(root).is_err() {
        return json!({"coverage":"partial","path":root,"bytes":null,"count":null});
    }
    let identity = crate::control_workspace::directory_identity(root).ok();
    let (mut entries, mut count, mut bytes) = (0u64, 0u64, 0u64);
    let mut complete = true;
    while let Some((p, level, in_staging)) = pending.pop() {
        if entries >= 2000 || started.elapsed() > Duration::from_millis(700) {
            complete = false;
            break;
        }
        if paths::local_existing(&p).is_err() {
            complete = false;
            continue;
        }
        let meta = match fs::symlink_metadata(&p) {
            Ok(m) => m,
            Err(_) => {
                complete = false;
                continue;
            }
        };
        if paths::is_redirect_or_placeholder(&meta) {
            complete = false;
            continue;
        }
        if meta.is_file() {
            if in_staging {
                bytes = match bytes.checked_add(meta.len()) {
                    Some(n) => n,
                    None => {
                        complete = false;
                        break;
                    }
                };
            }
            continue;
        }
        if !meta.is_dir() {
            complete = false;
            continue;
        }
        let children = match fs::read_dir(&p) {
            Ok(c) => c,
            Err(_) => {
                complete = false;
                continue;
            }
        };
        for child in children {
            entries += 1;
            if entries > 2000 || started.elapsed() > Duration::from_millis(700) {
                complete = false;
                break;
            }
            let child = match child {
                Ok(c) => c,
                Err(_) => {
                    complete = false;
                    continue;
                }
            };
            let name = child.file_name();
            let staging = name.to_str().is_some_and(|n| n.starts_with(".staging-"));
            if staging {
                count += 1;
            }
            if in_staging || staging || level < 2 {
                pending.push((child.path(), level + 1, in_staging || staging));
            }
        }
    }
    json!({"path":root,"identity":identity,"count":count,"bytes":bytes,"coverage":if complete{"complete"}else{"partial"},"duration_ms":started.elapsed().as_millis(),"physical_reclaim_bytes":null})
}
