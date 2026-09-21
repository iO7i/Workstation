//! Optional specialist adapters, never a generic installer or shell launcher.
use crate::{storage::Store, Error, Result};
use serde_json::{json, Value};
use std::{
    ffi::OsString,
    fs::{self, OpenOptions},
    io::{Read, Write},
    time::Duration,
};
use workstation_core::integrations::{Adapter, Integration};
/// Own immutable empty config is an input, not permission to load project hooks.
pub(crate) fn empty_worktrunk_config(store: &Store) -> Result<std::path::PathBuf> {
    let p = store.home.join("worktrunk-empty.toml");
    let bytes = b"# Workstation-owned no hooks\n";
    match OpenOptions::new().write(true).create_new(true).open(&p) {
        Ok(mut f) => {
            f.write_all(bytes)
                .map_err(|_| Error::new("WORKTRUNK_CONFIG_WRITE"))?;
            f.sync_all()
                .map_err(|_| Error::new("WORKTRUNK_CONFIG_SYNC"))?;
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(_) => return Err(Error::new("WORKTRUNK_CONFIG_CREATE")),
    }
    crate::paths::regular_file(&p)?;
    let mut content = Vec::new();
    fs::File::open(&p)
        .map_err(|_| Error::new("WORKTRUNK_CONFIG_READ"))?
        .take(bytes.len() as u64 + 1)
        .read_to_end(&mut content)
        .map_err(|_| Error::new("WORKTRUNK_CONFIG_READ"))?;
    if content != bytes {
        return Err(Error::new("WORKTRUNK_CONFIG_CHANGED"));
    }
    Ok(p)
}
pub fn worktrunk_list(store: &Store, i: &Integration) -> Result<Value> {
    if i.adapter != Adapter::Worktrunk {
        return Err(Error::new("WORKTRUNK_PROFILE_REQUIRED"));
    }
    let p = store
        .projects()?
        .into_iter()
        .find(|p| p.id == i.project_id)
        .ok_or(Error::new("PROJECT_MISSING"))?;
    // Project configuration can define shell-backed custom columns/aliases. Rather than
    // guessing merge semantics, refuse it for this narrowly non-executing inventory.
    for name in [".config/wt.toml", ".worktrunk.toml", "wt.toml"] {
        if crate::paths::entry_exists(&p.path.join(name))? {
            return Err(Error::new(
                "WORKTRUNK_PROJECT_CONFIG_REQUIRES_SEPARATE_REVIEW",
            ));
        }
    }
    let cfg = empty_worktrunk_config(store)?;
    let args: Vec<OsString> = vec![
        "--config".into(),
        cfg.as_os_str().into(),
        "--config-set".into(),
        "list.full = false".into(),
        "--config-set".into(),
        "list.json-schema = 2".into(),
        "list".into(),
        "--format=json".into(),
        "--no-progressive".into(),
    ];
    let mut env =
        crate::external::scoped_environment(&store.home, &[], &std::collections::BTreeMap::new())?;
    env.insert(
        "PATH".into(),
        p.git_executable
            .parent()
            .ok_or(Error::new("GIT_PARENT"))?
            .as_os_str()
            .into(),
    );
    #[cfg(windows)]
    let null = "NUL";
    #[cfg(not(windows))]
    let null = "/dev/null";
    for (k, v) in [
        ("GIT_CONFIG_NOSYSTEM", "1"),
        ("GIT_CONFIG_GLOBAL", null),
        ("GIT_CONFIG_SYSTEM", null),
        ("GIT_CONFIG_COUNT", "3"),
        ("GIT_CONFIG_KEY_0", "core.hooksPath"),
        ("GIT_CONFIG_VALUE_0", ""),
        ("GIT_CONFIG_KEY_1", "protocol.allow"),
        ("GIT_CONFIG_VALUE_1", "never"),
        ("GIT_CONFIG_KEY_2", "core.fsmonitor"),
        ("GIT_CONFIG_VALUE_2", "false"),
    ] {
        env.insert(k.into(), v.into());
    }
    env.insert("GIT_OPTIONAL_LOCKS".into(), "0".into());
    env.insert("GIT_TERMINAL_PROMPT".into(), "0".into());
    env.insert("GIT_NO_LAZY_FETCH".into(), "1".into());
    let out = crate::external::one_shot(
        crate::external::Launch {
            executable: i.executable.clone().into(),
            expected_sha256: i.executable_sha256.clone(),
            cwd: p.path.clone(),
            args,
            env,
            timeout: Duration::from_secs(10),
            output_limit: 1024 * 1024,
        },
        None,
    )?;
    if out.exit_code != 0 {
        return Err(Error::new("WORKTRUNK_LIST_NONZERO"));
    }
    let v: Value =
        serde_json::from_slice(&out.bytes.0).map_err(|_| Error::new("WORKTRUNK_JSON"))?;
    project_list(&v)
}
/// Version-gated schema2 projection. Descriptions, remote URLs, messages, summaries and
/// hook text never reach the report. No display.state field authorizes disposal.
pub fn project_list(v: &Value) -> Result<Value> {
    if v.get("schema").and_then(Value::as_u64) != Some(2) {
        return Err(Error::new("WORKTRUNK_SCHEMA_UNSUPPORTED"));
    }
    if v.pointer("/collected/ci").and_then(Value::as_bool) != Some(false)
        || v.pointer("/collected/summary").and_then(Value::as_bool) != Some(false)
    {
        return Err(Error::new("WORKTRUNK_UNEXPECTED_REMOTE_COLLECTION"));
    }
    let items = v
        .get("items")
        .and_then(Value::as_array)
        .ok_or(Error::new("WORKTRUNK_ITEMS_MISSING"))?;
    if items.len() > 256 {
        return Err(Error::new("WORKTRUNK_ROW_LIMIT"));
    }
    let mut rows = vec![];
    for item in items {
        let Some(w) = item.get("worktree") else {
            continue;
        };
        let path = w
            .get("path")
            .and_then(Value::as_str)
            .ok_or(Error::new("WORKTRUNK_PATH_MISSING"))?;
        workstation_core::integrations::absolute_path(path).map_err(Error::new)?;
        let branch = item.get("branch").and_then(Value::as_str);
        if let Some(b) = branch {
            workstation_core::control::text(b, 1024).map_err(Error::new)?;
        }
        let head = item.pointer("/head/sha").and_then(Value::as_str);
        if head.is_some_and(|h| {
            !matches!(h.len(), 40 | 64) || !h.bytes().all(|x| x.is_ascii_hexdigit())
        }) {
            return Err(Error::new("WORKTRUNK_HEAD_INVALID"));
        }
        rows.push(json!({"path":path,"branch":branch,"head":head,"main":w.get("main").and_then(Value::as_bool),"locked":w.get("locked").is_some(),"prunable":w.get("prunable").is_some(),"cleanup":"protected","source":"worktrunk_schema2_not_authority"}));
    }
    Ok(
        json!({"worktrees":rows,"coverage":"partial","remote_requested":false,"backend":"worktrunk","cleanup_authorized":false}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_remote_collection() {
        assert!(project_list(
            &json!({"schema":2,"collected":{"ci":true,"summary":false},"items":[]})
        )
        .is_err());
    }
    #[test]
    fn rejects_bare_array_instead_of_guessing_version() {
        assert!(project_list(&json!([])).is_err());
    }
    #[test]
    fn ignores_commit_subject_and_summaries() {
        let v = json!({"schema":2,"collected":{"ci":false,"summary":false},"items":[{"branch":"main","head":{"sha":"a".repeat(40),"subject":"SECRET_CANARY"},"worktree":{"path":"D:/fixture"}}]});
        let x = project_list(&v).unwrap();
        assert!(!x.to_string().contains("SECRET_CANARY"));
        assert_eq!(x["worktrees"][0]["cleanup"], "protected");
    }
}
