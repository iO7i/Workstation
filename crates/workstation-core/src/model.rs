use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const SCHEMA_VERSION: &str = "workstation.r0.v1";
pub const MAX_ROOTS: usize = 24;
pub const MAX_PROJECTS: usize = 20;
pub const MAX_WIRE_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Coverage {
    Complete,
    Partial,
    Denied,
    Unsupported,
    TimedOut,
}
impl Coverage {
    pub fn is_complete(self) -> bool {
        self == Self::Complete
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Root {
    pub id: String,
    pub path: PathBuf,
    pub kind: RootKind,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RootKind {
    Codex,
    Claude,
    Cursor,
    Vscode,
    Docker,
    Workspace,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    pub id: String,
    pub path: PathBuf,
    pub git_executable: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub schema_version: String,
    pub home: PathBuf,
    pub installation_id: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScanBudget {
    pub deadline_ms: u64,
    pub max_entries: u64,
    pub max_depth: u32,
}
impl ScanBudget {
    pub fn normal() -> Self {
        Self {
            deadline_ms: 700,
            max_entries: 20_000,
            max_depth: 32,
        }
    }
    pub fn deep() -> Self {
        Self {
            deadline_ms: 4_000,
            max_entries: 100_000,
            max_depth: 64,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorageObservation {
    pub root: Root,
    pub observed_at: String,
    pub coverage: Coverage,
    /// Null means unavailable. With partial coverage this is a lower-bound entry sum.
    pub logical_entry_bytes: Option<u64>,
    /// R0 does not claim physical allocation or hard-link-deduplicated totals.
    pub allocated_bytes: Option<u64>,
    pub files: u64,
    pub directories: u64,
    pub visited_entries: u64,
    pub skipped_links_or_placeholders: u64,
    pub io_errors: u64,
    pub notes: Vec<String>,
    pub delta_since_previous_complete_bytes: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiskObservation {
    pub target: String,
    pub coverage: Coverage,
    pub available_bytes: Option<u64>,
    pub total_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProcessObservation {
    pub pid: u32,
    pub parent_pid_observed: u32,
    /// Windows FILETIME (100ns ticks since 1601); null if not accessible.
    pub birth_filetime_100ns: Option<u64>,
    pub executable_name: String,
    pub working_set_bytes: Option<u64>,
    pub private_commit_bytes: Option<u64>,
    pub coverage: Coverage,
    /// R0 never establishes session ownership or recommends termination.
    pub ownership: String,
    pub disposition: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProcessSnapshot {
    pub observed_at: String,
    pub coverage: Coverage,
    pub enumerated_count: u64,
    pub selected: Vec<ProcessObservation>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Worktree {
    pub path: String,
    pub head: Option<String>,
    pub branch: Option<String>,
    pub detached: bool,
    pub bare: bool,
    pub locked: bool,
    pub prunable: bool,
    pub disposition: String,
    pub content_coverage: Coverage,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceObservation {
    pub project_id: String,
    pub observed_at: String,
    pub coverage: Coverage,
    pub worktrees: Vec<Worktree>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Finding {
    pub code: String,
    pub severity: Severity,
    pub target_id: Option<String>,
    pub explanation: String,
    pub next_action: String,
    pub repair_available: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub schema_version: String,
    pub run_id: String,
    pub observed_at: String,
    pub installation_id: String,
    pub platform: String,
    pub coverage: Coverage,
    pub storage: Vec<StorageObservation>,
    pub disks: Vec<DiskObservation>,
    pub processes: ProcessSnapshot,
    pub workspaces: Vec<WorkspaceObservation>,
    pub findings: Vec<Finding>,
    pub not_inspected: Vec<String>,
    pub saved: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Envelope<T> {
    pub schema_version: String,
    pub operation: String,
    pub run_id: String,
    pub observed_at: String,
    pub status: String,
    pub coverage: Coverage,
    pub data: Option<T>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkerRequest {
    HostGraph {
        host: String,
        boot: String,
        profiles: Vec<crate::integrations::Integration>,
    },
    WorkspaceDetail {
        project: Project,
        workspace_id: String,
        path: PathBuf,
    },
    Storage {
        root: Root,
        budget: ScanBudget,
    },
    Processes,
    Worktrees {
        project: Project,
    },
    WorkspaceStamp {
        project: Project,
        workspace_id: String,
        path: PathBuf,
    },
    AgentProfile {
        integration: crate::integrations::Integration,
    },
    /// Own-tool fixture probes only; never external commands.
    Fixture {
        mode: FixtureMode,
    },
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FixtureMode {
    Sleep,
    Flood,
    Exit,
    Descendant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IncidentInput {
    pub id: String,
    pub family: IncidentFamily,
    pub fresh_error: bool,
    pub active_owner: Option<bool>,
    pub correct_registry_checked: Option<bool>,
    pub contents_inspected: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IncidentFamily {
    DockerSocket,
    CodexAttachments,
    OldWorkspace,
}
