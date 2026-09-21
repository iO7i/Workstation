//! Durable run operations; none is exposed as a writable MCP tool.
use super::{
    control_cli::{answer, parse},
    Response,
};
use clap::Subcommand;
use serde_json::json;
use std::{
    io::Write,
    path::PathBuf,
    time::{Duration, Instant},
};
use workstation_core::durable::VerificationContract;
use workstation_platform::{self as platform, storage::Store, Error, Result};
#[derive(Subcommand)]
pub(super) enum RuntimeAction {
    Status {
        #[arg(long)]
        id: String,
    },
    List {
        #[arg(long)]
        project: String,
    },
    Events {
        #[arg(long)]
        id: String,
        #[arg(long, default_value_t = 0)]
        after: u64,
        #[arg(long, default_value_t = 100)]
        limit: u32,
    },
    Watch {
        #[arg(long)]
        id: String,
        #[arg(long, default_value_t = 30)]
        seconds: u32,
    },
    Cancel {
        #[arg(long)]
        id: String,
        #[arg(long)]
        expected_revision: u64,
    },
    Reconcile {
        #[arg(long)]
        id: String,
        #[arg(long)]
        expected_revision: u64,
        #[arg(long)]
        provider: bool,
        #[arg(long, requires = "provider")]
        approve_sha256: Option<String>,
    },
    Recover {
        #[arg(long)]
        project: String,
    },
    Baseline {
        #[arg(long)]
        id: String,
        #[arg(long)]
        acknowledge_content_hashing: bool,
    },
    Verify {
        #[arg(long)]
        id: String,
        #[arg(long, conflicts_with = "verification_id")]
        contract: Option<PathBuf>,
        #[arg(long)]
        verification_id: Option<String>,
        #[arg(long)]
        approve_sha256: Option<String>,
        #[arg(long)]
        acknowledge_task_execution: bool,
    },
    Detach {
        #[arg(long)]
        id: String,
        #[arg(long)]
        approve_sha256: String,
        #[arg(long)]
        acknowledge_uncertified_execution: bool,
    },
}
pub(super) fn execute(home: &std::path::Path, action: &RuntimeAction) -> Result<Response> {
    // Read paths don't acquire effects.lock or invoke a new scan/adapter.
    match action {
        RuntimeAction::Status { id } => {
            let s = Store::open_readonly(home)?;
            let r = s.durable_run(id)?;
            return answer(json!({"state":r.state(),"run":r,"cached_only":true}));
        }
        RuntimeAction::List { project } => {
            let s = Store::open_readonly(home)?;
            let rows = s.durable_runs(project)?;
            return answer(
                json!({"runs":rows.iter().map(|r|json!({"state":r.state(),"run":r})).collect::<Vec<_>>(),"limit":100,"cached_only":true}),
            );
        }
        RuntimeAction::Events { id, after, limit } => {
            let s = Store::open_readonly(home)?;
            return answer(s.durable_events(id, *after, *limit)?);
        }
        RuntimeAction::Watch { id, seconds } => {
            if *seconds == 0 || *seconds > 300 {
                return Err(Error::new("WATCH_BUDGET_1_TO_300_SECONDS"));
            }
            let s = Store::open_readonly(home)?;
            let start = Instant::now();
            let mut last = None;
            let mut frames = 0;
            while start.elapsed() < Duration::from_secs((*seconds).into()) {
                let r = s.durable_run(id)?;
                if last != Some(r.revision) {
                    let mut out = std::io::stdout().lock();
                    writeln!(
                        out,
                        "{}",
                        json!({"operation":"runtime_watch","state":r.state(),"run":r})
                    )
                    .map_err(|_| Error::new("WATCH_OUTPUT_CLOSED"))?;
                    last = Some(r.revision);
                    frames += 1;
                }
                if r.owner_released || frames >= 256 {
                    break;
                }
                std::thread::sleep(Duration::from_millis(250));
            }
            return answer(json!({"watch_finished":true,"frames":frames,"cached_only":true}));
        }
        _ => {}
    }
    let mut s = Store::open(home)?;
    s.require_v5()?;
    let result = match action {
        RuntimeAction::Cancel {
            id,
            expected_revision,
        } => s.durable_cancel(id, *expected_revision)?,
        RuntimeAction::Recover { project } => {
            platform::durable_runtime::recovery_scan(&mut s, project)?
        }
        RuntimeAction::Reconcile {
            id,
            expected_revision,
            provider,
            approve_sha256,
        } => {
            if s.durable_run(id)?.revision != *expected_revision {
                return Err(Error::new("DURABLE_REVISION_CONFLICT"));
            }
            if *provider {
                if let Some(d) = approve_sha256 {
                    platform::durable_runtime::reconcile_provider(&mut s, id, d)?
                } else {
                    json!({"approve_sha256":platform::durable_runtime::reconcile_approval(&s,id)?,"query":"exact_thread_read_only","run_id":id,"provider_called":false,"approval_lifetime":"current_5_minute_bucket"})
                }
            } else {
                platform::durable_runtime::reconcile_local(&mut s, id, *expected_revision)?
            }
        }
        RuntimeAction::Baseline {
            id,
            acknowledge_content_hashing,
        } => {
            if !acknowledge_content_hashing {
                return Err(Error::new("EXPLICIT_CONTENT_HASHING_ACK_REQUIRED"));
            }
            if s.durable_run(id)?.execution_id.is_some() {
                return Err(Error::new("BASELINE_MUST_PRECEDE_RUN"));
            }
            let m = platform::durable_verify::content_manifest(&s, id)?;
            s.save_durable_baseline(id, &m)?
        }
        RuntimeAction::Verify {
            id,
            contract,
            verification_id,
            approve_sha256,
            acknowledge_task_execution,
        } => {
            if let Some(path) = contract {
                let c: VerificationContract = parse(path)?;
                platform::durable_verify::prepare(&mut s, id, c)?
            } else {
                if !acknowledge_task_execution {
                    return Err(Error::new("EXPLICIT_VERIFICATION_TASK_ACK_REQUIRED"));
                }
                platform::durable_verify::execute(
                    &mut s,
                    id,
                    verification_id
                        .as_deref()
                        .ok_or(Error::new("VERIFICATION_ID_REQUIRED"))?,
                    approve_sha256
                        .as_deref()
                        .ok_or(Error::new("VERIFICATION_APPROVAL_REQUIRED"))?,
                )?
            }
        }
        RuntimeAction::Detach {
            id,
            approve_sha256,
            acknowledge_uncertified_execution,
        } => {
            if !acknowledge_uncertified_execution {
                return Err(Error::new("UNVERIFIED_RELEASE_EXPLICIT_ACK_REQUIRED"));
            }
            let r = s.durable_run(id)?;
            let (p, d) = s.effect_plan(id)?;
            p.approve(&d, approve_sha256, platform::control_store::epoch())
                .map_err(Error::new)?;
            if r.execution_id.is_some() || r.cancellation_requested {
                return Err(Error::new("DURABLE_ALREADY_STARTED_OR_CANCELLED"));
            }
            let exe = std::env::current_exe().map_err(|_| Error::new("EXECUTABLE_PATH"))?;
            let mut cmd = std::process::Command::new(exe);
            cmd.arg("--home")
                .arg(home)
                .arg("--json")
                .args([
                    "effect",
                    "apply",
                    "--id",
                    id,
                    "--approve-sha256",
                    approve_sha256,
                    "--acknowledge-uncertified-execution",
                ])
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .current_dir(home);
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                cmd.creation_flags(0x08000200);
            }
            let mut child = cmd
                .spawn()
                .map_err(|_| Error::new("SUPERVISOR_SPAWN_FAILED"))?;
            let pid = child.id();
            // This explicitly detached supervisor owns its own child Job Object. We do not terminate it on CLI exit.
            std::thread::spawn(move || {
                let _ = child.wait();
            });
            json!({"run_id":id,"supervisor_launch_pid":pid,"state":"launch_requested_not_claimed","inspect_with":"effect runtime status","automatic_retry":false})
        }
        _ => return Err(Error::new("RUNTIME_OPERATION_INVALID")),
    };
    answer(result)
}
