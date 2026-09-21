//! Explicit, pinned, environment-scoped credentialed execution. No secret-returning public tool.
use crate::{
    external::{self, Launch},
    storage::Store,
    vault::SecretBytes,
    Error, Result,
};
use serde_json::{json, Value};
use std::{collections::BTreeMap, ffi::OsString, path::Path, time::Duration};
use workstation_core::{
    control::{EvidenceKind, Resource, ResourceKind},
    integrations::*,
};
pub fn validate_files(t: &ApprovedTask) -> Result<()> {
    t.validate().map_err(Error::new)?;
    crate::paths::directory(Path::new(&t.cwd))?;
    if crate::control_workspace::directory_identity(Path::new(&t.cwd))? != t.directory_identity {
        return Err(Error::new("TASK_DIRECTORY_CHANGED"));
    }
    if external::hash_file(Path::new(&t.executable))? != t.executable_sha256 {
        return Err(Error::new("TASK_EXECUTABLE_CHANGED"));
    }
    for p in &t.script_pins {
        if external::hash_file(Path::new(&p.path))? != p.sha256 {
            return Err(Error::new("TASK_SCRIPT_CHANGED"));
        }
    }
    // Pinned interpreter code must actually be named among the arguments. A pin of an unrelated file is not proof.
    let name = t
        .executable
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    if ["node.exe", "node", "python.exe", "python", "python3"].contains(&name.as_str())
        && !t
            .script_pins
            .iter()
            .any(|p| t.args.iter().any(|a| a == &p.path))
    {
        return Err(Error::new("TASK_SCRIPT_ARGUMENT_NOT_PINNED"));
    }
    Ok(())
}
pub fn resource(store: &Store, project: &str, environment: &str, id: &str) -> Result<Resource> {
    use rusqlite::{params, OptionalExtension};
    store.require_environment(project, environment)?;
    let mut r = store
        .resources(project, environment)?
        .into_iter()
        .find(|r| r.id == id)
        .ok_or(Error::new("RESOURCE_NOT_IN_EXACT_ENVIRONMENT"))?;
    let confirmation:Option<(i64,String)>=store.conn.query_row("SELECT observed_at,payload FROM resource_verifications WHERE resource_id=?1 AND claim='user_confirmed' ORDER BY observed_at DESC,id DESC LIMIT 1",params![id],|row|Ok((row.get(0)?,row.get(1)?))).optional().map_err(|_|Error::new("RESOURCE_CONFIRMATION_QUERY"))?;
    if let Some((at, p)) = confirmation {
        let v: workstation_core::control::Verification =
            serde_json::from_str(&p).map_err(|_| Error::new("RESOURCE_CONFIRMATION_INVALID"))?;
        if v.resource_id != r.id || v.source.kind != EvidenceKind::UserApproved {
            return Err(Error::new("RESOURCE_CONFIRMATION_MISMATCH"));
        }
        let now = crate::control_store::epoch();
        if at > now || now - at > i64::from(r.fresh_for_secs) {
            return Err(Error::new("RESOURCE_CONFIRMATION_STALE_REVIEW_REQUIRED"));
        }
        r.evidence = v.source;
    }
    Ok(r)
}
fn local_secret(store: &Store, r: &Resource) -> Result<SecretBytes> {
    if r.kind != ResourceKind::SecretReference {
        return Err(Error::new("RESOURCE_NOT_A_SECRET_REFERENCE"));
    }
    if !matches!(
        r.evidence.kind,
        EvidenceKind::UserApproved | EvidenceKind::ProviderVerified
    ) {
        return Err(Error::new("SECRET_REFERENCE_NOT_APPROVED"));
    }
    let expected = format!("local://{}/{}/{}", r.project_id, r.environment, r.id);
    if r.locator != expected {
        return Err(Error::new("LOCAL_SECRET_SCOPE_MISMATCH"));
    }
    crate::secret_lifecycle::resolve(store, &r.project_id, &r.environment, &r.id)
}
/// Auth bootstrap is local DPAPI only: recursive provider authentication cannot loop.
pub fn environment(store: &Store, i: &Integration) -> Result<BTreeMap<OsString, OsString>> {
    i.validate().map_err(Error::new)?;
    store.require_environment(&i.project_id, &i.environment)?;
    let mut e = external::scoped_environment(&store.home, &i.inherit_env, &i.configuration)?;
    for (k, id) in &i.authentication {
        let r = resource(store, &i.project_id, &i.environment, id)?;
        let s = local_secret(store, &r)?;
        let text = std::str::from_utf8(&s.0).map_err(|_| Error::new("AUTH_SECRET_ENCODING"))?;
        if text.contains('\0') {
            return Err(Error::new("AUTH_SECRET_NUL"));
        }
        e.insert(k.into(), text.into());
    }
    Ok(e)
}
fn provider_profile(
    store: &Store,
    project: &str,
    environment: &str,
    adapter: Adapter,
) -> Result<Integration> {
    let p: Vec<_> = store
        .integrations(project)?
        .into_iter()
        .filter(|p| p.environment == environment && p.adapter == adapter)
        .collect();
    if p.len() != 1 {
        return Err(Error::new("SECRET_PROVIDER_PROFILE_MISSING_OR_AMBIGUOUS"));
    }
    Ok(p[0].clone())
}
pub(crate) fn resolve(
    store: &Store,
    project: &str,
    environment_name: &str,
    id: &str,
) -> Result<SecretBytes> {
    let r = resource(store, project, environment_name, id)?;
    if r.kind != ResourceKind::SecretReference {
        return Err(Error::new("SECRET_REFERENCE_REQUIRED"));
    }
    if !matches!(
        r.evidence.kind,
        EvidenceKind::UserApproved | EvidenceKind::ProviderVerified
    ) {
        return Err(Error::new("SECRET_REFERENCE_NOT_APPROVED"));
    }
    if r.locator.starts_with("local://") {
        return local_secret(store, &r);
    }
    let (adapter, args) = if let Some(rest) = r.locator.strip_prefix("doppler://") {
        let parts: Vec<_> = rest.split('/').collect();
        if parts.len() != 3
            || parts.iter().any(|s| {
                s.is_empty()
                    || s.starts_with('-')
                    || !s
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c))
            })
        {
            return Err(Error::new("DOPPLER_REFERENCE_FORMAT"));
        }
        (
            Adapter::Doppler,
            vec![
                "secrets".into(),
                "get".into(),
                parts[2].into(),
                "--plain".into(),
                "--project".into(),
                parts[0].into(),
                "--config".into(),
                parts[1].into(),
            ],
        )
    } else if r.locator.starts_with("op://") {
        if r.locator[5..].split('/').count() != 3 {
            return Err(Error::new("OP_REFERENCE_FORMAT"));
        }
        (
            Adapter::OnePassword,
            vec!["read".into(), r.locator.clone(), "--no-newline".into()],
        )
    } else {
        return Err(Error::new("SECRET_PROVIDER_REFERENCE_ONLY"));
    };
    let i = provider_profile(store, project, environment_name, adapter)?;
    let capture = external::one_shot(
        Launch {
            executable: i.executable.clone().into(),
            expected_sha256: i.executable_sha256.clone(),
            cwd: store.home.clone(),
            args: args.into_iter().map(OsString::from).collect(),
            env: environment(store, &i)?,
            timeout: Duration::from_secs(15),
            output_limit: 16 * 1024,
        },
        None,
    )?;
    if capture.exit_code != 0 {
        return Err(Error::new("SECRET_PROVIDER_FAILED_OUTPUT_SUPPRESSED"));
    }
    if capture.bytes.0.is_empty() || capture.bytes.0.contains(&0) {
        return Err(Error::new("SECRET_PROVIDER_EMPTY_OR_NUL"));
    }
    Ok(capture.bytes)
}
pub fn execute(store: &Store, t: &ApprovedTask) -> Result<Value> {
    validate_files(t)?;
    store.require_environment(&t.project_id, &t.environment)?;
    let mut env = external::scoped_environment(&store.home, &[], &BTreeMap::new())?;
    for (key, id) in &t.secret_bindings {
        safe_secret_env(key).map_err(Error::new)?;
        let secret = resolve(store, &t.project_id, &t.environment, id)?;
        let s = std::str::from_utf8(&secret.0).map_err(|_| Error::new("TASK_SECRET_ENCODING"))?;
        if s.contains('\0') {
            return Err(Error::new("TASK_SECRET_NUL"));
        }
        env.insert(key.into(), s.into());
    }
    // Revalidate after resolution/network delay. Child code and dependencies can read injected values;
    // this is deliberate authorization, NOT a sandbox against that code or same-user software.
    validate_files(t)?;
    let output = external::one_shot(
        Launch {
            executable: t.executable.clone().into(),
            expected_sha256: t.executable_sha256.clone(),
            cwd: t.cwd.clone().into(),
            args: t.args.iter().map(OsString::from).collect(),
            env,
            timeout: Duration::from_secs(t.timeout_seconds.into()),
            output_limit: 1024 * 1024,
        },
        None,
    )?;
    if output.exit_code != 0 {
        return Err(Error::new("TASK_NONZERO_OUTPUT_SUPPRESSED"));
    }
    Ok(
        json!({"task_id":t.id,"exit_code":0,"stdout":"discarded","stderr":"discarded","secret_bindings":t.secret_bindings.len(),"data_effects":"authorized_child_may_have_changed_external_state"}),
    )
}
