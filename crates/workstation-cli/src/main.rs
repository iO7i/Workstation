mod advanced_cli;
mod control_cli;
mod mcp;
mod operations_cli;
mod runtime_cli;
use clap::{Parser, Subcommand, ValueEnum};
use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use workstation_core::*;
use workstation_platform::{
    self as platform, filesystem, git, paths, runner, storage::Store, Error, Result,
};

#[derive(Parser)]
#[command(
    name = "workstation",
    version,
    about = "Local Workstation control-plane source candidate: seven modules, protected workspaces, no automatic vendor actions."
)]
struct Cli {
    #[arg(long, global = true, env = "WORKSTATION_HOME")]
    home: Option<PathBuf>,
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Action,
}
#[derive(Subcommand)]
enum Action {
    Discovery {
        #[command(subcommand)]
        command: operations_cli::DiscoveryAction,
    },
    Atlas {
        #[command(subcommand)]
        command: operations_cli::AtlasAction,
    },
    Health {
        #[command(subcommand)]
        command: operations_cli::HealthAction,
    },
    Workspace {
        #[command(subcommand)]
        command: operations_cli::WorkspaceAction,
    },
    Secret {
        #[command(subcommand)]
        command: operations_cli::SecretAction,
    },
    Journal {
        #[command(subcommand)]
        command: operations_cli::JournalAction,
    },
    Telemetry {
        #[command(subcommand)]
        command: operations_cli::TelemetryAction,
    },
    /// Registered explicit executable/version/config profiles. No auto-install or global PATH discovery.
    Integration {
        #[command(subcommand)]
        command: advanced_cli::IntegrationAction,
    },
    Task {
        #[command(subcommand)]
        command: advanced_cli::TaskAction,
    },
    Effect {
        #[command(subcommand)]
        command: advanced_cli::EffectAction,
    },
    Lifecycle {
        #[command(subcommand)]
        command: advanced_cli::LifecycleAction,
    },
    Chronicle {
        #[command(subcommand)]
        command: advanced_cli::ChronicleAction,
    },
    Manifest {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        approve_sha256: Option<String>,
    },
    /// Explain declared birth-qualified associations; no process termination authority.
    Ownership {
        #[arg(long)]
        input: PathBuf,
    },
    /// Explicit supported CSV/SDK/cost import normalization only; no account access.
    UsageImport {
        #[arg(long)]
        format: String,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        account: Option<String>,
        #[arg(long)]
        version: Option<String>,
        #[arg(long)]
        observed_at: Option<i64>,
    },

    /// Back up and migrate the existing R0 database explicitly; never reset it.
    Upgrade,
    /// Restore a checksum-approved metadata backup into a NEW home only.
    Restore {
        #[arg(long)]
        backup: PathBuf,
        #[arg(long)]
        sha256: String,
    },
    /// Preview or approve typed changes to Workstation metadata. No arbitrary execution.
    Record {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        approve_sha256: Option<String>,
    },
    /// Bounded cached project context; no scan, model call or online search.
    Context {
        #[arg(long)]
        project: String,
        #[arg(long)]
        environment: Option<String>,
        #[arg(long, conflicts_with = "share")]
        html: bool,
        #[arg(long)]
        share: bool,
    },
    Timeline {
        #[arg(long)]
        project: String,
        #[arg(long)]
        before: Option<i64>,
    },
    Roster {
        #[arg(long)]
        project: String,
    },
    Decisions {
        #[arg(long)]
        project: String,
        #[arg(long)]
        valid_at: Option<i64>,
        #[arg(long)]
        known_at: Option<i64>,
    },
    Resources {
        #[arg(long)]
        project: String,
        #[arg(long)]
        environment: String,
    },
    Economics {
        #[arg(long)]
        project: String,
        #[arg(long)]
        at: Option<i64>,
    },
    /// Normalize an explicitly supplied supported wire format. Does not call a provider.
    NormalizeUsage {
        #[arg(long)]
        format: String,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        version: String,
        #[arg(long)]
        account: String,
        #[arg(long)]
        observed_at: i64,
    },
    Runway {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        at: Option<i64>,
    },
    Simulate {
        #[arg(long)]
        input: PathBuf,
    },
    ModelFrontier {
        #[arg(long)]
        input: PathBuf,
    },
    SelectModels {
        #[arg(long)]
        input: PathBuf,
    },
    /// Classify explicitly imported symptoms; never claims live certification or permits repair.
    DiagnoseEvidence {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        at: Option<i64>,
    },
    PlanFit {
        #[arg(long)]
        input: PathBuf,
    },
    Capabilities {
        #[arg(long)]
        project: String,
    },
    WorkspaceRegister {
        #[arg(long)]
        project: String,
        #[arg(long)]
        id: String,
        #[arg(long)]
        path: PathBuf,
    },
    Checkpoint {
        #[arg(long)]
        work: String,
        #[arg(long)]
        input: PathBuf,
    },
    Handoff {
        #[arg(long)]
        checkpoint: String,
        #[arg(long)]
        target: String,
    },
    /// Read-only stdio MCP, fixed project/environment scope. No secret values or mutations.
    Mcp {
        #[arg(long)]
        project: String,
        #[arg(long)]
        environment: Option<String>,
    },
    /// Windows console-only entry into user-scoped DPAPI; no reveal or generic run command.
    VaultPut {
        #[arg(long)]
        project: String,
        #[arg(long)]
        environment: String,
        #[arg(long)]
        id: String,
    },
    /// Initialize a NEW data directory. Its parent must already exist.
    Init,
    /// Suggest known storage roots. Does not register or scan them.
    Discover,
    /// Register/list explicit storage roots. Only Workstation metadata is changed.
    Root {
        #[command(subcommand)]
        command: RootAction,
    },
    /// Register/list repositories for worktree-registration inspection.
    Project {
        #[command(subcommand)]
        command: ProjectAction,
    },
    /// Bounded local observation. Saves at most 20 snapshots in Workstation's own DB.
    Doctor {
        #[arg(long)]
        deep: bool,
        #[arg(long)]
        no_save: bool,
        #[arg(long, value_enum)]
        fail_on: Option<Threshold>,
    },
    /// Read cached evidence; never presents it as live health.
    Report {
        #[arg(long, conflicts_with = "share")]
        html: bool,
        #[arg(long)]
        share: bool,
    },
    /// Consistent backup of Workstation's own metadata only.
    Backup,
    /// Replay a bounded synthetic incident record; does not diagnose the live machine.
    ReplayFixture { path: PathBuf },
    /// Actual linked SQLite runtime and build/target information.
    BuildInfo,
}
#[derive(Subcommand)]
enum RootAction {
    Add {
        #[arg(long)]
        id: String,
        #[arg(long)]
        path: PathBuf,
        #[arg(long, value_enum)]
        kind: Kind,
    },
    List,
}
#[derive(Subcommand)]
enum ProjectAction {
    Add {
        path: PathBuf,
        #[arg(long)]
        id: String,
        #[arg(long)]
        git: PathBuf,
        /// Acknowledges invoking the explicitly selected Git binary in this repository.
        #[arg(long)]
        trust_repository: bool,
    },
    List,
}
#[derive(Clone, Copy, ValueEnum)]
enum Kind {
    Codex,
    Claude,
    Cursor,
    Vscode,
    Docker,
    Workspace,
    Other,
}
impl From<Kind> for RootKind {
    fn from(k: Kind) -> Self {
        match k {
            Kind::Codex => Self::Codex,
            Kind::Claude => Self::Claude,
            Kind::Cursor => Self::Cursor,
            Kind::Vscode => Self::Vscode,
            Kind::Docker => Self::Docker,
            Kind::Workspace => Self::Workspace,
            Kind::Other => Self::Other,
        }
    }
}
#[derive(Clone, Copy, ValueEnum)]
enum Threshold {
    Warning,
    Critical,
}
struct Response {
    data: Value,
    text: String,
    coverage: Coverage,
    code: u8,
    warnings: Vec<String>,
}
fn ok(data: Value, text: String) -> Response {
    Response {
        data,
        text,
        coverage: Coverage::Complete,
        code: 0,
        warnings: vec![],
    }
}
fn value<T: serde::Serialize>(v: &T) -> Result<Value> {
    serde_json::to_value(v).map_err(|_| Error::new("SERIALIZE_FAILED"))
}
fn own_exe() -> Result<PathBuf> {
    std::env::current_exe().map_err(|_| Error::new("EXECUTABLE_UNAVAILABLE"))
}
fn collect<T: serde::de::DeserializeOwned>(
    home: &Path,
    request: WorkerRequest,
    duration: Duration,
) -> Result<T> {
    let out = runner::collect(&own_exe()?, home, &request, duration)?;
    if out.exit_code != 0 {
        return Err(Error::new("WORKER_EXIT_FAILED"));
    }
    serde_json::from_slice(&out.bytes).map_err(|_| Error::new("WORKER_RESPONSE_INVALID"))
}
fn coverage_for(e: &Error) -> Coverage {
    if e.code == "WORKER_TIMEOUT" {
        Coverage::TimedOut
    } else {
        Coverage::Denied
    }
}

fn doctor(
    store: &mut Store,
    deep: bool,
    save: bool,
    fail_on: Option<Threshold>,
) -> Result<Response> {
    store.health()?;
    let previous = store.latest()?;
    let start = Instant::now();
    let overall = if deep {
        Duration::from_secs(120)
    } else {
        Duration::from_secs(10)
    };
    let remaining = || overall.saturating_sub(start.elapsed());
    let mut s = Snapshot {
        schema_version: SCHEMA_VERSION.into(),
        run_id: platform::new_id(),
        observed_at: platform::now(),
        installation_id: store.config.installation_id.clone(),
        platform: std::env::consts::OS.into(),
        coverage: Coverage::Complete,
        storage: vec![],
        disks: vec![],
        processes: ProcessSnapshot {
            observed_at: platform::now(),
            coverage: Coverage::TimedOut,
            enumerated_count: 0,
            selected: vec![],
            notes: vec!["OVERALL_BUDGET".into()],
        },
        workspaces: vec![],
        findings: vec![],
        not_inspected: vec![
            "session ownership".into(),
            "worktree contents and unique commits".into(),
            "Docker API and WSL state".into(),
            "secrets and transcripts".into(),
            "vendor diagnostics and updates".into(),
            "physical allocation and hard-link deduplication".into(),
        ],
        saved: false,
    };
    s.disks.push(platform::windows::disk(&store.home));
    #[cfg(windows)]
    {
        let system = PathBuf::from(format!(
            "{}\\",
            std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into())
        ));
        s.disks.push(platform::windows::disk(&system));
    }
    s.processes = match collect(
        &store.home,
        WorkerRequest::Processes,
        Duration::from_secs(2).min(remaining()),
    ) {
        Ok(p) => p,
        Err(e) => ProcessSnapshot {
            observed_at: platform::now(),
            coverage: coverage_for(&e),
            enumerated_count: 0,
            selected: vec![],
            notes: vec![e.code.into()],
        },
    };
    for root in store.roots()? {
        let mut observed = if remaining().is_zero() {
            filesystem::unavailable(root.clone(), Coverage::TimedOut, "OVERALL_BUDGET")
        } else {
            let budget = if deep {
                ScanBudget::deep()
            } else {
                ScanBudget::normal()
            };
            let duration = Duration::from_millis(budget.deadline_ms + 300).min(remaining());
            match collect(
                &store.home,
                WorkerRequest::Storage {
                    root: root.clone(),
                    budget,
                },
                duration,
            ) {
                Ok(r) => r,
                Err(e) => filesystem::unavailable(root.clone(), coverage_for(&e), e.code),
            }
        };
        if let Some(old) = previous
            .as_ref()
            .and_then(|p| p.storage.iter().find(|r| r.root.id == root.id))
        {
            observed.delta_since_previous_complete_bytes =
                workstation_core::policy::storage_delta(&observed, old);
        }
        s.storage.push(observed);
    }
    for project in store.projects()? {
        let record = if remaining().is_zero() {
            WorkspaceObservation {
                project_id: project.id.clone(),
                observed_at: platform::now(),
                coverage: Coverage::TimedOut,
                worktrees: vec![],
                notes: vec!["OVERALL_BUDGET".into()],
            }
        } else {
            match collect(
                &store.home,
                WorkerRequest::Worktrees {
                    project: project.clone(),
                },
                Duration::from_secs(3).min(remaining()),
            ) {
                Ok(w) => w,
                Err(e) => WorkspaceObservation {
                    project_id: project.id.clone(),
                    observed_at: platform::now(),
                    coverage: coverage_for(&e),
                    worktrees: vec![],
                    notes: vec![e.code.into()],
                },
            }
        };
        s.workspaces.push(record);
    }
    s.coverage = workstation_core::policy::summarize_coverage(
        s.storage
            .iter()
            .map(|x| x.coverage)
            .chain(s.disks.iter().map(|x| x.coverage))
            .chain(s.workspaces.iter().map(|x| x.coverage))
            .chain([s.processes.coverage]),
    );
    s.findings = workstation_core::policy::diagnose(&s);
    let mut warnings = vec![];
    if s.storage.is_empty() {
        warnings.push("NO_STORAGE_ROOTS_REGISTERED".into());
    }
    if s.workspaces.is_empty() {
        warnings.push("NO_PROJECTS_REGISTERED".into());
    }
    if save {
        s.saved = true;
        if let Err(e) = store.save(&s) {
            s.saved = false;
            s.coverage = Coverage::Partial;
            warnings.push(e.code.into());
        }
    }
    let threshold = fail_on.map(|t| match t {
        Threshold::Warning => Severity::Warning,
        Threshold::Critical => Severity::Critical,
    });
    let code = workstation_core::policy::exit_code(s.coverage, &s.findings, threshold);
    Ok(Response {
        data: value(&s)?,
        text: workstation_core::render::human_report(&s),
        coverage: s.coverage,
        code,
        warnings,
    })
}

fn execute(cli: &Cli) -> Result<Response> {
    if operations_cli::handles(&cli.command) {
        let home = paths::select_home(cli.home.clone(), None)?;
        let mut store = Store::open(&home)?;
        return operations_cli::execute(cli, &mut store, &cli.command);
    }
    if advanced_cli::handled(&cli.command) {
        return advanced_cli::execute(cli);
    }
    if control_cli::handled(&cli.command) {
        return control_cli::execute(cli);
    }
    match &cli.command {
        Action::BuildInfo => {
            return Ok(ok(
                json!({"version": env!("CARGO_PKG_VERSION"), "os": std::env::consts::OS,
            "architecture": std::env::consts::ARCH, "sqlite_version": platform::storage::linked_sqlite_version(),
            "windows_certification": "not_implied_by_build", "network_required_for_runtime": false}),
                "Use --json for build details.\n".into(),
            ))
        }
        Action::ReplayFixture { path } => {
            let mut bytes = Vec::new();
            std::fs::File::open(path)
                .map_err(|_| Error::new("FIXTURE_UNAVAILABLE"))?
                .take(64 * 1024 + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| Error::new("FIXTURE_UNREADABLE"))?;
            if bytes.len() > 64 * 1024 {
                return Err(Error::new("FIXTURE_TOO_LARGE"));
            }
            let i: IncidentInput =
                serde_json::from_slice(&bytes).map_err(|_| Error::new("FIXTURE_SCHEMA_INVALID"))?;
            let finding = workstation_core::policy::classify_incident(&i);
            return Ok(ok(
                json!({"synthetic_replay": true, "finding": finding}),
                "Synthetic incident replay only; no live diagnosis or repair.\n".into(),
            ));
        }
        Action::Discover => {
            let candidates = filesystem::discover();
            return Ok(ok(value(&candidates)?, format!("Storage-root candidates, not proven active homes:\n{}\nRegister only the roots you approve.\n",
                candidates.iter().map(|r| format!("{}: {}", r.id, workstation_core::render::terminal_safe(&r.path.to_string_lossy()))).collect::<Vec<_>>().join("\n"))));
        }
        Action::Init => {
            let home = cli
                .home
                .as_ref()
                .ok_or_else(|| Error::new("EXPLICIT_HOME_REQUIRED"))?;
            let s = Store::init(home)?;
            let health = s.health()?;
            return Ok(ok(json!({"config":s.config,"database":health}),
                format!("Initialized {}\nNo profile locator, startup task, or third-party setting was changed.\nUse --home or WORKSTATION_HOME on subsequent commands.\n", workstation_core::render::terminal_safe(&home.to_string_lossy()))));
        }
        _ => {}
    }
    let home = paths::select_home(cli.home.clone(), None)?;
    let mut store = Store::open(&home)?;
    match &cli.command {
        Action::Root {
            command: RootAction::Add { id, path, kind },
        } => {
            let root = Root {
                id: id.clone(),
                path: path.clone(),
                kind: (*kind).into(),
            };
            store.register_root(&root)?;
            Ok(ok(
                value(&root)?,
                "Registered storage root; no files scanned or changed.\n".into(),
            ))
        }
        Action::Root {
            command: RootAction::List,
        } => {
            let roots = store.roots()?;
            Ok(ok(
                value(&roots)?,
                serde_json::to_string_pretty(&roots).map_err(|_| Error::new("SERIALIZE_FAILED"))?,
            ))
        }
        Action::Project {
            command:
                ProjectAction::Add {
                    path,
                    id,
                    git,
                    trust_repository,
                },
        } => {
            if !trust_repository {
                return Err(Error::new("EXPLICIT_REPOSITORY_TRUST_REQUIRED"));
            }
            let p = Project {
                id: id.clone(),
                path: path.clone(),
                git_executable: git.clone(),
            };
            let result: WorkspaceObservation = collect(
                &home,
                WorkerRequest::Worktrees { project: p.clone() },
                Duration::from_secs(3),
            )?;
            if !result.coverage.is_complete() || result.worktrees.is_empty() {
                return Err(Error::new("WORKTREE_REGISTRATION_UNVERIFIED"));
            }
            store.register_project(&p)?;
            Ok(ok(
                value(&p)?,
                "Registered repository; worktrees remain protected.\n".into(),
            ))
        }
        Action::Project {
            command: ProjectAction::List,
        } => {
            let projects = store.projects()?;
            Ok(ok(
                value(&projects)?,
                serde_json::to_string_pretty(&projects)
                    .map_err(|_| Error::new("SERIALIZE_FAILED"))?,
            ))
        }
        Action::Doctor {
            deep,
            no_save,
            fail_on,
        } => doctor(&mut store, *deep, !*no_save, *fail_on),
        Action::Report { html, share } => {
            let s = store
                .latest()?
                .ok_or_else(|| Error::new("NO_SAVED_SNAPSHOT"))?;
            if *share {
                let report = workstation_core::render::share_report(&s);
                Ok(Response {
                    data: value(&report)?,
                    text: serde_json::to_string_pretty(&report)
                        .map_err(|_| Error::new("SERIALIZE_FAILED"))?,
                    coverage: s.coverage,
                    code: 0,
                    warnings: vec![
                        "CACHED_NOT_LIVE".into(),
                        "SHARE_EXPORT_REVIEW_BEFORE_PUBLISHING".into(),
                    ],
                })
            } else {
                let text = if *html {
                    workstation_core::render::html_report(&s)
                } else {
                    workstation_core::render::human_report(&s)
                };
                Ok(Response {
                    data: value(&s)?,
                    text,
                    coverage: s.coverage,
                    code: 0,
                    warnings: vec!["CACHED_NOT_LIVE".into()],
                })
            }
        }
        Action::Backup => {
            let path = store.backup()?;
            Ok(ok(
                json!({"backup":path}),
                "Metadata backup completed and integrity-checked.\n".into(),
            ))
        }
        _ => Err(Error::new("UNSUPPORTED_OPERATION")),
    }
}

fn worker() -> Result<()> {
    let mut bytes = Vec::new();
    std::io::stdin()
        .take(64 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::new("WORKER_STDIN_FAILED"))?;
    if bytes.len() > 64 * 1024 {
        return Err(Error::new("REQUEST_TOO_LARGE"));
    }
    let request: WorkerRequest =
        serde_json::from_slice(&bytes).map_err(|_| Error::new("WORKER_REQUEST_INVALID"))?;
    let output = match request {
        WorkerRequest::Storage { root, budget } => value(&filesystem::scan(root, budget))?,
        WorkerRequest::HostGraph {
            host,
            boot,
            profiles,
        } => value(&platform::process_graph::collect(&host, &boot, &profiles)?)?,
        WorkerRequest::WorkspaceDetail {
            project,
            workspace_id,
            path,
        } => value(&platform::workspace_lifecycle::inspect(
            &project,
            &workspace_id,
            &path,
        )?)?,
        WorkerRequest::Processes => value(&platform::windows::processes())?,
        WorkerRequest::Worktrees { project } => value(&git::collect(project))?,
        WorkerRequest::WorkspaceStamp {
            project,
            workspace_id,
            path,
        } => value(&platform::control_workspace::capture(
            project,
            workspace_id,
            path,
        )?)?,
        WorkerRequest::AgentProfile { integration } => {
            platform::profile_health::inspect(&integration)?
        }
        WorkerRequest::Fixture { mode } => {
            if !cfg!(debug_assertions) {
                return Err(Error::new("FIXTURE_PROBE_DISABLED_IN_RELEASE"));
            }
            match mode {
                FixtureMode::Sleep => {
                    std::thread::sleep(Duration::from_secs(30));
                    json!({"slept":true})
                }
                FixtureMode::Flood => {
                    let b = vec![b'x'; MAX_WIRE_BYTES + 100_000];
                    std::io::stdout()
                        .write_all(&b)
                        .map_err(|_| Error::new("PIPE_CLOSED"))?;
                    return Ok(());
                }
                FixtureMode::Exit => json!({"ok":true}),
                FixtureMode::Descendant => {
                    use std::process::{Command, Stdio};
                    let mut c = Command::new(own_exe()?)
                        .arg("__collector")
                        .stdin(Stdio::piped())
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .spawn()
                        .map_err(|_| Error::new("FIXTURE_SPAWN_FAILED"))?;
                    let request = serde_json::to_vec(&WorkerRequest::Fixture {
                        mode: FixtureMode::Sleep,
                    })
                    .map_err(|_| Error::new("SERIALIZE_FAILED"))?;
                    c.stdin
                        .take()
                        .ok_or_else(|| Error::new("PIPE_MISSING"))?
                        .write_all(&request)
                        .map_err(|_| Error::new("PIPE_CLOSED"))?;
                    // Deliberately leave a child for the supervisor's containment test, debug builds only.
                    json!({"descendant_pid":c.id()})
                }
            }
        }
    };
    serde_json::to_writer(std::io::stdout().lock(), &output)
        .map_err(|_| Error::new("OUTPUT_FAILED"))
}

fn operation(a: &Action) -> &'static str {
    match a {
        Action::Discovery { .. } => "discovery",
        Action::Atlas { .. } => "atlas",
        Action::Health { .. } => "health",
        Action::Workspace { .. } => "workspace",
        Action::Secret { .. } => "secret",
        Action::Journal { .. } => "journal",
        Action::Telemetry { .. } => "telemetry",
        Action::Integration { .. } => "integration",
        Action::Task { .. } => "task",
        Action::Effect { .. } => "effect",
        Action::Lifecycle { .. } => "lifecycle",
        Action::Chronicle { .. } => "chronicle",
        Action::Manifest { .. } => "manifest",
        Action::Ownership { .. } => "ownership",
        Action::UsageImport { .. } => "usage_import",
        Action::Init => "init",
        Action::Discover => "discover",
        Action::Root { .. } => "root",
        Action::Project { .. } => "project",
        Action::Doctor { .. } => "doctor",
        Action::Report { .. } => "report",
        Action::Upgrade => "upgrade",
        Action::Restore { .. } => "restore",
        Action::Record { .. } => "record",
        Action::Context { .. } => "context",
        Action::Timeline { .. } => "timeline",
        Action::Roster { .. } => "roster",
        Action::Decisions { .. } => "decisions",
        Action::Resources { .. } => "resources",
        Action::Economics { .. } => "economics",
        Action::NormalizeUsage { .. } => "normalize_usage",
        Action::Runway { .. } => "runway",
        Action::Simulate { .. } => "simulate",
        Action::ModelFrontier { .. } => "model_frontier",
        Action::SelectModels { .. } => "select_models",
        Action::DiagnoseEvidence { .. } => "diagnose_evidence",
        Action::PlanFit { .. } => "plan_fit",
        Action::Capabilities { .. } => "capabilities",
        Action::WorkspaceRegister { .. } => "workspace_register",
        Action::Checkpoint { .. } => "checkpoint",
        Action::Handoff { .. } => "handoff",
        Action::Mcp { .. } => "mcp",
        Action::VaultPut { .. } => "vault_put",
        Action::Backup => "backup",
        Action::ReplayFixture { .. } => "replay_fixture",
        Action::BuildInfo => "build_info",
    }
}
fn main() -> std::process::ExitCode {
    if std::env::args_os()
        .nth(1)
        .is_some_and(|a| a == "__collector")
    {
        return match worker() {
            Ok(()) => std::process::ExitCode::SUCCESS,
            Err(_) => std::process::ExitCode::from(2),
        };
    }
    let cli = match Cli::try_parse() {
        Ok(c) => c,
        Err(e) => {
            let code = if e.use_stderr() {
                eprintln!("INVALID_ARGUMENTS: use --help; argument values are not echoed.");
                4
            } else {
                let _ = e.print();
                0
            };
            return std::process::ExitCode::from(code);
        }
    };
    if let Action::Mcp {
        project,
        environment,
    } = &cli.command
    {
        let result = paths::select_home(cli.home.clone(), None)
            .and_then(|h| mcp::serve(&h, project, environment.as_deref()));
        return match result {
            Ok(()) => std::process::ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("MCP stopped: {}", e.code);
                std::process::ExitCode::from(2)
            }
        };
    }
    let op = operation(&cli.command);
    let run_id = platform::new_id();
    let observed_at = platform::now();
    let (envelope, text, code) = match execute(&cli) {
        Ok(r) => (
            Envelope {
                schema_version: SCHEMA_VERSION.into(),
                operation: op.into(),
                run_id,
                observed_at,
                status: if r.code == 2 {
                    "check_failed"
                } else if !r.coverage.is_complete() {
                    "partial"
                } else {
                    "completed"
                }
                .into(),
                coverage: r.coverage,
                data: Some(r.data),
                warnings: r.warnings,
                errors: vec![],
            },
            r.text,
            r.code,
        ),
        Err(e) => {
            let code = if [
                "HOME_NOT_CONFIGURED",
                "EXPLICIT_HOME_REQUIRED",
                "INVALID_ID",
                "CONFIG_INVALID",
            ]
            .contains(&e.code)
            {
                4
            } else {
                2
            };
            (
                Envelope::<Value> {
                    schema_version: SCHEMA_VERSION.into(),
                    operation: op.into(),
                    run_id,
                    observed_at,
                    status: "blocked".into(),
                    coverage: Coverage::Denied,
                    data: None,
                    warnings: vec![],
                    errors: vec![e.code.into()],
                },
                format!(
                    "Blocked: {}\nNo fallback home or broad recovery was attempted.\n",
                    e.code
                ),
                code,
            )
        }
    };
    let output = if cli.json {
        serde_json::to_string_pretty(&envelope)
            .unwrap_or_else(|_| "{\"status\":\"serialization_failed\"}".into())
    } else {
        text
    };
    if std::io::stdout()
        .lock()
        .write_all(format!("{output}\n").as_bytes())
        .is_err()
    {
        return std::process::ExitCode::from(2);
    }
    std::process::ExitCode::from(code)
}
