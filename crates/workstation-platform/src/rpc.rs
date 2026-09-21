//! Request-correlated JSON-line protocol driver. Unknown server requests fail closed.
use crate::{external::Running, Error, Result};
use serde_json::{json, Value};
use std::collections::VecDeque;
use workstation_core::control;
pub struct Rpc {
    pending_turns: VecDeque<Value>,
    pub transport: Running,
    pub usage_summary: Option<Value>,
    seq: u64,
    frames: u32,
    pub notifications: u32,
    pub denied_permissions: u32,
    pub allow_once: bool,
    pub permission_session: Option<String>,
    pub permission_workspace: Option<std::path::PathBuf>,
}
impl Rpc {
    pub fn new(transport: Running, allow_once: bool) -> Self {
        Self {
            pending_turns: VecDeque::new(),
            transport,
            usage_summary: None,
            seq: 0,
            frames: 0,
            notifications: 0,
            denied_permissions: 0,
            allow_once,
            permission_session: None,
            permission_workspace: None,
        }
    }
    pub fn notify(&self, method: &str, params: Value) -> Result<()> {
        self.transport
            .send_json(&json!({"jsonrpc":"2.0","method":method,"params":params}))
    }
    pub fn request(&mut self, method: &str, params: Value) -> Result<Value> {
        crate::durable_runtime::request_started(method)?;
        self.seq += 1;
        let id = self.seq;
        self.transport
            .send_json(&json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}))?;
        loop {
            let v = self.frame()?;
            if v.get("method").is_some() {
                // A server may complete a fast turn before replying to turn/start. Preserve
                // only bounded correlation/status metadata rather than losing the event.
                if v.get("id").is_none() {
                    if let Some(terminal) = terminal_turn(&v)? {
                        if self.pending_turns.len() >= 32 {
                            return Err(Error::new("RPC_TERMINAL_QUEUE_LIMIT"));
                        }
                        self.pending_turns.push_back(terminal);
                    }
                }
                self.respond_to_server(&v)?;
                continue;
            }
            if v.get("id").and_then(Value::as_u64) != Some(id) {
                return Err(Error::new("RPC_UNCORRELATED_RESPONSE"));
            }
            if v.get("error").is_some() {
                return Err(Error::new("RPC_REMOTE_REJECTED_DETAILS_SUPPRESSED"));
            }
            let result = v
                .get("result")
                .cloned()
                .ok_or(Error::new("RPC_RESULT_MISSING"))?;
            crate::durable_runtime::response_received(method, &result)?;
            return Ok(result);
        }
    }
    pub fn frame(&mut self) -> Result<Value> {
        self.frames += 1;
        if self.frames > 10000 {
            return Err(Error::new("RPC_FRAME_COUNT_LIMIT"));
        }
        loop {
            if let Some(cancel) = crate::durable_runtime::tick()? {
                self.transport.send_json(&cancel)?;
            }
            let v = self.transport.next_json()?;
            if v.get("id").and_then(Value::as_str) == Some("workstation-cancel") {
                continue;
            }
            crate::durable_runtime::observe(&v)?;
            return Ok(v);
        }
    }
    pub fn respond_to_server(&mut self, v: &Value) -> Result<()> {
        let method = v
            .get("method")
            .and_then(Value::as_str)
            .ok_or(Error::new("RPC_METHOD_INVALID"))?;
        let Some(id) = v.get("id") else {
            self.notifications += 1;
            if method == "session/update"
                && v.pointer("/params/sessionId").and_then(Value::as_str)
                    == self.permission_session.as_deref()
                && self.permission_session.is_some()
                && v.pointer("/params/update/sessionUpdate")
                    .and_then(Value::as_str)
                    == Some("usage_update")
            {
                let u = v
                    .pointer("/params/update")
                    .ok_or(Error::new("ACP_USAGE_SHAPE"))?;
                self.usage_summary = Some(
                    json!({"source":"reported_acp_usage_update","used":u.get("used").and_then(Value::as_u64),"size":u.get("size").and_then(Value::as_u64),"unit":"context_tokens_where_reported","subscription_quota":null,"account_verified":false}),
                );
            }
            return Ok(());
        };
        if !(id.is_number() || id.as_str().is_some_and(|s| s.len() <= 128)) {
            return Err(Error::new("RPC_ID_INVALID"));
        }
        if method == "session/request_permission" {
            let in_scope = v.pointer("/params/sessionId").and_then(Value::as_str)
                == self.permission_session.as_deref()
                && self.permission_session.is_some();
            let option = if self.allow_once
                && in_scope
                && self
                    .permission_workspace
                    .as_deref()
                    .is_some_and(|root| permission_locations_safe(v, root))
            {
                v.pointer("/params/options")
                    .and_then(Value::as_array)
                    .and_then(|a| {
                        a.iter()
                            .find(|o| o.get("kind").and_then(Value::as_str) == Some("allow_once"))
                    })
                    .and_then(|o| o.get("optionId"))
                    .and_then(Value::as_str)
                    .filter(|s| s.len() <= 128)
            } else {
                None
            };
            let outcome = match option {
                Some(o) => json!({"outcome":"selected","optionId":o}),
                None => {
                    self.denied_permissions += 1;
                    json!({"outcome":"cancelled"})
                }
            };
            return self
                .transport
                .send_json(&json!({"jsonrpc":"2.0","id":id,"result":{"outcome":outcome}}));
        }
        // Codex approvals have vendor-specific granular schemas. Never fabricate an accept response.
        // Continuation runs with vendor sandbox and approvalPolicy=never; unsupported requests deny.
        self.denied_permissions += 1;
        self.transport.send_json(&json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":"Client capability not granted"}}))
    }
    pub fn wait_codex_turn(&mut self, thread: &str, turn: &str) -> Result<Value> {
        loop {
            let v = match self.pending_turns.pop_front() {
                Some(v) => v,
                None => self.frame()?,
            };
            if v.get("method").and_then(Value::as_str) == Some("turn/completed") {
                if v.pointer("/params/threadId").and_then(Value::as_str) != Some(thread)
                    || v.pointer("/params/turn/id").and_then(Value::as_str) != Some(turn)
                {
                    continue;
                }
                let status = v
                    .pointer("/params/turn/status")
                    .and_then(Value::as_str)
                    .ok_or(Error::new("TURN_STATUS_MISSING"))?;
                if !["completed", "interrupted", "failed"].contains(&status) {
                    return Err(Error::new("TURN_STATUS_UNKNOWN"));
                }
                return Ok(
                    json!({"thread_id":thread,"turn_id":turn,"turn_status":status,"notifications_discarded":self.notifications,"permissions_denied":self.denied_permissions,"assistant_text":"not_retained"}),
                );
            }
            self.respond_to_server(&v)?;
        }
    }
}
fn terminal_turn(v: &Value) -> Result<Option<Value>> {
    if v.get("method").and_then(Value::as_str) != Some("turn/completed") {
        return Ok(None);
    }
    let params = v.get("params").ok_or(Error::new("TURN_STATUS_MISSING"))?;
    let thread = opaque_id(params, "threadId")?;
    let turn = params
        .get("turn")
        .ok_or(Error::new("TURN_STATUS_MISSING"))?;
    let id = opaque_id(turn, "id")?;
    let status = turn
        .get("status")
        .and_then(Value::as_str)
        .ok_or(Error::new("TURN_STATUS_MISSING"))?;
    if !["completed", "interrupted", "failed"].contains(&status) {
        return Err(Error::new("TURN_STATUS_UNKNOWN"));
    }
    Ok(Some(
        json!({"method":"turn/completed","params":{"threadId":thread,"turn":{"id":id,"status":status}}}),
    ))
}

pub fn opaque_id(value: &Value, field: &str) -> Result<String> {
    let id = value
        .get(field)
        .and_then(Value::as_str)
        .ok_or(Error::new("EXTERNAL_ID_MISSING"))?;
    control::text(id, 256).map_err(Error::new)?;
    if id.trim().is_empty() || id.starts_with('-') {
        return Err(Error::new("EXTERNAL_ID_INVALID"));
    }
    Ok(id.into())
}

/// ACP metadata is not a security sandbox. Only explicitly located file read/edit/search
/// requests inside the approved worktree can be auto-acknowledged. Exec/fetch/delete,
/// absent locations and sensitive control/config files always require another route.
fn permission_locations_safe(request: &Value, root: &std::path::Path) -> bool {
    let Some(kind) = request
        .pointer("/params/toolCall/kind")
        .and_then(Value::as_str)
    else {
        return false;
    };
    if !["read", "edit", "search"].contains(&kind) {
        return false;
    }
    let Some(locations) = request
        .pointer("/params/toolCall/locations")
        .and_then(Value::as_array)
    else {
        return false;
    };
    if locations.is_empty() || locations.len() > 64 {
        return false;
    }
    if crate::paths::directory(root).is_err() {
        return false;
    }
    // Keep the validated normal drive spelling. canonicalize() yields a \\?\ path
    // on Windows, which local_existing intentionally refuses as an input path.
    locations.iter().all(|loc| {
        loc.get("path")
            .and_then(Value::as_str)
            .is_some_and(|p| scoped_file_path(p, root))
    })
}

/// A policy precheck, not filesystem isolation. No shell, URL, hidden state or secret paths.
pub(crate) fn scoped_file_path(p: &str, root: &std::path::Path) -> bool {
    if workstation_core::integrations::absolute_path(p).is_err() {
        return false;
    }
    let path = std::path::Path::new(p);
    if crate::paths::validate_lexical(path).is_err() {
        return false;
    }
    if path.components().any(|c| {
        let n = c.as_os_str().to_string_lossy().to_ascii_lowercase();
        n == ".git"
            || n.starts_with(".env")
            || [
                ".ssh",
                ".aws",
                ".codex",
                ".claude",
                ".cursor",
                ".workstation",
                ".vscode",
                ".npmrc",
                "agents.md",
                "claude.md",
            ]
            .contains(&n.as_str())
            || n.ends_with(".pem")
            || n.ends_with(".key")
    }) {
        return false;
    }
    let target = match std::fs::symlink_metadata(path) {
        Ok(m) if crate::paths::is_redirect_or_placeholder(&m) => return false,
        Ok(_) => path,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => match path.parent() {
            Some(parent) => parent,
            None => return false,
        },
        Err(_) => return false,
    };
    if crate::paths::directory(root).is_err() || crate::paths::local_existing(target).is_err() {
        return false;
    }
    match (std::fs::canonicalize(root), std::fs::canonicalize(target)) {
        (Ok(a), Ok(b)) => b.starts_with(a),
        _ => false,
    }
}
#[cfg(test)]
mod permission_tests {
    use super::*;
    #[test]
    fn terminal_notification_projection_discards_text_and_errors() {
        let v = json!({"method":"turn/completed","params":{"threadId":"thread-1","turn":{"id":"turn-1","status":"completed","items":[{"text":"PRIVATE_CONTENT"}],"error":{"message":"PRIVATE_CONTENT"}}}});
        let projected = terminal_turn(&v).unwrap().unwrap();
        assert_eq!(
            projected.pointer("/params/turn/id").and_then(Value::as_str),
            Some("turn-1")
        );
        assert!(!projected.to_string().contains("PRIVATE_CONTENT"));
    }
    #[test]
    fn unrelated_notification_not_retained() {
        assert!(terminal_turn(
            &json!({"method":"item/agentMessage/delta","params":{"delta":"text"}})
        )
        .unwrap()
        .is_none());
    }
    #[test]
    fn no_exec_permission_by_label() {
        let v = json!({"params":{"toolCall":{"kind":"execute","locations":[{"path":"/tmp"}]}}});
        assert!(!permission_locations_safe(&v, std::path::Path::new("/tmp")));
    }
    #[test]
    fn ordinary_existing_file_is_in_scope() {
        let t = tempfile::tempdir().unwrap();
        let p = t.path().join("ordinary.txt");
        std::fs::write(&p, b"x").unwrap();
        assert!(scoped_file_path(p.to_str().unwrap(), t.path()));
    }
    #[test]
    fn new_file_under_existing_parent_is_in_scope() {
        let t = tempfile::tempdir().unwrap();
        assert!(scoped_file_path(
            t.path().join("new.txt").to_str().unwrap(),
            t.path()
        ));
    }
    #[test]
    fn outside_file_is_denied() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        assert!(!scoped_file_path(
            b.path().join("new.txt").to_str().unwrap(),
            a.path()
        ));
    }
    #[cfg(unix)]
    #[test]
    fn dangling_link_is_denied() {
        let a = tempfile::tempdir().unwrap();
        let p = a.path().join("new.txt");
        std::os::unix::fs::symlink("/definitely-missing/secret", &p).unwrap();
        assert!(!scoped_file_path(p.to_str().unwrap(), a.path()));
    }
    #[cfg(windows)]
    #[test]
    fn permission_accepts_normal_windows_root_without_verbatim_roundtrip() {
        let a = tempfile::tempdir().unwrap();
        let p = a.path().join("ordinary.txt");
        std::fs::write(&p, b"x").unwrap();
        let v = json!({"params":{"toolCall":{"kind":"edit","locations":[{"path":p}]}}});
        assert!(permission_locations_safe(&v, a.path()));
    }
    #[cfg(windows)]
    #[test]
    fn new_ads_path_is_denied() {
        let a = tempfile::tempdir().unwrap();
        let p = a.path().join("missing.txt:stream");
        assert!(!scoped_file_path(p.to_str().unwrap(), a.path()));
    }
    #[test]
    fn omitted_location_denied() {
        let v = json!({"params":{"toolCall":{"kind":"edit"}}});
        assert!(!permission_locations_safe(&v, std::path::Path::new("/tmp")));
    }
}
