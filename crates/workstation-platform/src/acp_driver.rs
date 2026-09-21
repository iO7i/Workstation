//! Shared protocol mechanics, explicit vendor launch contracts. No guessed parity.
use crate::{
    adapters::launch,
    external::Running,
    rpc::{opaque_id, Rpc},
    storage::Store,
    Error, Result,
};
use serde_json::{json, Value};
use workstation_core::{
    effects::QueryKind,
    integrations::{Adapter, Integration},
};
fn arguments(a: Adapter) -> Result<&'static [&'static str]> {
    match a {
        Adapter::CursorAcp | Adapter::OpenCode => Ok(&["acp"]),
        Adapter::GrokAcp => Ok(&["--no-auto-update", "agent", "stdio"]),
        Adapter::GeminiAcp => Ok(&["--acp"]),
        Adapter::Cline => Ok(&["--acp", "--auto-approve", "false"]),
        _ => Err(Error::new("ACP_ADAPTER_UNSUPPORTED")),
    }
}
fn connect(
    store: &Store,
    i: &Integration,
    cwd: &str,
    timeout: u32,
    allow: bool,
) -> Result<(Rpc, Value)> {
    if i.protocol_revision.is_some_and(|x| x != 1) {
        return Err(Error::new("ACP_PROTOCOL_REVISION_UNSUPPORTED"));
    }
    let mut rpc = Rpc::new(
        Running::spawn(launch(store, i, cwd, arguments(i.adapter)?, timeout)?)?,
        allow,
    );
    let hello=rpc.request("initialize",json!({"protocolVersion":1,"clientCapabilities":{"fs":{"readTextFile":false,"writeTextFile":false},"terminal":false},"clientInfo":{"name":"workstation","version":env!("CARGO_PKG_VERSION")}}))?;
    if hello.get("protocolVersion").and_then(Value::as_u64) != Some(1) {
        return Err(Error::new("ACP_VERSION_UNSUPPORTED"));
    }
    Ok((rpc, hello))
}
fn advertised_auth_methods(hello: &Value) -> Vec<String> {
    hello
        .get("authMethods")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|row| row.get("id").and_then(Value::as_str))
                .filter(|id| id.len() <= 128 && !id.chars().any(char::is_control))
                .take(16)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}
fn authenticate(rpc: &mut Rpc, hello: &Value, i: &Integration) -> Result<()> {
    if let Some(method) = &i.auth_method {
        if !hello
            .get("authMethods")
            .and_then(Value::as_array)
            .is_some_and(|v| {
                v.iter()
                    .any(|m| m.get("id").and_then(Value::as_str) == Some(method.as_str()))
            })
        {
            return Err(Error::new("ACP_AUTH_METHOD_NOT_ADVERTISED"));
        }
        rpc.request(
            "authenticate",
            json!({"methodId":method,"_meta":{"headless":true}}),
        )?;
    }
    // With no explicit method we rely only on the vendor's previously established
    // local auth. An auth-required response fails; no browser/cookie/login fallback.
    Ok(())
}
pub fn query(store: &Store, i: &Integration, kind: QueryKind) -> Result<Value> {
    let p = store
        .projects()?
        .into_iter()
        .find(|p| p.id == i.project_id)
        .ok_or(Error::new("PROJECT_MISSING"))?;
    let cwd = p
        .path
        .to_str()
        .ok_or(Error::new("WORKSPACE_PATH_ENCODING"))?;
    let (mut rpc, hello) = connect(store, i, cwd, 15, false)?;
    if kind == QueryKind::ProtocolHealth {
        let caps = hello.get("agentCapabilities").unwrap_or(&Value::Null);
        return Ok(
            json!({"protocol_version":1,"connected":true,"load_session":caps.get("loadSession").and_then(Value::as_bool).unwrap_or(false),"list_sessions":caps.pointer("/sessionCapabilities/list").is_some_and(Value::is_object),"resume_without_replay":caps.pointer("/sessionCapabilities/resume").is_some_and(Value::is_object),"auth_method_ids":advertised_auth_methods(&hello),"provider_authentication":"not_proven","coverage":"partial"}),
        );
    }
    if kind != QueryKind::VendorSessions {
        return Err(Error::new("ACP_QUERY_UNSUPPORTED"));
    }
    if !hello
        .pointer("/agentCapabilities/sessionCapabilities/list")
        .is_some_and(Value::is_object)
    {
        return Err(Error::new("ACP_LIST_NOT_ADVERTISED"));
    }
    authenticate(&mut rpc, &hello, i)?;
    let v = rpc.request("session/list", json!({"cwd":cwd}))?;
    let rows = v
        .get("sessions")
        .and_then(Value::as_array)
        .ok_or(Error::new("ACP_SESSION_LIST_SHAPE"))?;
    if rows.len() > 200 {
        return Err(Error::new("ACP_SESSION_LIST_LIMIT"));
    }
    let mut out = vec![];
    for row in rows {
        let path = row
            .get("cwd")
            .and_then(Value::as_str)
            .ok_or(Error::new("ACP_SESSION_CWD_MISSING"))?;
        if !same_directory(cwd, path) {
            continue;
        }
        out.push(json!({"external_id":opaque_id(row,"sessionId")?,"cwd":path,"updated_at":row.get("updatedAt").and_then(Value::as_str).filter(|s|s.len()<=64),"activity":"unknown_not_inferred_from_list","coverage":"partial"}));
    }
    Ok(
        json!({"sessions":out,"more_available":v.get("nextCursor").is_some_and(|x|!x.is_null()),"content":"titles_history_and_meta_omitted","coverage":"partial"}),
    )
}
fn same_directory(a: &str, b: &str) -> bool {
    match (
        crate::external::path_identity(std::path::Path::new(a)),
        crate::external::path_identity(std::path::Path::new(b)),
    ) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}
#[expect(
    clippy::too_many_arguments,
    reason = "ACP continuation binds explicit session, permission, timeout and persistence callback inputs"
)]
pub fn continue_once<F: FnMut(&str, Option<(u32, u64)>) -> Result<()>>(
    store: &Store,
    i: &Integration,
    cwd: &str,
    prompt: &str,
    resume: Option<&str>,
    permission: &str,
    timeout: u32,
    mut on_session: F,
) -> Result<Value> {
    if i.adapter == Adapter::GeminiAcp && permission == "read_only" {
        return Err(Error::new("GEMINI_READONLY_MODE_NOT_RELIABLY_SUPPORTED"));
    }
    let (mut rpc, hello) = connect(store, i, cwd, timeout, permission == "allow_once")?;
    authenticate(&mut rpc, &hello, i)?;
    let mut params = json!({"cwd":cwd,"mcpServers":[]});
    if hello
        .pointer("/agentCapabilities/sessionCapabilities/additionalDirectories")
        .is_some_and(Value::is_object)
    {
        params["additionalDirectories"] = json!([]);
    }
    let (sid, response, mode) = if let Some(sid) = resume {
        store.assert_resume_scope(&i.project_id, i.adapter.id(), sid, cwd)?;
        params["sessionId"] = json!(sid);
        // Prefer the standard no-transcript replay method only when advertised.
        let method = if hello
            .pointer("/agentCapabilities/sessionCapabilities/resume")
            .is_some_and(Value::is_object)
        {
            "session/resume"
        } else if hello
            .pointer("/agentCapabilities/loadSession")
            .and_then(Value::as_bool)
            == Some(true)
        {
            "session/load"
        } else {
            return Err(Error::new("ACP_RESUME_NOT_ADVERTISED"));
        };
        let response = rpc.request(method, params)?;
        (sid.to_owned(), response, method)
    } else {
        let r = rpc.request("session/new", params)?;
        (opaque_id(&r, "sessionId")?, r, "session/new")
    };
    rpc.permission_session = Some(sid.clone());
    rpc.permission_workspace = Some(cwd.into());
    if permission == "read_only" {
        // Gemini describes its plan mode as incomplete. Do not use a label as proof
        // of read-only enforcement; require an explicitly approved write-capable plan instead.
        let modes = response
            .pointer("/modes/availableModes")
            .and_then(Value::as_array)
            .ok_or(Error::new("ACP_READONLY_MODE_NOT_ADVERTISED"))?;
        let plan = modes
            .iter()
            .find_map(|m| m.get("id").and_then(Value::as_str).filter(|v| *v == "plan"));
        let Some(plan) = plan else {
            return Err(Error::new("ACP_PLAN_MODE_NOT_AVAILABLE"));
        };
        rpc.request("session/set_mode", json!({"sessionId":sid,"modeId":plan}))?;
    }
    on_session(&sid, rpc.transport.identity().ok())?;
    let v = rpc.request(
        "session/prompt",
        json!({"sessionId":sid,"prompt":[{"type":"text","text":prompt}]}),
    )?;
    let reason = v
        .get("stopReason")
        .and_then(Value::as_str)
        .ok_or(Error::new("ACP_STOP_REASON_MISSING"))?;
    if ![
        "end_turn",
        "max_tokens",
        "max_turn_requests",
        "refusal",
        "cancelled",
    ]
    .contains(&reason)
    {
        return Err(Error::new("ACP_STOP_REASON_UNKNOWN"));
    }
    if hello
        .pointer("/agentCapabilities/sessionCapabilities/close")
        .is_some_and(Value::is_object)
    {
        rpc.request("session/close", json!({"sessionId":sid}))?;
    }
    Ok(
        json!({"external_session_id":sid,"method":mode,"stop_reason":reason,"completed":reason=="end_turn","permissions_denied":rpc.denied_permissions,"assistant_text":"not_retained","raw_transcript_imported":false,"read_only_claim":"provider_mode_not_os_sandbox","usage":rpc.usage_summary.clone()}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gemini_not_cmd_interactive() {
        assert_eq!(arguments(Adapter::GeminiAcp).unwrap(), &["--acp"]);
    }
    #[test]
    fn cline_auto_approval_explicitly_off() {
        assert_eq!(
            arguments(Adapter::Cline).unwrap(),
            &["--acp", "--auto-approve", "false"]
        );
    }
    #[test]
    fn windsurf_not_fabricated() {
        assert!(arguments(Adapter::Windsurf).is_err());
    }
    #[test]
    fn auth_method_ids_are_bounded_metadata_only() {
        let hello = json!({"authMethods":[{"id":"cached_token"},{"id":"xai.api_key"},{"id":"bad\nmethod"}]});
        assert_eq!(
            advertised_auth_methods(&hello),
            vec!["cached_token".to_owned(), "xai.api_key".to_owned()]
        );
    }
}
