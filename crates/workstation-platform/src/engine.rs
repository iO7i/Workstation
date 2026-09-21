//! Explicitly invoked effect engine. No background action and no MCP mutation endpoint.
use crate::{
    control_store::{epoch, sha},
    storage::Store,
    Error, Result,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{fs, path::Path, time::Duration};
use workstation_core::{continuity, control::*, effects::*, integrations::*, WorkerRequest};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum Prepare {
    Operation {
        request: crate::operation_runtime::Request,
    },
    Query {
        integration_id: String,
        query: QueryKind,
    },
    Task {
        task_id: String,
    },
    Continue {
        integration_id: String,
        checkpoint_id: String,
        resume_external_id: Option<String>,
        permission_mode: String,
        timeout_seconds: u32,
        #[serde(default)]
        deadlines: Option<workstation_core::durable::DeadlinePolicy>,
    },
    Billing {
        provider: BillingProvider,
        credential_resource_id: String,
        account_alias: String,
        start_time: i64,
        end_time: i64,
    },
    Endpoint {
        resource_id: String,
    },
    Repair {
        kind: RepairKind,
        root: String,
        error_file: String,
        source_version: String,
    },
    RepairUndo {
        execution_id: String,
    },
    DockerStart {
        integration_id: String,
    },
}
pub fn capture(store: &Store, project: &str, workspace: &str) -> Result<WorkspaceStamp> {
    let p = store
        .projects()?
        .into_iter()
        .find(|p| p.id == project)
        .ok_or(Error::new("PROJECT_MISSING"))?;
    let request = WorkerRequest::WorkspaceStamp {
        project: p,
        workspace_id: workspace.into(),
        path: store.workspace_path(project, workspace)?.into(),
    };
    let executable = std::env::current_exe().map_err(|_| Error::new("EXECUTABLE_PATH"))?;
    let output =
        crate::runner::collect(&executable, &store.home, &request, Duration::from_secs(5))?;
    if output.exit_code != 0 {
        return Err(Error::new("WORKSPACE_COLLECTOR_FAILED"));
    }
    serde_json::from_slice(&output.bytes).map_err(|_| Error::new("WORKSPACE_COLLECTOR_SHAPE"))
}
fn profile(store: &Store, id: &str, project: &str, env: &str) -> Result<Integration> {
    let i = store.integration(id)?;
    if i.project_id != project || i.environment != env {
        return Err(Error::new("INTEGRATION_SCOPE_MISMATCH"));
    }
    i.validate().map_err(Error::new)?;
    Ok(i)
}
fn encoded<T: Serialize>(v: &T) -> Result<Vec<u8>> {
    serde_json::to_vec(v).map_err(|_| Error::new("ENCODING_FAILED"))
}
fn stamp_digest(s: &WorkspaceStamp) -> Result<String> {
    // Observation time changes on every read; hash only the exact relevant identity/state.
    Ok(sha(&encoded(
        &json!({"project":s.project_id,"workspace":s.workspace_id,"path":s.path,"identity":s.directory_identity,"head":s.head,"branch":s.branch,"status":s.status_digest,"coverage":s.coverage,"blockers":s.blockers}),
    )?))
}
pub fn prepare(
    store: &mut Store,
    project: &str,
    environment: &str,
    request: Prepare,
) -> Result<Value> {
    store.require_environment(project, environment)?;
    let at = epoch();
    let effect = match request {
        Prepare::Operation { request } => Effect::Operation {
            request: crate::operation_runtime::prepare(store, project, environment, request)?,
        },
        Prepare::Query {
            integration_id,
            query,
        } => {
            let i = profile(store, &integration_id, project, environment)?;
            Effect::IntegrationQuery {
                integration_id,
                integration_digest: sha(&encoded(&i)?),
                query,
            }
        }
        Prepare::Task { task_id } => {
            let t = store.task(&task_id)?;
            crate::tasks::validate_files(&t)?;
            Effect::CredentialedTask { task: t }
        }
        Prepare::Continue {
            integration_id,
            checkpoint_id,
            resume_external_id,
            permission_mode,
            timeout_seconds,
            deadlines,
        } => {
            store.require_v5()?;
            let i = profile(store, &integration_id, project, environment)?;
            crate::durable_compat::preflight(
                store,
                &i,
                resume_external_id.as_deref(),
                &permission_mode,
            )?;
            if !matches!(
                i.adapter,
                Adapter::Codex
                    | Adapter::Claude
                    | Adapter::CursorAcp
                    | Adapter::GrokAcp
                    | Adapter::GeminiAcp
                    | Adapter::OpenCode
                    | Adapter::Cline
                    | Adapter::Copilot
            ) {
                return Err(Error::new("CONTINUATION_ADAPTER_NOT_IMPLEMENTED"));
            }
            let cp = store.checkpoint(&checkpoint_id)?;
            if cp.work.project_id != project {
                return Err(Error::new("CHECKPOINT_SCOPE"));
            }
            let work = store.work(&cp.work.id)?;
            if work.version != cp.work.version {
                return Err(Error::new("CHECKPOINT_WORK_VERSION_STALE"));
            }
            let live = capture(store, project, &cp.workspace.workspace_id)?;
            // The collector stamps the observation in a child process. Compare it with a
            // wall-clock sample taken after collection so crossing a second boundary cannot
            // make a fresh observation appear to come from the future.
            continuity::revalidate_checkpoint(&cp, &live, epoch()).map_err(Error::new)?;
            Effect::Continue {
                integration_id,
                integration_digest: sha(&encoded(&i)?),
                checkpoint_id,
                workspace_digest: stamp_digest(&live)?,
                work_id: work.id,
                work_version: work.version,
                resume_external_id,
                permission_mode,
                timeout_seconds,
                deadlines,
                packet_digest: sha(&encoded(&cp)?),
                authority_digest: store.authority_digest(project, environment)?,
            }
        }
        Prepare::Billing {
            provider,
            credential_resource_id,
            account_alias,
            start_time,
            end_time,
        } => {
            crate::tasks::resource(store, project, environment, &credential_resource_id)?;
            Effect::BillingRead {
                provider,
                credential_resource_id,
                account_alias,
                start_time,
                end_time,
            }
        }
        Prepare::Endpoint { resource_id } => {
            let r = crate::tasks::resource(store, project, environment, &resource_id)?;
            Effect::EndpointCheck {
                resource_id,
                resource_digest: sha(&encoded(&r)?),
            }
        }
        Prepare::Repair {
            kind,
            root,
            error_file,
            source_version,
        } => Effect::Repair {
            target: crate::repairs::inspect(
                Path::new(&root),
                kind,
                Path::new(&error_file),
                &source_version,
            )?,
        },
        Prepare::RepairUndo { execution_id } => {
            let r = store.completed_receipt(&execution_id, project)?;
            Effect::RepairUndo {
                execution_id,
                receipt_digest: sha(&encoded(&r)?),
            }
        }
        Prepare::DockerStart { integration_id } => {
            let i = profile(store, &integration_id, project, environment)?;
            if i.adapter != Adapter::Docker {
                return Err(Error::new("DOCKER_ADAPTER_REQUIRED"));
            }
            let (executable, sha256) = crate::repairs::desktop_identity(&i)?;
            Effect::DockerStart {
                integration_id,
                integration_digest: sha(&encoded(&i)?),
                desktop_executable: executable,
                desktop_sha256: sha256,
            }
        }
    };
    let plan=EffectPlan{id:crate::new_id(),project_id:project.into(),environment:environment.into(),policy:EFFECT_POLICY.into(),created_at:at,expires_at:at+600,effect,secret_generation_digest:store.secret_generation_digest(project,environment)?,inherited_paths_digest:inherited_paths_digest(),warnings:vec!["Source implementation is unverified; native certification is deferred.".into(),"External processes may execute user configuration/hooks. Pinning executable/script identity is not a sandbox.".into(),"Review exact project/environment, destinations, inherited paths, credentials and intended effects locally.".into(),"ACP allow_once acknowledges only explicitly located in-worktree read/edit/search; execute, fetch, delete, unknown locations and sensitive configuration remain denied.".into(),"Approval is a same-user CLI acknowledgement, not an authentication boundary against same-user software.".into()]};
    let saved = store.save_effect_plan(&plan)?;
    if matches!(plan.effect, Effect::Continue { .. }) {
        store.durable_prepare(&plan)?;
    }
    Ok(saved)
}
fn verify_profile(store: &Store, id: &str, digest: &str, p: &EffectPlan) -> Result<Integration> {
    let i = profile(store, id, &p.project_id, &p.environment)?;
    if sha(&encoded(&i)?) != digest {
        return Err(Error::new("INTEGRATION_CHANGED"));
    }
    Ok(i)
}
pub fn apply(
    store: &mut Store,
    id: &str,
    approval: &str,
    ack_uncertified: bool,
) -> Result<Receipt> {
    if !ack_uncertified {
        return Err(Error::new("UNVERIFIED_RELEASE_EXPLICIT_ACK_REQUIRED"));
    }
    // Serialize external effects, not read-only MCP. OS releases lock on a crash; never deletes lock by age.
    let lock_path = store.home.join("effects.lock");
    if crate::paths::entry_exists(&lock_path)? {
        crate::paths::local_existing(&lock_path)?;
    }
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(lock_path)
        .map_err(|_| Error::new("EFFECT_LOCK_OPEN"))?;
    lock.try_lock()
        .map_err(|_| Error::new("ANOTHER_EFFECT_RUNNING"))?;
    let (p, d) = store.effect_plan(id)?;
    if p.secret_generation_digest
        != store.secret_generation_digest(&p.project_id, &p.environment)?
    {
        return Err(Error::new("SECRET_GENERATION_CHANGED_SINCE_APPROVAL"));
    }
    if p.inherited_paths_digest != inherited_paths_digest() {
        return Err(Error::new("INHERITED_PATH_ENVIRONMENT_CHANGED"));
    }
    let durable = matches!(p.effect, Effect::Continue { .. });
    let run = if durable {
        store.durable_begin(&p, &d, approval)?
    } else {
        store.begin_effect(&p, &d, approval)?
    };
    let start = epoch();
    store.effect_step(
        &run,
        "execute_typed_plan",
        "intent",
        &json!({"plan_id":p.id,"policy":p.policy,"approval_digest":d}),
    )?;
    let monitor = if durable {
        Some(crate::durable_runtime::install(&store.home, &p.id)?)
    } else {
        None
    };
    let result = dispatch(store, &run, &p);
    drop(monitor);
    if durable {
        crate::durable_runtime::finish(store, &p.id, &result)?;
    }
    let receipt = match result {
        Ok(v) => Receipt {
            execution_id: run.clone(),
            plan_id: p.id,
            state: ExecutionState::Succeeded,
            started_at: start,
            finished_at: Some(epoch()),
            error_code: None,
            output: v,
            root_cause_fixed: false,
        },
        Err(e) => Receipt {
            execution_id: run.clone(),
            plan_id: p.id,
            state: ExecutionState::Indeterminate,
            started_at: start,
            finished_at: Some(epoch()),
            error_code: Some(e.code.into()),
            output: json!({"review_required":true,"retry_automatically":false,"partial_external_effects_possible":true}),
            root_cause_fixed: false,
        },
    };
    if matches!(p.effect, Effect::Continue { .. }) {
        store.finish_continuation(&run, receipt.state == ExecutionState::Succeeded)?;
    }
    store.finish_effect(&receipt)?;
    Ok(receipt)
}
fn dispatch(store: &mut Store, run: &str, p: &EffectPlan) -> Result<Value> {
    match &p.effect {
        Effect::Operation { request } => crate::operation_runtime::execute(store, run, p, request),
        Effect::IntegrationQuery {
            integration_id,
            integration_digest,
            query,
        } => {
            let i = verify_profile(store, integration_id, integration_digest, p)?;
            let value = crate::adapters::query(store, &i, *query)?;
            let category = match query {
                QueryKind::CodexQuota | QueryKind::CopilotQuota => "quota",
                QueryKind::CodexThreads | QueryKind::VendorSessions => "sessions",
                QueryKind::CopilotModels => "models",
                QueryKind::ProtocolHealth => "protocol-health",
                QueryKind::WorktrunkList => "worktrunk",
                QueryKind::DockerLocalEngine => "docker-local",
                QueryKind::Version => "version",
            };
            let category = format!("{category}-{}", &sha(i.id.as_bytes())[..16]);
            if matches!(query, QueryKind::CodexQuota | QueryKind::CopilotQuota) {
                if let Some(samples) = value.get("samples") {
                    let mut rows: Vec<workstation_core::economics::QuotaSample> =
                        serde_json::from_value(samples.clone())
                            .map_err(|_| Error::new("QUOTA_NORMALIZATION"))?;
                    for (idx, s) in rows.iter_mut().enumerate() {
                        s.id = format!("sample-{run}-{idx}");
                        s.evidence.kind = EvidenceKind::Observed;
                        s.evidence.source = format!(
                            "received_owned_{}_protocol_not_account_identity_verification",
                            i.adapter.id()
                        );
                    }
                    store.save_received_quota(&p.project_id, run, rows)?;
                }
            }
            store.cache_observation(&p.project_id, &category, &value)?;
            crate::durable_compat::save(store, &i, *query, &value)?;
            Ok(value)
        }
        Effect::CredentialedTask { task } => crate::tasks::execute(store, task),
        Effect::Continue {
            integration_id,
            integration_digest,
            checkpoint_id,
            workspace_digest,
            work_id,
            work_version,
            resume_external_id,
            permission_mode,
            timeout_seconds,
            deadlines,
            packet_digest,
            authority_digest,
        } => {
            let i = verify_profile(store, integration_id, integration_digest, p)?;
            crate::durable_compat::preflight(
                store,
                &i,
                resume_external_id.as_deref(),
                permission_mode,
            )?;
            let effective_timeout = deadlines
                .as_ref()
                .map(|p| p.absolute_seconds)
                .unwrap_or(*timeout_seconds);
            if store.authority_digest(&p.project_id, &p.environment)? != *authority_digest {
                return Err(Error::new("AUTHORITATIVE_CONTEXT_CHANGED"));
            }
            let cp = store.checkpoint(checkpoint_id)?;
            if cp.work.project_id != p.project_id
                || cp.work.id != *work_id
                || store.work(work_id)?.version != *work_version
                || sha(&encoded(&cp)?) != *packet_digest
            {
                return Err(Error::new("CONTINUATION_STATE_CHANGED"));
            }
            let live = capture(store, &p.project_id, &cp.workspace.workspace_id)?;
            let at = epoch();
            continuity::revalidate_checkpoint(&cp, &live, at).map_err(Error::new)?;
            if stamp_digest(&live)? != *workspace_digest {
                return Err(Error::new("WORKSPACE_CHANGED_SINCE_PREVIEW"));
            }
            let roster = store.roster(&p.project_id)?;
            let mut packet =
                continuity::handoff(&cp, &live, i.adapter.id(), &roster, at).map_err(Error::new)?;
            packet.mode = if resume_external_id.is_some() {
                "native_resume"
            } else {
                "cross_agent_new_session"
            }
            .into();
            packet.authority = "exact_expiring_continuation_plan_only".into();
            // Include authoritative current decisions and environment-scoped references; no credential values.
            let context = store.context(&p.project_id, Some(&p.environment))?;
            let payload = json!({"handoff":packet,"current_context":context});
            if encoded(&payload)?.len() > 65536 {
                return Err(Error::new("CONTINUATION_PACKET_LIMIT"));
            }
            store.reserve_continuation(
                run,
                &cp,
                &i,
                resume_external_id.as_deref(),
                effective_timeout,
            )?;
            let reader = Store::open_readonly(&store.home)?;
            let outcome = crate::adapters::continue_once(
                &reader,
                &i,
                &live.path,
                &payload,
                resume_external_id.as_deref(),
                permission_mode,
                effective_timeout,
                |external, identity| {
                    store.bind_continuation_session(run, external)?;
                    crate::durable_runtime::session_bound(external)?;
                    if let Some((pid, created)) = identity {
                        store.bind_owned_process(run, pid, created, &i.executable_sha256)?;
                    }
                    Ok(())
                },
            )?;
            if outcome
                .get("turn_status")
                .and_then(Value::as_str)
                .is_some_and(|s| s != "completed")
                || outcome.get("completed").and_then(Value::as_bool) == Some(false)
            {
                return Err(Error::new("AGENT_TURN_NOT_COMPLETED"));
            }
            Ok(
                json!({"continuation":outcome,"work_item_marked_done":false,"context_content_revalidated_by_model":"not_assumed","native_certification":"not_claimed"}),
            )
        }
        Effect::BillingRead {
            provider,
            credential_resource_id,
            account_alias,
            start_time,
            end_time,
        } => {
            let v = crate::http_provider::billing(
                store,
                &p.project_id,
                &p.environment,
                *provider,
                credential_resource_id,
                account_alias,
                *start_time,
                *end_time,
            )?;
            let category = format!(
                "{}-{}",
                if *provider == BillingProvider::OpenAiCosts {
                    "openai-costs"
                } else {
                    "anthropic-costs"
                },
                &sha(account_alias.as_bytes())[..16]
            );
            store.cache_observation(&p.project_id, &category, &v)?;
            Ok(v)
        }
        Effect::EndpointCheck {
            resource_id,
            resource_digest,
        } => {
            let r = crate::tasks::resource(store, &p.project_id, &p.environment, resource_id)?;
            if sha(&encoded(&r)?) != *resource_digest {
                return Err(Error::new("RESOURCE_CHANGED"));
            }
            let v = crate::http_provider::endpoint(&r)?;
            store.cache_observation(
                &p.project_id,
                &format!("endpoint-{}", &sha(resource_id.as_bytes())[..16]),
                &v,
            )?;
            Ok(v)
        }
        Effect::Repair { target } => crate::repairs::quarantine(store, run, target),
        Effect::RepairUndo {
            execution_id,
            receipt_digest,
        } => {
            let receipt = store.completed_receipt(execution_id, &p.project_id)?;
            if sha(&encoded(&receipt)?) != *receipt_digest {
                return Err(Error::new("RESTORE_RECEIPT_CHANGED"));
            }
            crate::repairs::restore(store, run, &receipt.output)
        }
        Effect::DockerStart {
            integration_id,
            integration_digest,
            desktop_executable,
            desktop_sha256,
        } => {
            let i = verify_profile(store, integration_id, integration_digest, p)?;
            crate::repairs::start_docker(store, run, &i, desktop_executable, desktop_sha256)
        }
    }
}

fn inherited_paths_digest() -> String {
    // Hash the exact inherited path/config values; never read auth tokens or persist these values.
    let mut bytes = Vec::new();
    for key in [
        "PATH",
        "USERPROFILE",
        "HOME",
        "APPDATA",
        "LOCALAPPDATA",
        "CODEX_HOME",
        "CLAUDE_CONFIG_DIR",
        "DOCKER_CONFIG",
        "OP_CONFIG_DIR",
        "DOPPLER_CONFIG_DIR",
        "COPILOT_HOME",
        "GEMINI_CLI_HOME",
        "XDG_DATA_HOME",
        "XDG_CONFIG_HOME",
    ] {
        bytes.extend_from_slice(key.as_bytes());
        bytes.push(0);
        if let Some(v) = std::env::var_os(key) {
            bytes.extend_from_slice(v.as_encoded_bytes());
        }
        bytes.push(0);
    }
    sha(&bytes)
}
