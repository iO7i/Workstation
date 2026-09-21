//! Explicit local metadata interface. Read-only MCP is a separate entry point.
use super::{collect, ok, Action, Cli, Response};
use serde::{de::DeserializeOwned, Deserialize};
use serde_json::{json, Value};
use std::{
    io::Read,
    path::{Path, PathBuf},
    time::Duration,
};
use workstation_core::{continuity, control::*, economics, Coverage, WorkerRequest};
use workstation_platform::{
    self as platform,
    control_store::{epoch, sha, Record},
    paths,
    storage::Store,
    Error, Result,
};
pub(super) fn input(path: &Path) -> Result<Vec<u8>> {
    paths::local_existing(path)?;
    let m = std::fs::symlink_metadata(path).map_err(|_| Error::new("INPUT_UNAVAILABLE"))?;
    if !m.is_file() {
        return Err(Error::new("REGULAR_INPUT_REQUIRED"));
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|_| Error::new("INPUT_UNAVAILABLE"))?
        .take(INPUT_LIMIT as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::new("INPUT_READ_FAILED"))?;
    if bytes.len() > INPUT_LIMIT {
        return Err(Error::new("INPUT_LIMIT"));
    }
    Ok(bytes)
}
pub(super) fn parse<T: DeserializeOwned>(path: &Path) -> Result<T> {
    serde_json::from_slice(&input(path)?).map_err(|_| Error::new("CONTROL_INPUT_SCHEMA_INVALID"))
}
pub(super) fn answer(data: Value) -> Result<Response> {
    let text = serde_json::to_string_pretty(&data).map_err(|_| Error::new("SERIALIZE_FAILED"))?;
    if text.len() > CONTEXT_LIMIT * 2 {
        return Err(Error::new("CONTROL_OUTPUT_LIMIT"));
    }
    let coverage = match data.get("coverage").and_then(Value::as_str) {
        Some("partial") => Coverage::Partial,
        Some("denied") => Coverage::Denied,
        Some("unsupported") => Coverage::Unsupported,
        Some("timed_out") => Coverage::TimedOut,
        _ => Coverage::Complete,
    };
    let mut response = ok(data, workstation_core::render::terminal_safe(&text));
    response.coverage = coverage;
    Ok(response)
}
pub fn handled(action: &Action) -> bool {
    matches!(
        action,
        Action::Upgrade
            | Action::Restore { .. }
            | Action::Record { .. }
            | Action::Context { .. }
            | Action::Timeline { .. }
            | Action::Roster { .. }
            | Action::Decisions { .. }
            | Action::Resources { .. }
            | Action::Economics { .. }
            | Action::NormalizeUsage { .. }
            | Action::Runway { .. }
            | Action::Simulate { .. }
            | Action::ModelFrontier { .. }
            | Action::SelectModels { .. }
            | Action::DiagnoseEvidence { .. }
            | Action::PlanFit { .. }
            | Action::Capabilities { .. }
            | Action::WorkspaceRegister { .. }
            | Action::Checkpoint { .. }
            | Action::Handoff { .. }
            | Action::VaultPut { .. }
    )
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Scenario {
    buckets: Vec<economics::ScenarioBucket>,
    assumptions: Vec<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckpointInput {
    session_id: Option<String>,
    completed: Vec<String>,
    remaining: Vec<String>,
    blockers: Vec<String>,
    tests: Vec<TestObservation>,
    decision_ids: Vec<String>,
    resource_ids: Vec<String>,
}
fn project(store: &Store, id: &str) -> Result<workstation_core::Project> {
    store
        .projects()?
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(|| Error::new("PROJECT_NOT_REGISTERED"))
}
fn stamp(
    store: &Store,
    project_id: &str,
    workspace_id: &str,
    path: PathBuf,
) -> Result<WorkspaceStamp> {
    collect(
        &store.home,
        WorkerRequest::WorkspaceStamp {
            project: project(store, project_id)?,
            workspace_id: workspace_id.into(),
            path,
        },
        Duration::from_secs(5),
    )
}
pub fn execute(cli: &Cli) -> Result<Response> {
    // Pure calculations need no Workstation home and never contact a provider.
    match &cli.command {
        Action::NormalizeUsage {
            format,
            input: source,
            version,
            account,
            observed_at,
        } => {
            let payload: Value = parse(source)?;
            let rows =
                economics::normalize_vendor(format, &payload, version, account, *observed_at)
                    .map_err(Error::new)?;
            return answer(
                json!({"samples":rows,"source":"explicit_operator_import","persisted":false,"provider_contacted":false}),
            );
        }
        Action::Runway { input: source, at } => {
            let rows: Vec<economics::QuotaSample> = parse(source)?;
            return answer(json!(
                economics::forecast(&rows, at.unwrap_or_else(epoch)).map_err(Error::new)?
            ));
        }
        Action::Simulate { input: source } => {
            let s: Scenario = parse(source)?;
            return answer(
                economics::counterfactual(&s.buckets, &s.assumptions).map_err(Error::new)?,
            );
        }
        Action::ModelFrontier { input: source } => {
            let rows: Vec<economics::ModelMetric> = parse(source)?;
            return answer(economics::pareto(&rows).map_err(Error::new)?);
        }
        Action::SelectModels { input: source } => {
            let query: economics::SelectionRequest = parse(source)?;
            return answer(economics::select_models(&query).map_err(Error::new)?);
        }
        Action::DiagnoseEvidence { input: source, at } => {
            let data: workstation_core::diagnostics::DiagnosticEvidence = parse(source)?;
            return answer(
                json!({"findings":workstation_core::diagnostics::classify(&data,at.unwrap_or_else(epoch)).map_err(Error::new)?,"live_host_collected":false,"source_kind":"agent_reported","repair_available":false}),
            );
        }
        Action::PlanFit { input: source } => {
            let rows: Vec<economics::Cycle> = parse(source)?;
            return answer(economics::plan_fit(&rows).map_err(Error::new)?);
        }
        _ => {}
    }
    if let Action::Restore { backup, sha256 } = &cli.command {
        let home = cli
            .home
            .as_ref()
            .ok_or_else(|| Error::new("EXPLICIT_HOME_REQUIRED"))?;
        let s = Store::restore_into(home, backup, sha256)?;
        return answer(
            json!({"restored_schema":s.schema_version()?,"integrity":s.health()?,"existing_home_overwritten":false,"vault_ciphertext_restored":false,"source_backup_unchanged":true}),
        );
    }
    let home = paths::select_home(cli.home.clone(), None)?;
    let mut store = Store::open(&home)?;
    match &cli.command {
        Action::Upgrade => answer(store.upgrade()?),
        Action::Record {
            input: source,
            approve_sha256,
        } => {
            let bytes = input(source)?;
            let command: Record = serde_json::from_slice(&bytes)
                .map_err(|_| Error::new("CONTROL_INPUT_SCHEMA_INVALID"))?;
            let digest = sha(&bytes);
            let operation = serde_json::to_value(&command)
                .ok()
                .and_then(|v| v.get("operation").cloned())
                .unwrap_or(Value::Null);
            if let Some(approval) = approve_sha256 {
                if *approval != digest {
                    return Err(Error::new("INPUT_CHANGED_APPROVAL_MISMATCH"));
                }
                answer(store.record(command, approval)?)
            } else {
                answer(
                    json!({"status":"preview_only","operation":operation,"input_sha256":digest,"input_bytes":bytes.len(),"payload_echoed":false,"next":"Review the input file locally, then rerun with --approve-sha256 equal to this exact digest.","external_changes":false,"authorization_model":"Explicit CLI acknowledgement; not a security boundary against same-user software."}),
                )
            }
        }
        Action::Context {
            project,
            environment,
            html,
            share,
        } => {
            let packet = store.context(project, environment.as_deref())?;
            if *share {
                return answer(share_context(&packet));
            }
            if *html {
                let raw = serde_json::to_string_pretty(&packet)
                    .map_err(|_| Error::new("SERIALIZE_FAILED"))?;
                let escaped = workstation_core::render::html_escape(&raw);
                let text=format!("<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'\"><title>Workstation project</title><style>body{{font:16px system-ui;max-width:1100px;margin:32px auto;padding:20px}}pre{{white-space:pre-wrap;overflow-wrap:anywhere}}</style><h1>Workstation · project overview</h1><p>Private cached context. Not a live health certification or execution authority.</p><pre>{escaped}</pre></html>");
                return Ok(ok(packet, text));
            }
            answer(packet)
        }
        Action::Timeline { project, before } => answer(store.timeline(project, *before)?),
        Action::Roster { project } => {
            answer(json!({"roster":store.roster(project)?,"advisory_only":true}))
        }
        Action::Decisions {
            project,
            valid_at,
            known_at,
        } => {
            let at = epoch();
            answer(
                json!({"decisions":store.decisions(project,valid_at.unwrap_or(at),known_at.unwrap_or(at))?}),
            )
        }
        Action::Resources {
            project,
            environment,
        } => answer(
            json!({"resources":store.resources(project,environment)?,"secret_resolution":false}),
        ),
        Action::Economics { project, at } => {
            answer(store.economics(project, at.unwrap_or_else(epoch))?)
        }
        Action::Capabilities { project } => answer(store.suggestions(project, epoch())?),
        Action::WorkspaceRegister { project, id, path } => {
            let s = stamp(&store, project, id, path.clone())?;
            store.register_workspace(project, id, path, &s.directory_identity)?;
            store.record_workspace_observation(&s)?;
            answer(
                json!({"workspace_id":id,"observed":s,"protection":"protected","no_cleanup_authority":true}),
            )
        }
        Action::Checkpoint {
            work,
            input: source,
        } => {
            let content: CheckpointInput = parse(source)?;
            let w = store.work(work)?;
            let workspace = w
                .workspace_id
                .as_ref()
                .ok_or_else(|| Error::new("WORKSPACE_ASSIGNMENT_REQUIRED"))?;
            lines(&content.completed).map_err(Error::new)?;
            lines(&content.remaining).map_err(Error::new)?;
            lines(&content.blockers).map_err(Error::new)?;
            if content.tests.len() > 16
                || content.decision_ids.len() > 32
                || content.resource_ids.len() > 32
            {
                return Err(Error::new("CHECKPOINT_COUNT_LIMIT"));
            }
            let observed = stamp(
                &store,
                &w.project_id,
                workspace,
                store.workspace_path(&w.project_id, workspace)?.into(),
            )?;
            store.record_workspace_observation(&observed)?;
            let at = epoch();
            let evidence = Evidence {
                kind: EvidenceKind::AgentReported,
                source: "explicit_checkpoint_report".into(),
                at,
                coverage: Coverage::Partial,
            };
            let mut tests = content.tests;
            for t in &mut tests {
                text(&t.name, 256).map_err(Error::new)?;
                text(&t.result, 256).map_err(Error::new)?;
                t.evidence = evidence.clone();
            }
            let cp = workstation_core::control::Checkpoint {
                id: platform::new_id(),
                work: w,
                session_id: content.session_id,
                workspace: observed,
                completed: content.completed,
                remaining: content.remaining,
                blockers: content.blockers,
                tests,
                decision_ids: content.decision_ids,
                resource_ids: content.resource_ids,
                recorded_at: at,
                source: evidence,
            };
            store.save_checkpoint(&cp)?;
            answer(
                json!({"checkpoint":cp,"test_status_evidence":"agent_reported_not_executed_here","workspace_evidence":"observed_metadata_only"}),
            )
        }
        Action::Handoff { checkpoint, target } => {
            let cp = store.checkpoint(checkpoint)?;
            let w = store.work(&cp.work.id)?;
            if w.version != cp.work.version {
                return Err(Error::new("HANDOFF_WORK_VERSION_STALE"));
            }
            let live = stamp(
                &store,
                &cp.work.project_id,
                &cp.workspace.workspace_id,
                store
                    .workspace_path(&cp.work.project_id, &cp.workspace.workspace_id)?
                    .into(),
            )?;
            store.record_workspace_observation(&live)?;
            let roster = store.roster(&cp.work.project_id)?;
            let at = epoch();
            let mut packet =
                continuity::handoff(&cp, &live, target, &roster, at).map_err(Error::new)?;
            packet.decision_ids = store
                .decisions(&cp.work.project_id, at, at)?
                .iter()
                .map(|d| d.id.clone())
                .collect();
            if serde_json::to_vec(&packet)
                .map_err(|_| Error::new("SERIALIZE_FAILED"))?
                .len()
                > HANDOFF_LIMIT
            {
                return Err(Error::new("HANDOFF_TOO_LARGE"));
            }
            answer(store.save_handoff(&packet)?)
        }
        Action::VaultPut {
            project,
            environment,
            id,
        } => {
            eprintln!("Enter credential in the local console; input will not echo. DPAPI is same-user encryption at rest, not agent isolation.");
            let bytes = platform::vault::read_hidden()?;
            let r = platform::vault::put(&mut store, project, environment, id, bytes)?;
            answer(
                json!({"stored_reference":r.locator,"plaintext_returned":false,"credentialed_execution":"explicit_pinned_task_and_expiring_effect_plan_only_uncertified"}),
            )
        }
        _ => Err(Error::new("UNSUPPORTED_CONTROL_OPERATION")),
    }
}
/// New object, never redact a raw context dump. No user strings, paths, timestamps or IDs.
pub fn share_context(packet: &Value) -> Value {
    let count = |name: &str| {
        packet
            .get(name)
            .and_then(Value::as_array)
            .map(|x| x.len())
            .unwrap_or(0)
    };
    json!({"schema_version":"workstation.control.share.v1","modules":7,"work_item_count":count("work_items"),"session_roster_count":count("agent_roster"),"workspace_count":count("workspaces"),"decision_count":count("current_decisions"),"resource_count":count("resources"),"suggestion_count":count("useful_discoveries"),"coverage":"partial","live_certification":false})
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn share_projection_does_not_leak_canaries() {
        let s=share_context(&json!({"project_id":"SECRET_CANARY","resources":[{"locator":"secret"}],"work_items":[{"objective":"SECRET_CANARY"}]})).to_string();
        assert!(!s.contains("SECRET_CANARY"));
        assert!(!s.contains("locator"));
    }
}
