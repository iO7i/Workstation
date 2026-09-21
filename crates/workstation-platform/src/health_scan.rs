//! Bounded local scans for registered profiles and live process/session evidence.
//! Neither the healthy verdict nor mutation authority is inferred from name-only matches.
use crate::{
    control_store::{epoch, sha},
    storage::Store,
    Error, Result,
};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use std::time::{Duration, Instant};
use workstation_core::{
    health_rules::*, host_graph::HostGraph, ownership::*, workspace_policy::WorkspaceDetail,
    Coverage, WorkerRequest,
};
fn collect<T: DeserializeOwned>(store: &Store, req: WorkerRequest, timeout: Duration) -> Result<T> {
    let exe = std::env::current_exe().map_err(|_| Error::new("EXECUTABLE_PATH"))?;
    let o = crate::runner::collect(&exe, &store.home, &req, timeout)?;
    if o.exit_code != 0 {
        return Err(Error::new("HEALTH_COLLECTOR_FAILED"));
    }
    serde_json::from_slice(&o.bytes).map_err(|_| Error::new("HEALTH_COLLECTOR_SHAPE"))
}
pub fn scan(store: &mut Store, project: &str, deep: bool) -> Result<Value> {
    store.require_v4()?;
    let start = Instant::now();
    let total = Duration::from_secs(if deep { 120 } else { 10 });
    let profiles = store.integrations(project)?;
    let mut observations = vec![];
    let mut notices = vec![];
    let rules: Vec<Fingerprint> =
        serde_json::from_str(include_str!("../../../fingerprints/rules-v1.json"))
            .map_err(|_| Error::new("BUILTIN_RULES_INVALID"))?;
    for profile in profiles.iter().take(if deep { 64 } else { 8 }) {
        let remaining = total.saturating_sub(start.elapsed());
        if remaining < Duration::from_millis(100) {
            notices.push("profile_budget_reached");
            break;
        }
        let mut observation = match collect::<Value>(
            store,
            WorkerRequest::AgentProfile {
                integration: profile.clone(),
            },
            remaining.min(Duration::from_secs(3)),
        ) {
            Ok(v) => v,
            Err(e) => json!({"profile_id":profile.id,"coverage":"partial","error_code":e.code}),
        };
        let key = format!("profile-{}", &sha(profile.id.as_bytes())[..16]);
        let old = store.cached_observation(project, &key)?;
        let signals = profile_signals(&profile.id, &observation, old.get("data"), epoch());
        observation["signals"] = json!(signals);
        observation["fingerprints"] = json!(evaluate(
            &rules,
            profile.adapter.id(),
            if cfg!(windows) { "windows" } else { "linux" },
            &profile.version_text,
            &signals,
            epoch()
        )
        .map_err(Error::new)?);
        store.cache_observation(
            project,
            &format!("profile-{}", &sha(profile.id.as_bytes())[..16]),
            &observation,
        )?;
        observations.push(observation);
    }
    let mut graph_value =
        json!({"coverage":"unsupported","reason":"native_windows_observation_unavailable"});
    if cfg!(windows) {
        let remaining = total.saturating_sub(start.elapsed());
        if remaining >= Duration::from_millis(100) {
            let boot = store.host_epoch(crate::process_graph::uptime_ms()?)?;
            let graph = collect::<HostGraph>(
                store,
                WorkerRequest::HostGraph {
                    host: store.config.installation_id.clone(),
                    boot,
                    profiles: profiles.clone(),
                },
                remaining.min(Duration::from_secs(5)),
            );
            match graph {
                Ok(g) => {
                    let old = store.cached_observation(project, "host-graph")?;
                    let previous = old
                        .pointer("/data/processes")
                        .cloned()
                        .map(serde_json::from_value::<Vec<ProcessFact>>)
                        .transpose()
                        .map_err(|_| Error::new("GRAPH_CACHE_INVALID"))?
                        .unwrap_or_default();
                    let ended = store
                        .roster(project)?
                        .into_iter()
                        .filter_map(|r| r.session.ended_at.map(|at| (r.session.id, at)))
                        .collect();
                    let input = OwnershipInput {
                        observed_at: g.observed_at,
                        process_coverage: g.coverage,
                        processes: g.processes.clone(),
                        bindings: store.process_bindings(project)?,
                        ended_sessions: ended,
                        previous,
                    };
                    let rows = correlate(&input).map_err(Error::new)?;
                    graph_value = json!({"coverage":g.coverage,"rows":rows,"attribution":"own_protocol_bound_sessions_plus_observed_ancestry","unmanaged_sessions":"unknown_until_explicit_metadata_binding","process_termination_authorized":false});
                    // Persist only relevant graph objects, not every unrelated executable path.
                    let known = g
                        .executables
                        .into_iter()
                        .filter(|x| !x.matching_integrations.is_empty())
                        .collect::<Vec<_>>();
                    let compact = json!({"observed_at":g.observed_at,"coverage":g.coverage,"processes":g.processes,"registered_executables":known,"denied":g.denied});
                    match store.cache_observation(project, "host-graph", &compact) {
                        Ok(()) => {}
                        Err(_) => notices.push("host_graph_cache_budget_or_io_failed"),
                    };
                    match store.cache_observation(project, "ownership", &graph_value) {
                        Ok(()) => {}
                        Err(_) => notices.push("ownership_cache_budget_or_io_failed"),
                    };
                }
                Err(_) => {
                    graph_value =
                        json!({"coverage":"timed_out","process_termination_authorized":false});
                }
            }
        }
    }
    let result = json!({"observed_at":epoch(),"project_id":project,"profiles":observations,"ownership":graph_value,"coverage":"partial","notices":notices,"all_processes_protected":true,"exhaustive_vendor_health":false,"duration_ms":start.elapsed().as_millis()});
    store.cache_observation(project, "health-v4", &result)?;
    Ok(result)
}
pub fn workspace_scan(store: &mut Store, project: &str) -> Result<Value> {
    store.require_v4()?;
    let started = Instant::now();
    let paths = store.registered_workspace_ids(project)?;
    let mut details: Vec<WorkspaceDetail> = vec![];
    let mut errors = vec![];
    for id in paths.iter().take(32) {
        if started.elapsed() > Duration::from_secs(115) {
            errors.push(json!({"workspace":id,"coverage":"timed_out"}));
            break;
        }
        match crate::workspace_lifecycle::collect(store, project, id) {
            Ok(d) => {
                store.record_workspace_observation(&d.stamp)?;
                details.push(d);
            }
            Err(e) => errors.push(json!({"workspace":id,"coverage":"partial","error_code":e.code})),
        }
    }
    let coverage = if errors.is_empty() && details.len() == paths.len() {
        Coverage::Complete
    } else {
        Coverage::Partial
    };
    let payload = json!({"details":details,"errors":errors,"coverage":coverage});
    store.cache_observation(project, "workspace-details", &payload)?;
    let actors = store.workspace_actors(project)?;
    let conflicts =
        workstation_core::workspace_policy::conflicts(&actors, epoch()).map_err(Error::new)?;
    Ok(
        json!({"workspaces":payload,"actors":actors,"collisions":conflicts,"all_worktrees_protected_until_reviewed_plan":true}),
    )
}
impl Store {
    pub fn registered_workspace_ids(&self, project: &str) -> Result<Vec<String>> {
        let mut q=self.conn.prepare("SELECT id FROM workspace_registrations WHERE project_id=?1 AND NOT EXISTS(SELECT 1 FROM workspace_retirements r WHERE r.project_id=?1 AND r.workspace_id=workspace_registrations.id) ORDER BY id LIMIT 257").map_err(|_|Error::new("WORKSPACE_QUERY"))?;
        let rows = q
            .query_map([project], |r| r.get(0))
            .map_err(|_| Error::new("WORKSPACE_QUERY"))?
            .collect::<std::result::Result<Vec<String>, _>>()
            .map_err(|_| Error::new("WORKSPACE_QUERY"))?;
        if rows.len() > 256 {
            return Err(Error::new("WORKSPACE_QUERY_LIMIT"));
        }
        Ok(rows)
    }
}

fn profile_signals(profile: &str, current: &Value, old: Option<&Value>, at: i64) -> Vec<Signal> {
    let mut out = vec![];
    let source = profile.to_owned();
    let mut add = |id: &str, value: bool| {
        out.push(Signal {
            id: id.into(),
            value,
            source: source.clone(),
            observed_at: at,
            coverage: Coverage::Complete,
        })
    };
    if let Some(matches) = current
        .get("executable_digest_matches")
        .and_then(Value::as_bool)
    {
        add("executable_changed", !matches);
    }
    let comparable = old.filter(|p| {
        p.get("profile_id") == current.get("profile_id")
            && p.get("executable_sha256").is_some()
            && p.get("executable_sha256") == current.get("executable_sha256")
            && p.get("observed_at")
                .and_then(Value::as_i64)
                .is_some_and(|t| t < at && at - t <= 86400)
    });
    if current
        .pointer("/numeric_signals/state_db_coverage")
        .and_then(Value::as_str)
        == Some("complete")
    {
        if let Some(size) = current
            .pointer("/numeric_signals/state_db_logical_bytes")
            .and_then(Value::as_u64)
        {
            add("large_state_database", size >= 1024 * 1024 * 1024);
            if let Some(p) = comparable.filter(|p| {
                p.pointer("/numeric_signals/state_db_coverage")
                    .and_then(Value::as_str)
                    == Some("complete")
                    && p.pointer("/numeric_signals/state_db_path")
                        == current.pointer("/numeric_signals/state_db_path")
                    && p.pointer("/numeric_signals/state_root_identity")
                        .is_some_and(|v| !v.is_null())
                    && p.pointer("/numeric_signals/state_root_identity")
                        == current.pointer("/numeric_signals/state_root_identity")
            }) {
                if let Some(before) = p
                    .pointer("/numeric_signals/state_db_logical_bytes")
                    .and_then(Value::as_u64)
                {
                    add("positive_comparable_growth", size > before);
                }
            }
        }
    }
    if current
        .pointer("/numeric_signals/staging/coverage")
        .and_then(Value::as_str)
        == Some("complete")
    {
        if let Some(count) = current
            .pointer("/numeric_signals/staging/count")
            .and_then(Value::as_u64)
        {
            add("repeated_staging", count >= 3);
        }
        if let (Some(size), Some(p)) = (
            current
                .pointer("/numeric_signals/staging/bytes")
                .and_then(Value::as_u64),
            comparable,
        ) {
            if p.pointer("/numeric_signals/staging/coverage")
                .and_then(Value::as_str)
                == Some("complete")
                && p.pointer("/numeric_signals/staging/path")
                    == current.pointer("/numeric_signals/staging/path")
                && p.pointer("/numeric_signals/staging/identity")
                    .is_some_and(|v| !v.is_null())
                && p.pointer("/numeric_signals/staging/identity")
                    == current.pointer("/numeric_signals/staging/identity")
            {
                if let Some(before) = p
                    .pointer("/numeric_signals/staging/bytes")
                    .and_then(Value::as_u64)
                {
                    add("positive_comparable_growth", size > before);
                }
            }
        }
    }
    // No fabricated `active_update=false` / `intentional_import=false`: those remain unknown.
    out
}
/// Optional best-effort ancestry evidence while a configured SessionStart hook is live.
/// It does not trust supplied PIDs, claim exclusive ownership, or start a vendor process.
pub fn observe_hook_ancestry(
    store: &mut Store,
    integration: &workstation_core::integrations::Integration,
    event: &workstation_core::lifecycle::LifecycleEvent,
) -> Result<Value> {
    store.require_v4()?;
    if !cfg!(windows) || event.event != "started" {
        return Ok(json!({"coverage":"unsupported","scope":"start_hook_ancestry_on_windows_only"}));
    }
    let boot = store.host_epoch(crate::process_graph::uptime_ms()?)?;
    let req = WorkerRequest::HostGraph {
        host: store.config.installation_id.clone(),
        boot,
        profiles: vec![integration.clone()],
    };
    let graph = match collect::<HostGraph>(store, req, Duration::from_millis(700)) {
        Ok(g) => g,
        Err(_) => {
            return Ok(
                json!({"coverage":"partial","binding":"unknown_collector_timeout_or_denied"}),
            )
        }
    };
    let candidate =
        hook_ancestor(&graph, std::process::id(), &integration.id).map_err(Error::new)?;
    let Some(key) = candidate else {
        return Ok(
            json!({"coverage":"partial","binding":"no_unambiguous_registered_caller_ancestor"}),
        );
    };
    use rusqlite::OptionalExtension;
    let sid: Option<String> = store
        .conn
        .query_row(
            "SELECT id FROM sessions WHERE project_id=?1 AND agent=?2 AND external_id=?3",
            rusqlite::params![
                integration.project_id,
                event.agent,
                event.external_session_id
            ],
            |r| r.get(0),
        )
        .optional()
        .map_err(|_| Error::new("HOOK_SESSION_QUERY"))?;
    let Some(sid) = sid else {
        return Ok(json!({"coverage":"partial","binding":"session_not_recorded"}));
    };
    let at = epoch();
    let tx = store
        .conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|_| Error::new("HOOK_BINDING_TRANSACTION"))?;
    tx.execute("INSERT INTO hook_process_bindings VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9) ON CONFLICT(session_id,host_id,boot_id,pid,creation_time) DO UPDATE SET bound_at=excluded.bound_at,expires_at=excluded.expires_at,source_event=excluded.source_event",rusqlite::params![sid,integration.id,key.host,key.boot,key.pid,key.created.to_string(),event.source_id,at,at+300]).map_err(|_|Error::new("HOOK_BINDING_REJECTED"))?;
    // Transient observations, not canonical decision/audit history. Expired entries are not authority.
    tx.execute(
        "DELETE FROM hook_process_bindings WHERE expires_at<?1",
        [at - 86400],
    )
    .map_err(|_| Error::new("HOOK_BINDING_RETENTION"))?;
    tx.commit().map_err(|_| Error::new("HOOK_BINDING_COMMIT"))?;
    Ok(
        json!({"coverage":"partial","binding":"observed_registered_ancestor_plus_agent_reported_session","shared_or_nonexclusive":true,"protected":true,"session_id":sid,"expires_at":at+300}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_inspection_does_not_claim_executable_changed_or_healthy() {
        assert!(profile_signals("x", &json!({"coverage":"partial"}), None, 10).is_empty());
    }
    #[test]
    fn partial_state_measurement_does_not_produce_size_signal() {
        assert!(profile_signals("x",&json!({"numeric_signals":{"state_db_coverage":"partial","state_db_logical_bytes":9999999999u64}}),None,10).is_empty());
    }
}
