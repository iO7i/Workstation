//! Implementation-first interfaces. External effects are opt-in, expiring and journaled.
//! No command in this module is reachable from the read-only MCP tool set.
use super::control_cli::{answer, input, parse};
use super::{Action, Cli, Response};
use clap::{Subcommand, ValueEnum};
use serde::Serialize;
use serde_json::{json, Value};
use std::{io::Read, path::PathBuf};
use workstation_core::{control::*, integrations::*, lifecycle, ownership, usage_import};
use workstation_platform::{
    self as platform,
    control_store::{epoch, sha},
    engine,
    storage::Store,
    Error, Result,
};
#[derive(Subcommand)]
pub(super) enum IntegrationAction {
    Register {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        approve_sha256: Option<String>,
    },
    List {
        #[arg(long)]
        project: String,
    },
    Inspect {
        #[arg(long)]
        id: String,
    },
    /// Describe source capabilities; does not probe or certify the installed agent.
    Compatibility {
        #[arg(long)]
        id: String,
    },
    Capabilities {
        #[arg(long, value_enum)]
        adapter: AdapterArg,
    },
}
#[derive(Clone, Copy, ValueEnum)]
pub(super) enum AdapterArg {
    Codex,
    Claude,
    Cursor,
    Grok,
    Gemini,
    Copilot,
    Opencode,
    Cline,
    Roo,
    Windsurf,
    Doppler,
    Onepassword,
    Docker,
    Worktrunk,
    Entire,
}
impl From<AdapterArg> for Adapter {
    fn from(a: AdapterArg) -> Self {
        match a {
            AdapterArg::Codex => Self::Codex,
            AdapterArg::Claude => Self::Claude,
            AdapterArg::Cursor => Self::CursorAcp,
            AdapterArg::Grok => Self::GrokAcp,
            AdapterArg::Gemini => Self::GeminiAcp,
            AdapterArg::Copilot => Self::Copilot,
            AdapterArg::Opencode => Self::OpenCode,
            AdapterArg::Cline => Self::Cline,
            AdapterArg::Roo => Self::Roo,
            AdapterArg::Windsurf => Self::Windsurf,
            AdapterArg::Doppler => Self::Doppler,
            AdapterArg::Onepassword => Self::OnePassword,
            AdapterArg::Docker => Self::Docker,
            AdapterArg::Worktrunk => Self::Worktrunk,
            AdapterArg::Entire => Self::Entire,
        }
    }
}
#[derive(Subcommand)]
pub(super) enum TaskAction {
    Register {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        approve_sha256: Option<String>,
    },
    List {
        #[arg(long)]
        project: String,
    },
}
#[derive(Subcommand)]
pub(super) enum EffectAction {
    /// Durable execution status, cancellation, reconciliation and verification.
    Runtime {
        #[command(subcommand)]
        command: super::runtime_cli::RuntimeAction,
    },
    /// Write an expiring exact plan. Preparation may inspect pinned local targets, never executes the effect.
    Prepare {
        #[arg(long)]
        project: String,
        #[arg(long)]
        environment: String,
        #[arg(long)]
        input: PathBuf,
    },
    Show {
        #[arg(long)]
        id: String,
    },
    /// Only after local review. Source is not certified; explicit prerelease acknowledgement required.
    Apply {
        #[arg(long)]
        id: String,
        #[arg(long)]
        approve_sha256: String,
        #[arg(long)]
        acknowledge_uncertified_execution: bool,
    },
    History {
        #[arg(long)]
        project: String,
    },
}
#[derive(Subcommand)]
pub(super) enum LifecycleAction {
    Import {
        #[arg(long)]
        project: String,
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        format: String,
        #[arg(long)]
        source_id: String,
        #[arg(long)]
        version: String,
        #[arg(long)]
        observed_at: i64,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        approve_sha256: Option<String>,
    },
    /// A preconfigured registered agent can write metadata-only reports through stdin, not MCP.
    Hook {
        #[arg(long)]
        integration: String,
        #[arg(long)]
        workspace: Option<String>,
        #[arg(long)]
        format: String,
        #[arg(long)]
        accept_agent_report: bool,
    },
    Renew {
        #[arg(long)]
        assignment: String,
        #[arg(long)]
        session: String,
        #[arg(long)]
        seconds: u32,
        #[arg(long)]
        approve_sha256: Option<String>,
    },
}
#[derive(Subcommand)]
pub(super) enum ChronicleAction {
    History {
        #[arg(long)]
        project: String,
        #[arg(long)]
        topic: Option<String>,
        #[arg(long)]
        known_at: Option<i64>,
    },
    Compare {
        #[arg(long)]
        project: String,
        #[arg(long)]
        before: i64,
        #[arg(long)]
        after: i64,
        #[arg(long)]
        known_at: Option<i64>,
    },
    /// Export accepted decisions with source time/evidence; no new authority is created.
    Export {
        #[arg(long)]
        project: String,
        #[arg(long)]
        valid_at: Option<i64>,
        #[arg(long)]
        known_at: Option<i64>,
    },
}
fn bytes<T: Serialize>(x: &T) -> Result<Vec<u8>> {
    serde_json::to_vec(x).map_err(|_| Error::new("SERIALIZE_FAILED"))
}
fn preview<T: Serialize>(x: &T, approved: &Option<String>) -> Result<Option<Value>> {
    let digest = sha(&bytes(x)?);
    if let Some(a) = approved {
        if a != &digest {
            return Err(Error::new("CANONICAL_INPUT_APPROVAL_MISMATCH"));
        }
        Ok(None)
    } else {
        Ok(Some(
            json!({"status":"preview_only","canonical_sha256":digest,"review":"Review the source input locally; this preview does not echo potentially sensitive values.","executed":false,"approval_not_same_user_security_boundary":true}),
        ))
    }
}
pub(super) fn handled(a: &Action) -> bool {
    matches!(
        a,
        Action::Integration { .. }
            | Action::Task { .. }
            | Action::Effect { .. }
            | Action::Lifecycle { .. }
            | Action::Chronicle { .. }
            | Action::Manifest { .. }
            | Action::Ownership { .. }
            | Action::UsageImport { .. }
    )
}
pub(super) fn execute(cli: &Cli) -> Result<Response> {
    if let Action::Integration {
        command: IntegrationAction::Capabilities { adapter },
    } = &cli.command
    {
        return answer(json!(capabilities((*adapter).into())));
    }
    if let Action::Ownership { input: source } = &cli.command {
        let data: ownership::OwnershipInput = parse(source)?;
        return answer(
            json!({"rows":ownership::correlate(&data).map_err(Error::new)?,"input_provenance":"explicit_report_not_authenticated_host_collection","authorizes_termination":false}),
        );
    }
    if let Action::UsageImport {
        format,
        input: source,
        account,
        version,
        observed_at,
    } = &cli.command
    {
        let raw = input(source)?;
        let at = observed_at.unwrap_or_else(epoch);
        let data = match format.as_str() {
            "quota-csv" => {
                let csv = std::str::from_utf8(&raw).map_err(|_| Error::new("CSV_ENCODING"))?;
                json!({"samples":usage_import::quota_csv(csv,at).map_err(Error::new)?})
            }
            "copilot-sdk" => {
                let v: Value =
                    serde_json::from_slice(&raw).map_err(|_| Error::new("INPUT_JSON"))?;
                json!({"samples":usage_import::copilot_quota(&v,account.as_deref().ok_or(Error::new("ACCOUNT_ALIAS_REQUIRED"))?,at,version.as_deref().ok_or(Error::new("SOURCE_VERSION_REQUIRED"))?).map_err(Error::new)?})
            }
            "openai-costs" | "anthropic-costs" => {
                let v: Value =
                    serde_json::from_slice(&raw).map_err(|_| Error::new("INPUT_JSON"))?;
                let p = if format == "openai-costs" {
                    "openai"
                } else {
                    "anthropic"
                };
                usage_import::cost_summary(&usage_import::cost_page(p, &v).map_err(Error::new)?)
                    .map_err(Error::new)?
            }
            _ => return Err(Error::new("USAGE_IMPORT_FORMAT_UNSUPPORTED")),
        };
        return answer(
            json!({"data":data,"coverage":"partial","source":"explicit_import_not_live_provider","persisted":false}),
        );
    }
    let home = platform::paths::select_home(cli.home.clone(), None)?;
    if let Action::Effect {
        command: EffectAction::Runtime { command },
    } = &cli.command
    {
        return super::runtime_cli::execute(&home, command);
    }
    let mut store = Store::open(&home)?;
    match &cli.command {
        Action::Integration { command } => match command {
            IntegrationAction::Register {
                input: source,
                approve_sha256,
            } => {
                let i: Integration = parse(source)?;
                i.validate().map_err(Error::new)?;
                if let Some(p) = preview(&i, approve_sha256)? {
                    answer(p)
                } else {
                    answer(
                        store.register_integration(
                            &i,
                            approve_sha256
                                .as_deref()
                                .ok_or(Error::new("APPROVAL_REQUIRED"))?,
                        )?,
                    )
                }
            }
            IntegrationAction::List { project } => answer(store.integration_summary(project)?),
            IntegrationAction::Compatibility { id } => {
                answer(platform::durable_compat::inspect(&store, id)?)
            }
            IntegrationAction::Inspect { id } => {
                let i = store.integration(id)?;
                let observed: Value = super::collect(
                    &store.home,
                    workstation_core::WorkerRequest::AgentProfile {
                        integration: i.clone(),
                    },
                    std::time::Duration::from_secs(5),
                )?;
                store.cache_observation(
                    &i.project_id,
                    &format!("profile-{}", &sha(i.id.as_bytes())[..16]),
                    &observed,
                )?;
                answer(observed)
            }
            _ => Err(Error::new("UNSUPPORTED_INTEGRATION_OPERATION")),
        },
        Action::Task { command } => match command {
            TaskAction::Register {
                input: source,
                approve_sha256,
            } => {
                let t: ApprovedTask = parse(source)?;
                t.validate().map_err(Error::new)?;
                if let Some(p) = preview(&t, approve_sha256)? {
                    answer(p)
                } else {
                    answer(
                        store.register_task(
                            &t,
                            approve_sha256
                                .as_deref()
                                .ok_or(Error::new("APPROVAL_REQUIRED"))?,
                        )?,
                    )
                }
            }
            TaskAction::List { project } => answer(store.task_inventory(project)?),
        },
        Action::Effect { command } => match command {
            EffectAction::Prepare {
                project,
                environment,
                input: source,
            } => {
                let p: engine::Prepare = parse(source)?;
                answer(engine::prepare(&mut store, project, environment, p)?)
            }
            EffectAction::Show { id } => {
                let (p, d) = store.effect_plan(id)?;
                answer(json!({"plan":p,"approve_sha256":d,"executed":false}))
            }
            EffectAction::Apply {
                id,
                approve_sha256,
                acknowledge_uncertified_execution,
            } => {
                let receipt = engine::apply(
                    &mut store,
                    id,
                    approve_sha256,
                    *acknowledge_uncertified_execution,
                )?;
                let success = receipt.state == workstation_core::effects::ExecutionState::Succeeded;
                let mut response =
                    answer(json!({"receipt":receipt,"certification":"not_claimed"}))?;
                if !success {
                    response.code = 2;
                    response.coverage = workstation_core::Coverage::Partial;
                }
                Ok(response)
            }
            EffectAction::History { project } => answer(store.effect_history(project)?),
            EffectAction::Runtime { .. } => Err(Error::new("RUNTIME_DISPATCH_REQUIRED")),
        },
        Action::Lifecycle { command } => match command {
            LifecycleAction::Import {
                project,
                workspace,
                format,
                source_id,
                version,
                observed_at,
                input: source,
                approve_sha256,
            } => {
                let payload: Value = parse(source)?;
                let event = lifecycle::normalize(
                    format,
                    &payload,
                    source_id,
                    project,
                    workspace.clone(),
                    *observed_at,
                    version,
                )
                .map_err(Error::new)?;
                if let Some(p) = preview(&event, approve_sha256)? {
                    answer(p)
                } else {
                    answer(store.ingest_lifecycle(&event)?)
                }
            }
            LifecycleAction::Hook {
                integration,
                workspace,
                format,
                accept_agent_report,
            } => {
                if !accept_agent_report {
                    return Err(Error::new("AGENT_REPORT_ACK_REQUIRED"));
                }
                let i = store.integration(integration)?;
                let supported = matches!(
                    (i.adapter, format.as_str()),
                    (Adapter::Claude, "claude-hook")
                        | (Adapter::Codex, "codex-notification")
                        | (Adapter::CursorAcp, "acp-notification")
                        | (Adapter::GrokAcp, "acp-notification")
                        | (Adapter::GeminiAcp, "gemini-hook")
                        | (Adapter::GeminiAcp, "acp-notification")
                        | (Adapter::OpenCode, "acp-notification")
                        | (Adapter::Cline, "acp-notification")
                        | (Adapter::Copilot, "copilot-session-event")
                );
                if !supported {
                    return Err(Error::new("PROFILE_LIFECYCLE_FORMAT_MISMATCH"));
                }
                let mut b = vec![];
                std::io::stdin()
                    .lock()
                    .take(INPUT_LIMIT as u64 + 1)
                    .read_to_end(&mut b)
                    .map_err(|_| Error::new("HOOK_INPUT_READ"))?;
                if b.len() > INPUT_LIMIT {
                    return Err(Error::new("HOOK_INPUT_LIMIT"));
                }
                let v: Value =
                    serde_json::from_slice(&b).map_err(|_| Error::new("HOOK_INPUT_JSON"))?;
                let mut e = lifecycle::normalize(
                    format,
                    &v,
                    &platform::new_id(),
                    &i.project_id,
                    workspace.clone(),
                    epoch(),
                    &i.version_text,
                )
                .map_err(Error::new)?;
                e.agent = i.adapter.id().into();
                let result = store.ingest_lifecycle(&e)?;
                let observed =
                    match platform::health_scan::observe_hook_ancestry(&mut store, &i, &e) {
                        Ok(x) => x,
                        Err(e) => json!({"coverage":"partial","error_code":e.code}),
                    };
                answer(
                    json!({"metadata_recorded":result,"ancestry_observation":observed,"prompts_copied":false,"authority":"agent_reported_not_accepted_decision"}),
                )
            }
            LifecycleAction::Renew {
                assignment,
                session,
                seconds,
                approve_sha256,
            } => {
                let intent = json!({"assignment":assignment,"session":session,"seconds":seconds});
                if let Some(p) = preview(&intent, approve_sha256)? {
                    answer(p)
                } else {
                    answer(store.renew_lease(assignment, session, *seconds)?)
                }
            }
        },
        Action::Manifest {
            input: source,
            approve_sha256,
        } => {
            let m: platform::project_tools::ProjectManifest = parse(source)?;
            m.validate()?;
            if let Some(p) = preview(&m, approve_sha256)? {
                answer(p)
            } else {
                answer(
                    store.import_manifest(
                        &m,
                        approve_sha256
                            .as_deref()
                            .ok_or(Error::new("APPROVAL_REQUIRED"))?,
                    )?,
                )
            }
        }
        Action::Chronicle { command } => match command {
            ChronicleAction::History {
                project,
                topic,
                known_at,
            } => answer(store.decision_history(
                project,
                topic.as_deref(),
                known_at.unwrap_or_else(epoch),
            )?),
            ChronicleAction::Compare {
                project,
                before,
                after,
                known_at,
            } => answer(store.compare_decisions(
                project,
                *before,
                *after,
                known_at.unwrap_or_else(epoch),
            )?),
            ChronicleAction::Export {
                project,
                valid_at,
                known_at,
            } => {
                let at = epoch();
                let valid = valid_at.unwrap_or(at);
                let known = known_at.unwrap_or(at);
                let decisions = store.decisions(project, valid, known)?;
                let mut markdown=format!("# Accepted decisions\n\nProject: {}\nEffective time: {valid}\nKnown at: {known}\n\n",workstation_core::render::terminal_safe(project));
                for d in &decisions {
                    markdown.push_str(&format!("## {} — {}\n\n{}\n\nRationale: {}\n\nPredecessor: {:?}\nEffective: {} | Recorded: {}\n\n",d.id,d.topic,d.statement,d.rationale,d.predecessor,d.effective_at,d.recorded_at));
                }
                let mut response = answer(
                    json!({"project":project,"valid_at":valid,"known_at":known,"decisions":decisions,"export_creates_no_authority":true}),
                )?;
                response.text = workstation_core::render::terminal_safe(&markdown);
                Ok(response)
            }
        },
        _ => Err(Error::new("ADVANCED_OPERATION_UNSUPPORTED")),
    }
}
