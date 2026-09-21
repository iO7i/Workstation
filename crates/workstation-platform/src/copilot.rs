//! Opt-in native Copilot CLI wire client, derived from github/copilot-sdk public source.
//! Content-Length stdio, one owned process and one session; never connects to an
//! arbitrary port or reads account auth material. Source-reviewed, not vendor-certified.
use crate::{adapters::launch, external::Running, rpc::opaque_id, storage::Store, Error, Result};
use serde_json::{json, Value};
use std::collections::{BTreeSet, VecDeque};
use workstation_core::{
    control,
    effects::QueryKind,
    integrations::{Adapter, Integration},
};
struct Copilot {
    transport: Running,
    seq: u64,
    frames: u32,
    notifications: VecDeque<Value>,
    session: Option<String>,
    prompt_armed: bool,
    denied: u32,
}
impl Copilot {
    fn start(store: &Store, i: &Integration, cwd: &str, timeout: u32) -> Result<Self> {
        if i.adapter != Adapter::Copilot {
            return Err(Error::new("COPILOT_PROFILE_REQUIRED"));
        }
        // Protocol version must be explicitly selected and is included in the integration digest.
        // Do not negotiate an unreviewed newer wire merely because it has a greater number.
        let expected = i
            .protocol_revision
            .ok_or(Error::new("COPILOT_PROTOCOL_REVISION_REQUIRED"))?;
        if expected != 3 {
            return Err(Error::new("COPILOT_WIRE_REVISION_NOT_IMPLEMENTED"));
        }
        let mut spec = launch(
            store,
            i,
            cwd,
            &["--headless", "--no-auto-update", "--stdio"],
            timeout,
        )?;
        let auth = spec
            .env
            .remove(std::ffi::OsStr::new("GH_TOKEN"))
            .or_else(|| spec.env.remove(std::ffi::OsStr::new("GITHUB_TOKEN")));
        if let Some(token) = auth {
            spec.env.insert("COPILOT_SDK_AUTH_TOKEN".into(), token);
            spec.args
                .extend(["--auth-token-env".into(), "COPILOT_SDK_AUTH_TOKEN".into()]);
        }
        let mut c = Self {
            transport: Running::spawn(spec)?,
            seq: 0,
            frames: 0,
            notifications: VecDeque::new(),
            session: None,
            prompt_armed: false,
            denied: 0,
        };
        let hello = c.request("connect", json!({"enableGitHubTelemetryForwarding":false}))?;
        if hello.get("ok").and_then(Value::as_bool) != Some(true)
            || hello.get("protocolVersion").and_then(Value::as_u64) != Some(expected.into())
        {
            return Err(Error::new("COPILOT_PROTOCOL_MISMATCH"));
        }
        Ok(c)
    }
    fn frame(&mut self) -> Result<Value> {
        self.frames += 1;
        if self.frames > 10000 {
            return Err(Error::new("COPILOT_FRAME_BUDGET"));
        }
        self.transport.next_framed()
    }
    fn request(&mut self, method: &str, params: Value) -> Result<Value> {
        self.seq = self
            .seq
            .checked_add(1)
            .ok_or(Error::new("RPC_ID_OVERFLOW"))?;
        let id = self.seq;
        self.transport
            .send_framed(&json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}))?;
        loop {
            let msg = self.frame()?;
            if msg.get("method").is_some() {
                self.accept_notification_or_deny(&msg)?;
                continue;
            }
            if msg.get("id").and_then(Value::as_u64) != Some(id) {
                return Err(Error::new("RPC_UNCORRELATED_RESPONSE"));
            }
            if msg.get("error").is_some() {
                return Err(Error::new("COPILOT_REMOTE_REJECTED_DETAILS_SUPPRESSED"));
            }
            return msg
                .get("result")
                .cloned()
                .ok_or(Error::new("RPC_RESULT_MISSING"));
        }
    }
    fn accept_notification_or_deny(&mut self, msg: &Value) -> Result<()> {
        if let Some(id) = msg.get("id") {
            if !(id.is_u64() || id.as_str().is_some_and(|s| s.len() <= 128)) {
                return Err(Error::new("RPC_ID_INVALID"));
            }
            // We offer no filesystem, tools, LLM, shell, credentials or UI request handlers.
            self.denied += 1;
            return self.transport.send_framed(&json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":"Capability not provided by Workstation"}}));
        }
        if msg.get("method").and_then(Value::as_str) != Some("session.event") {
            return Ok(());
        }
        if msg.pointer("/params/sessionId").and_then(Value::as_str) != self.session.as_deref()
            || self.session.is_none()
        {
            return Ok(());
        }
        let e = msg
            .pointer("/params/event")
            .ok_or(Error::new("COPILOT_EVENT_SHAPE"))?;
        let kind = e
            .get("type")
            .and_then(Value::as_str)
            .ok_or(Error::new("COPILOT_EVENT_TYPE"))?;
        // Never retain message deltas, tool arguments/results, stack traces or prompts.
        let projected = match kind {
            "permission.requested" => {
                let d = e
                    .get("data")
                    .ok_or(Error::new("COPILOT_PERMISSION_SHAPE"))?;
                let req = d
                    .get("permissionRequest")
                    .ok_or(Error::new("COPILOT_PERMISSION_SHAPE"))?;
                json!({"type":kind,"request_id":opaque_id(d,"requestId")?,"kind":req.get("kind").and_then(Value::as_str).filter(|s|s.len()<80),
     "path":req.get("fileName").or_else(||req.get("path")).and_then(Value::as_str).filter(|s|s.len()<32700),
     "managed":req.get("managedApprovalRequired").map(|v|v!=&Value::Bool(false)).unwrap_or(false),"resolved":d.get("resolvedByHook").and_then(Value::as_bool)==Some(true)})
            }
            "session.error" => json!({"type":kind}),
            "session.idle" | "assistant.turn_start" | "user.message" if self.prompt_armed => {
                json!({"type":kind,"id":e.get("id").and_then(Value::as_str).filter(|s|s.len()<=256)})
            }
            "session.usage_info" => {
                let d = e.get("data").ok_or(Error::new("COPILOT_USAGE_SHAPE"))?;
                json!({"type":kind,"currentTokens":d.get("currentTokens").and_then(Value::as_u64),"tokenLimit":d.get("tokenLimit").and_then(Value::as_u64)})
            }
            _ => return Ok(()),
        };
        if self.notifications.len() >= 256 {
            return Err(Error::new("COPILOT_EVENT_QUEUE_LIMIT"));
        }
        self.notifications.push_back(projected);
        Ok(())
    }
    fn next_event(&mut self) -> Result<Value> {
        loop {
            if let Some(e) = self.notifications.pop_front() {
                return Ok(e);
            }
            let f = self.frame()?;
            self.accept_notification_or_deny(&f)?;
        }
    }
    fn require_success(&mut self, method: &str, params: Value) -> Result<()> {
        if self
            .request(method, params)?
            .get("success")
            .and_then(Value::as_bool)
            != Some(true)
        {
            return Err(Error::new("COPILOT_POLICY_NOT_ACKNOWLEDGED"));
        }
        Ok(())
    }
}
pub fn query(store: &Store, i: &Integration, kind: QueryKind) -> Result<Value> {
    let cwd = store.home.to_str().ok_or(Error::new("HOME_ENCODING"))?;
    let mut c = Copilot::start(store, i, cwd, 20)?;
    match kind {
        QueryKind::CopilotQuota => {
            let v = c.request("account.getQuota", json!({}))?;
            let mut rows = workstation_core::usage_import::copilot_quota(
                &v,
                &i.id,
                crate::control_store::epoch(),
                &i.version_text,
            )
            .map_err(Error::new)?;
            for r in &mut rows {
                r.evidence.source = format!("received_local_copilot_wire:{}", i.version_text);
                r.evidence.kind = workstation_core::control::EvidenceKind::Observed;
            }
            Ok(
                json!({"samples":rows,"coverage":"partial","account_identity_verified":false,"meter":"subscription","observation":"received_local_cli","reset_missing_means_unknown":true}),
            )
        }
        QueryKind::CopilotModels => {
            let v = c.request("models.list", json!({}))?;
            let models = v
                .get("models")
                .and_then(Value::as_array)
                .ok_or(Error::new("COPILOT_MODELS_SHAPE"))?;
            if models.len() > 200 {
                return Err(Error::new("MODEL_LIST_LIMIT"));
            }
            let mut rows = vec![];
            for m in models {
                let id = opaque_id(m, "id")?;
                let b = m.get("billing").unwrap_or(&Value::Null);
                let p = b.get("tokenPrices").unwrap_or(&Value::Null);
                rows.push(json!({"id":id,"multiplier":finite(b.get("multiplier")),"input_price":finite(p.get("inputPrice")),"output_price":finite(p.get("outputPrice")),"batch_size":p.get("batchSize").and_then(Value::as_u64),"unit":"copilot_ai_credits_not_usd","benchmark_quality":null}));
            }
            Ok(json!({"models":rows,"coverage":"partial","automatic_selection":false}))
        }
        QueryKind::VendorSessions => {
            let project = store
                .projects()?
                .into_iter()
                .find(|p| p.id == i.project_id)
                .ok_or(Error::new("PROJECT_MISSING"))?;
            let v = c.request(
                "session.list",
                json!({"filter":{"workingDirectory":project.path}}),
            )?;
            let input = v
                .get("sessions")
                .and_then(Value::as_array)
                .ok_or(Error::new("COPILOT_SESSIONS_SHAPE"))?;
            if input.len() > 200 {
                return Err(Error::new("SESSION_LIST_LIMIT"));
            }
            let mut rows = vec![];
            for s in input {
                if s.get("isRemote").and_then(Value::as_bool) == Some(true) {
                    continue;
                }
                let Some(cwd) = s
                    .pointer("/context/workingDirectory")
                    .and_then(Value::as_str)
                else {
                    continue;
                };
                let Ok(identity) =
                    crate::control_workspace::directory_identity(std::path::Path::new(cwd))
                else {
                    continue;
                };
                if identity != crate::control_workspace::directory_identity(&project.path)? {
                    continue;
                }

                rows.push(json!({"external_id":opaque_id(s,"sessionId")?,"activity":"unknown_not_proven_by_listing","start":s.get("startTime").and_then(Value::as_str).filter(|s|s.len()<=64),"modified":s.get("modifiedTime").and_then(Value::as_str).filter(|s|s.len()<=64)}));
            }
            Ok(
                json!({"sessions":rows,"coverage":"partial","scope":"project_cwd_filter","raw_content":"not_collected"}),
            )
        }
        QueryKind::ProtocolHealth => Ok(
            json!({"protocol_version":3,"connected":true,"capability":"handshake_only","provider_authentication":"not_proven"}),
        ),
        _ => Err(Error::new("COPILOT_QUERY_UNSUPPORTED")),
    }
}
fn finite(v: Option<&Value>) -> Option<f64> {
    v.and_then(Value::as_f64)
        .filter(|v| v.is_finite() && *v >= 0.0)
}
#[expect(
    clippy::too_many_arguments,
    reason = "continuation binds explicit session, permission, timeout and persistence callback inputs"
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
    let mut c = Copilot::start(store, i, cwd, timeout)?;
    let id = resume.map(str::to_owned).unwrap_or_else(crate::new_id);
    control::text(&id, 256).map_err(Error::new)?;
    c.session = Some(id.clone());
    let opts = json!({"sessionId":id,"workingDirectory":cwd,"clientName":"workstation","requestPermission":true,"requestUserInput":false,"requestElicitation":false,"streaming":true,
  "includeSubAgentStreamingEvents":false,"enableSessionTelemetry":false,"remoteSession":false,"mcpServers":{},"customAgents":[],"customAgentsLocalOnly":true,"enableConfigDiscovery":false,"enableSkills":false,"enableFileHooks":false,"enableOnDemandInstructionDiscovery":false,"hooks":false});
    let created = c.request(
        if resume.is_some() {
            "session.resume"
        } else {
            "session.create"
        },
        opts,
    )?;
    if opaque_id(&created, "sessionId")? != id {
        return Err(Error::new("RESUMED_ID_MISMATCH"));
    }
    // Reassert per-session policy on resume; do not mutate location/global policies.
    c.require_success("session.permissions.configure",json!({"sessionId":id,"approveAllToolPermissionRequests":false,"approveAllReadPermissionRequests":false,"rules":{"approved":[],"denied":[]},"paths":{"unrestricted":false,"additionalDirectories":[],"includeTempDirectory":false,"workspacePath":cwd},"urls":{"unrestricted":false,"initialAllowed":[]}}))?;
    c.require_success("session.options.update",json!({"sessionId":id,"skipCustomInstructions":true,"enableSkills":false,"enableFileHooks":false,"enableConfigDiscovery":false,"installedPlugins":[]}))?;
    c.require_success(
        "session.permissions.setRequired",
        json!({"sessionId":id,"required":true}),
    )?;
    on_session(&id, c.transport.identity().ok())?;
    c.prompt_armed = true;
    let sent = c.request("session.send", json!({"sessionId":id,"prompt":prompt}))?;
    let message_id = opaque_id(&sent, "messageId")?;
    let mut seen_turn = false;
    let mut permission_ids = BTreeSet::new();
    let mut context_usage = None;
    loop {
        let event = c.next_event()?;
        match event.get("type").and_then(Value::as_str) {
            Some("user.message")
                if event.get("id").and_then(Value::as_str) == Some(message_id.as_str()) =>
            {
                seen_turn = true
            }
            Some("session.error") => {
                return Err(Error::new("COPILOT_TURN_ERROR_DETAILS_SUPPRESSED"))
            }
            Some("permission.requested") => {
                if event.get("resolved").and_then(Value::as_bool) == Some(true) {
                    return Err(Error::new("UNEXPECTED_HOOK_PERMISSION_RESOLUTION"));
                }
                let request = opaque_id(&event, "request_id")?;
                if !permission_ids.insert(request.clone()) || permission_ids.len() > 128 {
                    return Err(Error::new("PERMISSION_REPEAT_OR_LIMIT"));
                }
                let kind = event.get("kind").and_then(Value::as_str);
                let path = event.get("path").and_then(Value::as_str);
                let allowed = event.get("managed").and_then(Value::as_bool) == Some(false)
                    && matches!(kind, Some("read" | "write"))
                    && (kind == Some("read") || permission == "allow_once")
                    && path.is_some_and(|p| {
                        crate::rpc::scoped_file_path(p, std::path::Path::new(cwd))
                    });
                if !allowed {
                    c.denied += 1;
                }
                let decision = if allowed {
                    json!({"kind":"approve-once","approvedInteractively":false})
                } else {
                    json!({"kind":"user-not-available"})
                };
                c.require_success(
                    "session.permissions.handlePendingPermissionRequest",
                    json!({"sessionId":id,"requestId":request,"result":decision}),
                )?;
            }
            Some("session.usage_info") => {
                context_usage = Some(
                    json!({"current_tokens":event.get("currentTokens"),"token_limit":event.get("tokenLimit"),"source":"received_session_event","subscription_quota":null}),
                )
            }
            Some("session.idle") if seen_turn => break,
            _ => {}
        }
    }
    // Disconnect runtime state; native conversation remains vendor-owned and resumable.
    c.request("session.destroy", json!({"sessionId":id}))?;
    Ok(
        json!({"external_session_id":id,"message_id":message_id,"completed":true,"semantic_work_complete":false,"test_results":"not_inferred","permissions_denied":c.denied,"context_usage":context_usage,"scope":"client_permission_policy_not_os_sandbox","assistant_text":"not_retained"}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nonfinite_price_unknown() {
        assert_eq!(finite(None), None);
        assert_eq!(finite(Some(&json!(-1))), None);
    }
    #[test]
    fn quota_and_dollars_not_mixed() {
        let q = json!({"quotaSnapshots":{"chat":{"entitlementRequests":100,"usedRequests":1,"resetDate":"2026-10-01T00:00:00Z"}}});
        let rows =
            workstation_core::usage_import::copilot_quota(&q, "account", 100, "fixture").unwrap();
        assert_eq!(
            rows[0].meter,
            workstation_core::economics::Meter::Subscription
        );
    }
}
