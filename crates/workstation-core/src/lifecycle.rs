//! Metadata-only import of documented lifecycle shapes. Never reads transcript paths.
use crate::control::{self, Check};
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LifecycleEvent {
    pub source_id: String,
    pub project_id: String,
    pub workspace_id: Option<String>,
    pub agent: String,
    pub external_session_id: String,
    pub event: String,
    pub observed_at: i64,
    pub source_revision: String,
    pub evidence: String,
}
pub fn normalize(
    format: &str,
    payload: &Value,
    source_id: &str,
    project: &str,
    workspace: Option<String>,
    at: i64,
    version: &str,
) -> Check<LifecycleEvent> {
    control::id(source_id)?;
    control::id(project)?;
    control::timestamp(at)?;
    control::text(version, 128)?;
    if let Some(w) = &workspace {
        control::id(w)?;
    }
    let (agent, id, event) = match format {
        "claude-hook" => {
            let id = payload
                .get("session_id")
                .and_then(Value::as_str)
                .ok_or("HOOK_SESSION_ID_MISSING")?;
            let kind = payload
                .get("hook_event_name")
                .and_then(Value::as_str)
                .ok_or("HOOK_KIND_MISSING")?;
            let event = match kind {
                "SessionStart" => "started",
                "SessionEnd" => "ended",
                "Stop" => "turn_completed",
                "PreCompact" => "compaction_boundary",
                _ => return Err("HOOK_KIND_NOT_ALLOWLISTED"),
            };
            ("claude", id, event)
        }
        "gemini-hook" => {
            let id = payload
                .get("session_id")
                .and_then(Value::as_str)
                .ok_or("HOOK_SESSION_ID_MISSING")?;
            let kind = payload
                .get("hook_event_name")
                .and_then(Value::as_str)
                .ok_or("HOOK_KIND_MISSING")?;
            let event = match kind {
                "SessionStart" => "started",
                "SessionEnd" => "ended",
                "BeforeAgent" | "BeforeTool" | "AfterTool" => "activity",
                "AfterAgent" => "turn_completed",
                "PreCompress" => "compaction_boundary",
                _ => return Err("HOOK_KIND_NOT_ALLOWLISTED"),
            };
            ("gemini", id, event)
        }
        "copilot-session-event" => {
            if payload.get("method").and_then(Value::as_str) != Some("session.event") {
                return Err("NOTIFICATION_NOT_ALLOWLISTED");
            }
            let params = payload.get("params").ok_or("RPC_PARAMS_MISSING")?;
            let id = params
                .get("sessionId")
                .and_then(Value::as_str)
                .ok_or("SESSION_ID_MISSING")?;
            let kind = params
                .pointer("/event/type")
                .and_then(Value::as_str)
                .ok_or("HOOK_KIND_MISSING")?;
            let event = match kind {
                "session.start" | "session.resume" => "started",
                "session.idle" => "turn_completed",
                "session.error" => "activity",
                "assistant.turn_start" | "assistant.turn_end" | "session.usage_info" => "activity",
                _ => return Err("HOOK_KIND_NOT_ALLOWLISTED"),
            };
            ("copilot", id, event)
        }
        "codex-notification" => {
            let method = payload
                .get("method")
                .and_then(Value::as_str)
                .ok_or("RPC_METHOD_MISSING")?;
            let params = payload.get("params").ok_or("RPC_PARAMS_MISSING")?;
            let id = params
                .get("threadId")
                .or_else(|| params.pointer("/thread/id"))
                .and_then(Value::as_str)
                .ok_or("THREAD_ID_MISSING")?;
            let event = match method {
                "thread/started" => "started",
                "thread/archived" => "ended",
                "turn/completed" => "turn_completed",
                _ => return Err("NOTIFICATION_NOT_ALLOWLISTED"),
            };
            ("codex", id, event)
        }
        "acp-notification" => {
            if payload.get("method").and_then(Value::as_str) != Some("session/update") {
                return Err("NOTIFICATION_NOT_ALLOWLISTED");
            }
            let id = payload
                .pointer("/params/sessionId")
                .and_then(Value::as_str)
                .ok_or("SESSION_ID_MISSING")?;
            ("acp", id, "activity")
        }
        _ => return Err("LIFECYCLE_FORMAT_UNSUPPORTED"),
    };
    control::text(id, 256)?;
    if id.is_empty() {
        return Err("EMPTY_VENDOR_SESSION_ID");
    }
    Ok(LifecycleEvent {
        source_id: source_id.into(),
        project_id: project.into(),
        workspace_id: workspace,
        agent: agent.into(),
        external_session_id: id.into(),
        event: event.into(),
        observed_at: at,
        source_revision: version.into(),
        evidence: "agent_reported_import_not_authenticated_hook".into(),
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn prompt_not_copied() {
        let v = serde_json::json!({"hook_event_name":"SessionStart","session_id":"s","transcript_path":"PRIVATE","prompt":"PRIVATE"});
        let r = normalize("claude-hook", &v, "event", "project", None, 100, "v1").unwrap();
        assert!(!serde_json::to_string(&r).unwrap().contains("PRIVATE"));
    }
    #[test]
    fn stop_is_not_session_end() {
        let v = serde_json::json!({"hook_event_name":"Stop","session_id":"s"});
        assert_eq!(
            normalize("claude-hook", &v, "e", "p", None, 1, "v1")
                .unwrap()
                .event,
            "turn_completed"
        );
    }
}
