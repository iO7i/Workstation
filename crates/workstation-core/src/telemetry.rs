//! Metadata-only projections. Session token totals are NOT current context or quota.
use crate::{
    control::{self, Check},
    economics::{self, QuotaSample},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Telemetry {
    pub format: String,
    pub samples: Vec<QuotaSample>,
    pub metrics: Value,
    pub provenance: String,
}
fn count(v: Option<&Value>) -> Check<Option<u64>> {
    match v {
        None | Some(Value::Null) => Ok(None),
        Some(v) => {
            let n = v.as_u64().ok_or("TOKEN_COUNT_INVALID")?;
            if n > 1_000_000_000_000 {
                return Err("TOKEN_COUNT_LIMIT");
            }
            Ok(Some(n))
        }
    }
}
pub fn normalize(
    format: &str,
    payload: &Value,
    version: &str,
    account: &str,
    at: i64,
) -> Check<Telemetry> {
    control::id(account)?;
    control::timestamp(at)?;
    control::text(version, 128)?;
    let mut out = Telemetry {
        format: format.into(),
        samples: vec![],
        metrics: Value::Null,
        provenance: "agent_reported_metadata_not_authenticated_provider".into(),
    };
    match format {
        "claude-statusline-2026-09" | "codex-rate-limits-2026-09" => {
            out.samples = economics::normalize_vendor(format, payload, version, account, at)?;
        }
        "gemini-headless-json-2026-09" => {
            let models = payload
                .pointer("/stats/models")
                .and_then(Value::as_object)
                .ok_or("GEMINI_STATS_MISSING")?;
            if models.len() > 32 {
                return Err("MODEL_METRIC_LIMIT");
            }
            let mut rows = vec![];
            for (model, v) in models {
                control::text(model, 256)?;
                let tokens = v
                    .get("tokens")
                    .and_then(Value::as_object)
                    .ok_or("GEMINI_TOKENS_MISSING")?;
                rows.push(json!({"model":model,"input_uncached":count(tokens.get("input"))?,"input_prompt":count(tokens.get("prompt"))?,"output_candidates":count(tokens.get("candidates"))?,"total":count(tokens.get("total"))?,"cached":count(tokens.get("cached"))?,"thoughts":count(tokens.get("thoughts"))?,"tool":count(tokens.get("tool"))?}));
            }
            out.metrics = json!({"models":rows,"meaning":"reported_cumulative_session_tokens_not_current_context","subscription_quota":null,"current_context_capacity":null,"prices":null,"observed_at":at});
        }
        "copilot-context-event-2026-09" => {
            let data = payload.get("data").unwrap_or(payload);
            let used = count(data.get("currentTokens"))?;
            let limit = count(data.get("tokenLimit"))?;
            if used.is_none() {
                return Err("CONTEXT_USAGE_UNKNOWN");
            }
            out.metrics = json!({"current_tokens":used,"token_limit":limit,"meaning":"current_context_snapshot_not_subscription","quota_reset":null,"observed_at":at});
        }
        _ => return Err("TELEMETRY_FORMAT_UNSUPPORTED"),
    }
    Ok(out)
}
pub fn profile_formats(adapter: crate::integrations::Adapter) -> &'static [&'static str] {
    use crate::integrations::Adapter;
    match adapter {
        Adapter::Claude => &["claude-statusline-2026-09"],
        Adapter::Codex => &["codex-rate-limits-2026-09"],
        Adapter::GeminiAcp => &["gemini-headless-json-2026-09"],
        Adapter::Copilot => &["copilot-context-event-2026-09"],
        _ => &[],
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gemini_total_not_quota() {
        let x=normalize("gemini-headless-json-2026-09",&json!({"response":"SECRET_CANARY","stats":{"models":{"gemini-fixture":{"tokens":{"prompt":10,"candidates":5,"total":15}}}}}),"fixture","a",100).unwrap();
        assert!(x.samples.is_empty());
        assert!(x.metrics["subscription_quota"].is_null());
        assert!(!serde_json::to_string(&x).unwrap().contains("SECRET_CANARY"));
    }
    #[test]
    fn missing_token_is_unknown_not_zero() {
        let x = normalize(
            "gemini-headless-json-2026-09",
            &json!({"stats":{"models":{"m":{"tokens":{"total":1}}}}}),
            "v",
            "a",
            1,
        )
        .unwrap();
        assert!(x.metrics["models"][0]["cached"].is_null());
    }
    #[test]
    fn invalid_token_rejected() {
        assert!(count(Some(&json!(-1))).is_err());
        assert!(count(Some(&json!(1.5))).is_err());
    }
    #[test]
    fn claude_reports_not_bills() {
        let x=normalize("claude-statusline-2026-09",&json!({"rate_limits":{"five_hour":{"used_percentage":50,"resets_at":999}},"cost":{"total_cost_usd":1.1}}),"v","a",100).unwrap();
        assert_eq!(x.samples.len(), 2);
        assert_eq!(x.samples[1].precision, "estimated");
    }
}
