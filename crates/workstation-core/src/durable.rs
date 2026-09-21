//! Durable *evidence*, not exactly-once remote execution. No OS or provider effects here.
use crate::control::{self, Check};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const MAX_EVENTS: u64 = 2048;
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeadlineKind {
    Handshake,
    Session,
    Idle,
    Absolute,
}

pub const MAX_RUNS: u64 = 512;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DeadlinePolicy {
    pub handshake_seconds: u32,
    pub session_seconds: u32,
    pub idle_seconds: u32,
    pub absolute_seconds: u32,
    pub cancellation_grace_seconds: u32,
    pub reconciliation_seconds: u32,
}
impl Default for DeadlinePolicy {
    fn default() -> Self {
        Self {
            handshake_seconds: 30,
            session_seconds: 60,
            idle_seconds: 180,
            absolute_seconds: 1800,
            cancellation_grace_seconds: 5,
            reconciliation_seconds: 30,
        }
    }
}
impl DeadlinePolicy {
    pub fn validate(&self) -> Check<()> {
        if !(1..=120).contains(&self.handshake_seconds)
            || !(1..=300).contains(&self.session_seconds)
            || !(1..=1800).contains(&self.idle_seconds)
            || !(1..=3600).contains(&self.absolute_seconds)
            || !(1..=30).contains(&self.cancellation_grace_seconds)
            || !(1..=120).contains(&self.reconciliation_seconds)
            || self.handshake_seconds > self.absolute_seconds
            || self.session_seconds > self.absolute_seconds
            || self.idle_seconds > self.absolute_seconds
        {
            return Err("DURABLE_DEADLINE_INVALID");
        }
        Ok(())
    }
    pub fn from_legacy(total: u32) -> Self {
        let t = total.clamp(1, 3600);
        Self {
            handshake_seconds: t.min(30),
            session_seconds: t.min(60),
            idle_seconds: t.min(180),
            absolute_seconds: t,
            ..Self::default()
        }
    }
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TransportState {
    NotStarted,
    Starting,
    Connected,
    Exited,
    TimedOut,
    Lost,
    Cancelled,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderState {
    Unknown,
    Starting,
    Running,
    Completed,
    Failed,
    Interrupted,
    AuthenticationRequired,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceState {
    NotObserved,
    Unchanged,
    Changed,
    Unexpected,
    Unavailable,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VerificationState {
    NotRequested,
    Pending,
    Passed,
    Failed,
    Stale,
    Unavailable,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunState {
    Planned,
    Starting,
    Running,
    CancelRequested,
    ReconciliationRequired,
    AwaitingVerification,
    ProgressConfirmed,
    Verified,
    VerificationFailed,
    Failed,
    Cancelled,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProcessIdentity {
    pub pid: u32,
    pub birth: String,
    pub host: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunRecord {
    pub id: String,
    pub plan_id: String,
    pub execution_id: Option<String>,
    pub project_id: String,
    pub environment: String,
    pub integration_id: String,
    pub integration_digest: String,
    pub executable_sha256: String,
    pub declared_version: String,
    pub adapter: String,
    pub work_id: String,
    pub workspace_id: String,
    pub checkpoint_id: String,
    pub work_version: u64,
    pub workspace_digest: String,
    pub deadlines: DeadlinePolicy,
    pub supervisor: Option<ProcessIdentity>,
    pub child: Option<ProcessIdentity>,
    pub session_id: Option<String>,
    pub turn_id: Option<String>,
    pub transport: TransportState,
    pub provider: ProviderState,
    pub workspace: WorkspaceState,
    pub verification: VerificationState,
    pub created_at: i64,
    pub updated_at: i64,
    pub last_heartbeat_at: Option<i64>,
    pub last_progress_at: Option<i64>,
    pub finished_at: Option<i64>,
    pub cancellation_requested: bool,
    pub owner_released: bool,
    pub revision: u64,
    pub sequence: u64,
    pub error_code: Option<String>,
    pub deadline_expired: Option<DeadlineKind>,
}
impl RunRecord {
    pub fn state(&self) -> RunState {
        if self.cancellation_requested && self.execution_id.is_none() {
            return RunState::Cancelled;
        }
        if self.verification == VerificationState::Passed {
            return if self.provider == ProviderState::Completed {
                RunState::Verified
            } else {
                RunState::ProgressConfirmed
            };
        }
        if self.verification == VerificationState::Failed {
            return RunState::VerificationFailed;
        }
        if self.cancellation_requested && !self.owner_released {
            return RunState::CancelRequested;
        }
        if self.provider == ProviderState::Completed {
            return RunState::AwaitingVerification;
        }
        if self.provider == ProviderState::Failed
            || self.provider == ProviderState::AuthenticationRequired
        {
            return RunState::Failed;
        }
        if self.provider == ProviderState::Interrupted
            && self.transport == TransportState::Cancelled
        {
            return RunState::Cancelled;
        }
        if self.workspace == WorkspaceState::Changed && self.owner_released {
            return RunState::ProgressConfirmed;
        }
        if self.owner_released
            || matches!(
                self.transport,
                TransportState::TimedOut | TransportState::Lost | TransportState::Cancelled
            )
        {
            return RunState::ReconciliationRequired;
        }
        match self.transport {
            TransportState::NotStarted => RunState::Planned,
            TransportState::Starting => RunState::Starting,
            _ => RunState::Running,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum RunEvent {
    DeadlineExceeded {
        deadline: DeadlineKind,
    },
    Claimed {
        supervisor: ProcessIdentity,
        execution_id: String,
    },
    ChildStarted {
        identity: ProcessIdentity,
    },
    SessionBound {
        session_id: String,
    },
    TurnBound {
        turn_id: String,
    },
    Progress,
    ProviderTerminal {
        status: ProviderState,
    },
    TransportEnded {
        state: TransportState,
        error_code: Option<String>,
    },
    CancelRequested,
    Recovered {
        supervisor_gone: bool,
    },
    WorkspaceObserved {
        state: WorkspaceState,
    },
    Verification {
        state: VerificationState,
    },
    Reconciled {
        status: ProviderState,
    },
}
fn bounded_code(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 96
        && s.bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
}
pub fn opaque(s: &str) -> Check<()> {
    control::text(s, 256)?;
    if s.trim().is_empty() || s.starts_with('-') {
        return Err("DURABLE_ID_INVALID");
    }
    Ok(())
}
pub fn apply_event(r: &mut RunRecord, e: &RunEvent, at: i64) -> Check<()> {
    control::timestamp(at)?;
    // A changed wall clock does not authorize a transition. Operator can inspect history.
    if at < r.updated_at {
        return Err("DURABLE_CLOCK_REGRESSION");
    }
    match e {
        RunEvent::DeadlineExceeded { deadline } => {
            r.deadline_expired = Some(*deadline);
        }
        RunEvent::Claimed {
            supervisor,
            execution_id,
        } => {
            if r.execution_id.is_some() {
                return Err("DURABLE_ALREADY_CLAIMED");
            }
            r.supervisor = Some(supervisor.clone());
            r.execution_id = Some(execution_id.clone());
            r.transport = TransportState::Starting;
            r.provider = ProviderState::Starting;
        }
        RunEvent::ChildStarted { identity } => {
            r.child = Some(identity.clone());
            r.transport = TransportState::Connected;
        }
        RunEvent::SessionBound { session_id } => {
            opaque(session_id)?;
            if r.session_id.as_ref().is_some_and(|s| s != session_id) {
                return Err("DURABLE_SESSION_CHANGED");
            }
            r.session_id = Some(session_id.clone());
        }
        RunEvent::TurnBound { turn_id } => {
            opaque(turn_id)?;
            if r.session_id.is_none() {
                return Err("DURABLE_SESSION_REQUIRED");
            }
            if r.turn_id.as_ref().is_some_and(|s| s != turn_id) {
                return Err("DURABLE_TURN_CHANGED");
            }
            r.turn_id = Some(turn_id.clone());
            if !provider_terminal(r.provider) {
                r.provider = ProviderState::Running;
            }
        }
        RunEvent::Progress => {
            if !provider_terminal(r.provider) {
                r.provider = ProviderState::Running;
                r.last_progress_at = Some(at);
            }
        }
        RunEvent::ProviderTerminal { status } | RunEvent::Reconciled { status } => {
            if !provider_terminal(*status) {
                return Err("DURABLE_TERMINAL_REQUIRED");
            }
            if provider_terminal(r.provider) && r.provider != *status {
                return Err("DURABLE_TERMINAL_CONFLICT");
            }
            r.provider = *status;
        }
        RunEvent::TransportEnded { state, error_code } => {
            if !matches!(
                state,
                TransportState::Exited
                    | TransportState::Lost
                    | TransportState::TimedOut
                    | TransportState::Cancelled
            ) {
                return Err("DURABLE_TRANSPORT_END_REQUIRED");
            }
            if error_code.as_ref().is_some_and(|s| !bounded_code(s)) {
                return Err("DURABLE_ERROR_CODE_INVALID");
            }
            r.transport = *state;
            r.error_code = error_code.clone();
            r.owner_released = true;
            r.finished_at = Some(at);
        }
        RunEvent::CancelRequested => {
            if !r.owner_released {
                r.cancellation_requested = true;
            }
        }
        RunEvent::Recovered { supervisor_gone } => {
            if *supervisor_gone {
                r.owner_released = true;
                r.transport = TransportState::Lost;
                r.finished_at = Some(at);
            }
        }
        RunEvent::WorkspaceObserved { state } => {
            r.workspace = *state;
        }
        RunEvent::Verification { state } => {
            r.verification = *state;
        }
    }
    r.updated_at = at;
    r.revision = r
        .revision
        .checked_add(1)
        .ok_or("DURABLE_REVISION_OVERFLOW")?;
    r.sequence = r
        .sequence
        .checked_add(1)
        .ok_or("DURABLE_SEQUENCE_OVERFLOW")?;
    Ok(())
}
pub fn provider_terminal(p: ProviderState) -> bool {
    matches!(
        p,
        ProviderState::Completed
            | ProviderState::Failed
            | ProviderState::Interrupted
            | ProviderState::AuthenticationRequired
    )
}
/// Only exact matching terminal turn facts are admissible. Idle thread or newest turn is insufficient.
pub fn codex_turn_status(
    value: &serde_json::Value,
    session: &str,
    turn: &str,
) -> Check<Option<ProviderState>> {
    let t = value.get("thread").ok_or("RECONCILE_THREAD_MISSING")?;
    if t.get("id").and_then(serde_json::Value::as_str) != Some(session) {
        return Err("RECONCILE_THREAD_MISMATCH");
    }
    let Some(turns) = t.get("turns").and_then(serde_json::Value::as_array) else {
        return Ok(None);
    };
    if turns.len() > 4096 {
        return Err("RECONCILE_TURN_LIMIT");
    }
    let mut found = None;
    for item in turns {
        if item.get("id").and_then(serde_json::Value::as_str) != Some(turn) {
            continue;
        }
        if found.is_some() {
            return Err("RECONCILE_DUPLICATE_TURN");
        }
        found = Some(
            match item.get("status").and_then(serde_json::Value::as_str) {
                Some("completed") => Some(ProviderState::Completed),
                Some("failed") => Some(ProviderState::Failed),
                Some("interrupted") => Some(ProviderState::Interrupted),
                _ => None,
            },
        );
    }
    Ok(found.flatten())
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ContentManifest {
    pub directory_identity: String,
    pub files: BTreeMap<String, String>,
    pub bytes: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationContract {
    pub allowed_paths: Vec<String>,
    pub forbidden_paths: Vec<String>,
    pub task_ids: Vec<String>,
}
impl VerificationContract {
    pub fn validate(&self) -> Check<()> {
        if self.allowed_paths.len() > 128
            || self.forbidden_paths.len() > 128
            || self.task_ids.len() > 8
        {
            return Err("VERIFICATION_BUDGET");
        }
        if self.task_ids.is_empty() {
            return Err("VERIFICATION_TASK_REQUIRED");
        }
        for p in self.allowed_paths.iter().chain(&self.forbidden_paths) {
            relative(p)?;
        }
        for t in &self.task_ids {
            control::id(t)?;
        }
        Ok(())
    }
}
pub fn relative(p: &str) -> Check<()> {
    control::text(p, 1024)?;
    if p.is_empty()
        || p.starts_with('/')
        || p.contains(['\\', ':', '*', '?'])
        || p.split('/')
            .any(|s| s.is_empty() || s == "." || s == ".." || s.eq_ignore_ascii_case(".git"))
    {
        return Err("VERIFICATION_PATH_INVALID");
    }
    Ok(())
}
pub fn changed_paths(a: &ContentManifest, b: &ContentManifest) -> Check<Vec<String>> {
    if a.directory_identity != b.directory_identity {
        return Err("VERIFICATION_DIRECTORY_REPLACED");
    }
    let mut keys: std::collections::BTreeSet<_> = a.files.keys().cloned().collect();
    keys.extend(b.files.keys().cloned());
    Ok(keys
        .into_iter()
        .filter(|k| a.files.get(k) != b.files.get(k))
        .collect())
}
pub fn permitted_changes(paths: &[String], v: &VerificationContract) -> bool {
    paths.iter().all(|p| {
        v.allowed_paths.iter().any(|a| a.eq_ignore_ascii_case(p))
            && !v.forbidden_paths.iter().any(|f| f.eq_ignore_ascii_case(p))
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn deadlines_keep_absolute_ceiling() {
        let p = DeadlinePolicy {
            absolute_seconds: 1,
            ..DeadlinePolicy::default()
        };
        assert!(p.validate().is_err());
    }
    #[test]
    fn legacy_never_expands_authority() {
        let p = DeadlinePolicy::from_legacy(7);
        assert_eq!(p.absolute_seconds, 7);
        assert!(p.validate().is_ok());
    }
    #[test]
    fn thread_idle_is_not_turn_completion() {
        assert_eq!(
            codex_turn_status(
                &json!({"thread":{"id":"s","status":"idle","turns":[]}}),
                "s",
                "t"
            )
            .unwrap(),
            None
        );
    }
    #[test]
    fn reconciliation_uses_exact_turn() {
        assert_eq!(
            codex_turn_status(
                &json!({"thread":{"id":"s","turns":[{"id":"other","status":"completed"}]}}),
                "s",
                "t"
            )
            .unwrap(),
            None
        );
    }
    #[test]
    fn terminal_matched() {
        assert_eq!(
            codex_turn_status(
                &json!({"thread":{"id":"s","turns":[{"id":"t","status":"completed"}]}}),
                "s",
                "t"
            )
            .unwrap(),
            Some(ProviderState::Completed)
        );
    }
    #[test]
    fn other_thread_rejected() {
        assert!(codex_turn_status(&json!({"thread":{"id":"x"}}), "s", "t").is_err());
    }
    #[test]
    fn duplicate_turn_rejected() {
        assert!(codex_turn_status(&json!({"thread":{"id":"s","turns":[{"id":"t","status":"completed"},{"id":"t","status":"failed"}]}}),"s","t").is_err());
    }
    #[test]
    fn metadata_contracts_forbid_scripts() {
        assert!(serde_json::from_value::<VerificationContract>(
            json!({"allowed_paths":[],"forbidden_paths":[],"task_ids":["a"],"shell":"echo hi"})
        )
        .is_err());
    }
    #[test]
    fn path_escape_rejected() {
        for s in ["../x", "/x", "x/../y", "C:/x", "x\\y", ".git/config"] {
            assert!(relative(s).is_err());
        }
    }
    #[test]
    fn content_hash_not_mtime() {
        let mut a = ContentManifest {
            directory_identity: "i".into(),
            files: BTreeMap::new(),
            bytes: 1,
        };
        a.files.insert("a".into(), "hash1".into());
        let mut b = a.clone();
        b.files.insert("a".into(), "hash2".into());
        assert_eq!(changed_paths(&a, &b).unwrap(), vec!["a"]);
    }
    #[test]
    fn forbidden_wins_over_allowed() {
        let v = VerificationContract {
            allowed_paths: vec!["test.py".into()],
            forbidden_paths: vec!["test.py".into()],
            task_ids: vec!["test".into()],
        };
        assert!(!permitted_changes(&["test.py".into()], &v));
    }
}
