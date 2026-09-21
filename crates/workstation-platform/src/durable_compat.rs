//! Exact-binary scoped compatibility metadata. A probe is not authentication or certification.
use crate::{
    control_store::{epoch, sha},
    storage::Store,
    Error, Result,
};
use serde_json::{json, Value};
use workstation_core::{
    effects::QueryKind,
    integrations::{Adapter, Integration},
};
fn key(i: &Integration) -> String {
    format!("compat-{}", &sha(i.id.as_bytes())[..16])
}
pub fn save(store: &mut Store, i: &Integration, query: QueryKind, value: &Value) -> Result<()> {
    if !matches!(query, QueryKind::Version | QueryKind::ProtocolHealth) {
        return Ok(());
    }
    let old = store.cached_observation(&i.project_id, &key(i))?;
    let mut data = if old
        .pointer("/data/executable_sha256")
        .and_then(Value::as_str)
        == Some(&i.executable_sha256)
    {
        old.get("data").cloned().unwrap_or(json!({}))
    } else {
        json!({})
    };
    data["executable_sha256"] = json!(i.executable_sha256);
    data["declared_version"] = json!(i.version_text);
    data["adapter"] = json!(i.adapter.id());
    data["integration_id"] = json!(i.id);
    data["environment"] = json!(i.environment);
    data["observed_at"] = json!(epoch());
    if query == QueryKind::Version {
        data["version_matches"] = json!(value.get("matches_registration").and_then(Value::as_bool));
    } else {
        let mut projected = json!({});
        for k in [
            "connected",
            "load_session",
            "list_sessions",
            "resume_without_replay",
            "protocol_version",
        ] {
            if let Some(v) = value.get(k) {
                if v.is_boolean() || v.is_u64() {
                    projected[k] = v.clone();
                }
            }
        }
        let auth: Vec<_> = value
            .get("auth_method_ids")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .filter(|s| s.len() <= 128 && !s.chars().any(char::is_control))
            .take(16)
            .collect();
        projected["auth_method_ids"] = json!(auth);
        data["protocol"] = projected;
    }
    store.cache_observation(&i.project_id, &key(i), &data)
}
pub fn inspect(store: &Store, id: &str) -> Result<Value> {
    let i = store.integration(id)?;
    let cache = store.cached_observation(&i.project_id, &key(&i))?;
    let applies = cache.get("fresh").and_then(Value::as_bool) == Some(true)
        && cache
            .pointer("/data/executable_sha256")
            .and_then(Value::as_str)
            == Some(&i.executable_sha256)
        && cache
            .pointer("/data/declared_version")
            .and_then(Value::as_str)
            == Some(&i.version_text)
        && cache.pointer("/data/environment").and_then(Value::as_str) == Some(&i.environment);
    Ok(
        json!({"integration_id":id,"cached_only":true,"applies_to_current_registration":applies,"evidence":cache,"authentication_proven":false}),
    )
}
pub fn preflight(
    store: &Store,
    i: &Integration,
    resume: Option<&str>,
    permission: &str,
) -> Result<()> {
    i.validate().map_err(Error::new)?;
    if i.adapter == Adapter::GeminiAcp && permission == "read_only" {
        return Err(Error::new("GEMINI_READ_ONLY_MODE_UNSUPPORTED"));
    }
    if crate::external::hash_file(std::path::Path::new(&i.executable))? != i.executable_sha256 {
        return Err(Error::new("EXECUTABLE_CHANGED"));
    }
    let v = inspect(store, &i.id)?;
    if v.get("applies_to_current_registration")
        .and_then(Value::as_bool)
        != Some(true)
    {
        return Ok(());
    } // absence stays unknown, never a certification badge
    if v.pointer("/evidence/data/version_matches")
        .and_then(Value::as_bool)
        == Some(false)
    {
        return Err(Error::new("ADAPTER_VERSION_MISMATCH"));
    }
    if let (Some(expected), Some(actual)) = (
        i.protocol_revision,
        v.pointer("/evidence/data/protocol/protocol_version")
            .and_then(Value::as_u64),
    ) {
        if u64::from(expected) != actual {
            return Err(Error::new("ADAPTER_PROTOCOL_REVISION_MISMATCH"));
        }
    }
    if let Some(method) = &i.auth_method {
        if let Some(ids) = v
            .pointer("/evidence/data/protocol/auth_method_ids")
            .and_then(Value::as_array)
        {
            if !ids.iter().any(|x| x.as_str() == Some(method)) {
                return Err(Error::new("ACP_AUTH_METHOD_NOT_ADVERTISED"));
            }
        }
    }
    if resume.is_some()
        && v.pointer("/evidence/data/protocol/load_session")
            .and_then(Value::as_bool)
            == Some(false)
    {
        return Err(Error::new("ADAPTER_RESUME_NOT_ADVERTISED"));
    }
    Ok(())
}
