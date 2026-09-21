use super::control_cli::{answer, parse};
use super::{Action, Cli, Response};
use clap::Subcommand;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{io::Read, path::PathBuf};
use workstation_platform::{
    self as platform,
    control_store::{epoch, sha},
    storage::Store,
    Error, Result,
};
#[derive(Subcommand)]
pub(super) enum DiscoveryAction {
    Inventory {
        #[arg(long)]
        project: String,
    },
    AdoptionDraft {
        #[arg(long)]
        project: String,
        #[arg(long)]
        environment: String,
        #[arg(long)]
        resource: String,
        #[arg(long)]
        revision: String,
        #[arg(long)]
        rationale: String,
    },
}
#[derive(Subcommand)]
pub(super) enum AtlasAction {
    Status {
        #[arg(long)]
        project: String,
        #[arg(long)]
        environment: String,
    },
}
#[derive(Subcommand)]
pub(super) enum HealthAction {
    Scan {
        #[arg(long)]
        project: String,
        #[arg(long)]
        deep: bool,
    },
    Latest {
        #[arg(long)]
        project: String,
    },
    Rules,
}
#[derive(Subcommand)]
pub(super) enum WorkspaceAction {
    Scan {
        #[arg(long)]
        project: String,
    },
    Graph {
        #[arg(long)]
        project: String,
    },
    Eligibility {
        #[arg(long)]
        project: String,
        #[arg(long)]
        workspace: String,
    },
    Protect {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        approve_sha256: Option<String>,
    },
    ReleaseProtection {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        approve_sha256: Option<String>,
    },
}
#[derive(Subcommand)]
pub(super) enum SecretAction {
    History {
        #[arg(long)]
        project: String,
        #[arg(long)]
        environment: String,
        #[arg(long)]
        id: String,
    },
    RecoveryStatus {
        #[arg(long)]
        project: String,
        #[arg(long)]
        environment: String,
    },
}
#[derive(Subcommand)]
pub(super) enum JournalAction {
    Inventory {
        #[arg(long)]
        project: String,
    },
}
#[derive(Subcommand)]
pub(super) enum TelemetryAction {
    /// Project-local, preconfigured reported data only. No transcript is persisted.
    Hook {
        #[arg(long)]
        integration: String,
        #[arg(long)]
        format: String,
        #[arg(long)]
        accept_agent_report: bool,
    },
    Preview {
        #[arg(long)]
        format: String,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        account: String,
        #[arg(long)]
        version: String,
        #[arg(long)]
        observed_at: i64,
    },
    Summary {
        #[arg(long)]
        project: String,
    },
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Protection {
    project_id: String,
    workspace_id: String,
    reason: String,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Release {
    project_id: String,
    workspace_id: String,
    protection_id: String,
    reason: String,
}
fn approved<T: Serialize>(v: &T, approval: &Option<String>) -> Result<Option<Response>> {
    let digest = sha(&serde_json::to_vec(v).map_err(|_| Error::new("APPROVAL_ENCODING"))?);
    if let Some(p) = approval {
        if p != &digest {
            return Err(Error::new("APPROVAL_MISMATCH"));
        }
        Ok(None)
    } else {
        Ok(Some(answer(
            json!({"preview":v,"approve_sha256":digest,"changes_applied":false}),
        )?))
    }
}
pub(super) fn handles(a: &Action) -> bool {
    matches!(
        a,
        Action::Discovery { .. }
            | Action::Atlas { .. }
            | Action::Health { .. }
            | Action::Workspace { .. }
            | Action::Secret { .. }
            | Action::Journal { .. }
            | Action::Telemetry { .. }
    )
}
pub(super) fn execute(_cli: &Cli, store: &mut Store, a: &Action) -> Result<Response> {
    match a {
        Action::Discovery { command } => match command {
            DiscoveryAction::Inventory { project } => answer(
                json!({"capabilities":store.available_capabilities(project)?,"source":"cached_explicit_profiles","scan_triggered":false}),
            ),
            DiscoveryAction::AdoptionDraft {
                project,
                environment,
                resource,
                revision,
                rationale,
            } => answer(store.capability_adoption_draft(
                project,
                environment,
                resource,
                revision,
                rationale,
            )?),
        },
        Action::Atlas {
            command:
                AtlasAction::Status {
                    project,
                    environment,
                },
        } => answer(store.atlas_status(project, environment)?),
        Action::Health { command } => match command {
            HealthAction::Scan { project, deep } => {
                answer(platform::health_scan::scan(store, project, *deep)?)
            }
            HealthAction::Latest { project } => {
                answer(store.cached_observation(project, "health-v4")?)
            }
            HealthAction::Rules => {
                let data: Value =
                    serde_json::from_str(include_str!("../../../fingerprints/rules-v1.json"))
                        .map_err(|_| Error::new("RULES_JSON"))?;
                answer(
                    json!({"fingerprints":data,"tested_versions":[],"status":"source_rules_not_vendor_certification"}),
                )
            }
        },
        Action::Workspace { command } => match command {
            WorkspaceAction::Scan { project } => {
                answer(platform::health_scan::workspace_scan(store, project)?)
            }
            WorkspaceAction::Graph { project } => answer(store.graph_context(project)?),
            WorkspaceAction::Eligibility { project, workspace } => {
                let detail = platform::workspace_lifecycle::collect(store, project, workspace)?;
                let context = store.cleanup_context(detail, false)?;
                answer(
                    json!({"eligibility":workstation_core::workspace_policy::cleanup_eligibility(&context,epoch()).map_err(Error::new)?,"cleanup_authorized":false,"prepare":"effect prepare -> operation workspace_cleanup with explicit quiescence acknowledgement"}),
                )
            }
            WorkspaceAction::Protect {
                input,
                approve_sha256,
            } => {
                let p: Protection = parse(input)?;
                if let Some(v) = approved(&p, approve_sha256)? {
                    return Ok(v);
                }
                answer(store.protection(&p.project_id, &p.workspace_id, &p.reason, None)?)
            }
            WorkspaceAction::ReleaseProtection {
                input,
                approve_sha256,
            } => {
                let p: Release = parse(input)?;
                if let Some(v) = approved(&p, approve_sha256)? {
                    return Ok(v);
                }
                answer(store.protection(
                    &p.project_id,
                    &p.workspace_id,
                    &p.reason,
                    Some(&p.protection_id),
                )?)
            }
        },
        Action::Secret { command } => match command {
            SecretAction::History {
                project,
                environment,
                id,
            } => answer(store.secret_history(project, environment, id)?),
            SecretAction::RecoveryStatus {
                project,
                environment,
            } => answer(store.recovery_inventory(project, environment)?),
        },
        Action::Journal {
            command: JournalAction::Inventory { project },
        } => answer(store.archive_inventory(project)?),
        Action::Telemetry { command } => match command {
            TelemetryAction::Summary { project } => answer(store.telemetry_summary(project)?),
            TelemetryAction::Preview {
                format,
                input,
                account,
                version,
                observed_at,
            } => {
                let p: Value = parse(input)?;
                answer(
                    json!({"projection":workstation_core::telemetry::normalize(format,&p,version,account,*observed_at).map_err(Error::new)?,"stored":false,"source":"explicit_import"}),
                )
            }
            TelemetryAction::Hook {
                integration,
                format,
                accept_agent_report,
            } => {
                if !accept_agent_report {
                    return Err(Error::new("PRECONFIGURED_REPORT_ACK_REQUIRED"));
                }
                let i = store.integration(integration)?;
                let mut b = vec![];
                std::io::stdin()
                    .lock()
                    .take(262145)
                    .read_to_end(&mut b)
                    .map_err(|_| Error::new("HOOK_STDIN_FAILED"))?;
                if b.len() > 262144 {
                    return Err(Error::new("HOOK_INPUT_LIMIT"));
                }
                let p: Value = serde_json::from_slice(&b).map_err(|_| Error::new("HOOK_JSON"))?;
                let result = store.ingest_usage_hook(&i, format, &p)?;
                // Intended for statusline pipelines: textual mode emits an empty line, not private JSON.
                let mut response = answer(result)?;
                response.text = String::new();
                Ok(response)
            }
        },
        _ => Err(Error::new("OPERATIONS_COMMAND_UNSUPPORTED")),
    }
}
