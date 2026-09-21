//! Shared Slice 2 domain. Imported data carries evidence, never execution authority.
use crate::Coverage;
use serde::{Deserialize, Serialize};

pub type Check<T> = std::result::Result<T, &'static str>;
pub const INPUT_LIMIT: usize = 256 * 1024;
pub const HANDOFF_LIMIT: usize = 64 * 1024;
pub const CONTEXT_LIMIT: usize = 64 * 1024;
pub const CONTROL_SCHEMA: &str = "workstation.control.v2";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    Observed,
    AgentReported,
    Inferred,
    UserApproved,
    ProviderVerified,
    Unknown,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    pub kind: EvidenceKind,
    pub source: String,
    pub at: i64,
    pub coverage: Coverage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkState {
    Planned,
    Active,
    Paused,
    Blocked,
    Review,
    Done,
    Cancelled,
}
impl WorkState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Planned => "planned",
            Self::Active => "active",
            Self::Paused => "paused",
            Self::Blocked => "blocked",
            Self::Review => "review",
            Self::Done => "done",
            Self::Cancelled => "cancelled",
        }
    }
    pub fn can_transition(self, next: Self) -> bool {
        if self == next {
            return true;
        }
        match self {
            Self::Planned => matches!(next, Self::Active | Self::Blocked | Self::Cancelled),
            Self::Active => matches!(
                next,
                Self::Paused | Self::Blocked | Self::Review | Self::Cancelled
            ),
            Self::Paused | Self::Blocked => matches!(next, Self::Active | Self::Cancelled),
            Self::Review => matches!(
                next,
                Self::Active | Self::Done | Self::Blocked | Self::Cancelled
            ),
            Self::Done | Self::Cancelled => false,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkItem {
    pub id: String,
    pub project_id: String,
    pub objective: String,
    pub state: WorkState,
    pub priority: u8,
    pub version: u64,
    pub workspace_id: Option<String>,
    pub completed: Vec<String>,
    pub remaining: Vec<String>,
    pub blockers: Vec<String>,
    pub evidence: Evidence,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Session {
    pub id: String,
    pub project_id: String,
    pub agent: String,
    pub external_id: String,
    pub adapter_version: String,
    pub role: String,
    pub workspace_id: Option<String>,
    pub last_observed: i64,
    pub ended_at: Option<i64>,
    pub coverage: Coverage,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assignment {
    pub work_id: String,
    pub session_id: String,
    pub role: String,
    pub lease_until: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceStamp {
    pub project_id: String,
    pub workspace_id: String,
    pub path: String,
    pub directory_identity: String,
    pub head: String,
    pub branch: Option<String>,
    pub status_digest: String,
    pub dirty: Option<bool>,
    pub untracked: Option<u64>,
    pub ignored: Option<u64>,
    pub local_only_commits: Option<u64>,
    pub blockers: Vec<String>,
    pub observed_at: i64,
    pub coverage: Coverage,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Checkpoint {
    pub id: String,
    pub work: WorkItem,
    pub session_id: Option<String>,
    pub workspace: WorkspaceStamp,
    pub completed: Vec<String>,
    pub remaining: Vec<String>,
    pub blockers: Vec<String>,
    pub tests: Vec<TestObservation>,
    pub decision_ids: Vec<String>,
    pub resource_ids: Vec<String>,
    pub recorded_at: i64,
    pub source: Evidence,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TestObservation {
    pub name: String,
    pub result: String,
    pub evidence: Evidence,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Decision {
    pub id: String,
    pub project_id: String,
    pub topic: String,
    pub scope: String,
    pub statement: String,
    pub rationale: String,
    pub alternatives: Vec<String>,
    pub consequences: Vec<String>,
    pub predecessor: Option<String>,
    pub effective_at: i64,
    pub recorded_at: i64,
    pub source: Evidence,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionAcceptance {
    pub decision_id: String,
    pub accepted_at: i64,
    pub approval_ref: String,
}
/// Bitemporal selection: a future successor does not erase the currently effective parent.
/// Concurrent chains return a conflict instead of choosing the newest timestamp.
pub fn applicable_decisions(
    all: &[Decision],
    acceptances: &[DecisionAcceptance],
    valid_at: i64,
    known_at: i64,
) -> Check<Vec<Decision>> {
    let eligible: Vec<&Decision> = all
        .iter()
        .filter(|d| {
            d.recorded_at <= known_at
                && d.effective_at <= valid_at
                && acceptances
                    .iter()
                    .any(|a| a.decision_id == d.id && a.accepted_at <= known_at)
        })
        .collect();
    let current: Vec<Decision> = eligible
        .iter()
        .filter(|d| {
            !eligible
                .iter()
                .any(|n| n.predecessor.as_deref() == Some(d.id.as_str()))
        })
        .map(|d| (**d).clone())
        .collect();
    let mut scopes = std::collections::BTreeSet::new();
    for d in &current {
        if !scopes.insert((&d.project_id, &d.topic, &d.scope)) {
            return Err("DECISION_CONFLICT");
        }
    }
    Ok(current)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceKind {
    Service,
    Endpoint,
    Deployment,
    Database,
    Runbook,
    SecretReference,
    Reference,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Resource {
    pub id: String,
    pub project_id: String,
    pub environment: String,
    pub kind: ResourceKind,
    pub name: String,
    pub locator: String,
    pub provider: Option<String>,
    pub evidence: Evidence,
    pub fresh_for_secs: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Verification {
    pub resource_id: String,
    pub claim: String,
    pub observed_at: i64,
    pub source: Evidence,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Focus {
    pub needs: Vec<String>,
    pub free_only: bool,
    pub local_only: bool,
    pub platform: String,
    pub existing: Vec<String>,
    pub blocked: Vec<String>,
    pub evidence_ref: String,
}

pub fn id(value: &str) -> Check<()> {
    if value.is_empty()
        || value.len() > 64
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
    {
        Err("INVALID_ID")
    } else {
        Ok(())
    }
}
pub fn text(value: &str, max: usize) -> Check<()> {
    if value.len() > max
        || value.chars().any(|c| {
            c == '\0'
                || (c.is_control() && c != '\n' && c != '\t')
                || matches!(c,'\u{202a}'..='\u{202e}'|'\u{2066}'..='\u{2069}')
        })
    {
        return Err("TEXT_LIMIT_OR_CONTROL");
    }
    // A narrow guard against obvious accidental credentials, NOT a DLP guarantee.
    let lower = value.to_ascii_lowercase();
    if [
        "-----begin private key",
        "-----begin rsa private key",
        "secret_canary",
        "authorization: bearer ",
        "sk-proj-",
        "ghp_",
        "gho_",
    ]
    .iter()
    .any(|p| lower.contains(p))
    {
        return Err("SENSITIVE_TEXT_REJECTED");
    }
    Ok(())
}
pub fn lines(values: &[String]) -> Check<()> {
    if values.len() > 32 {
        return Err("LIST_LIMIT");
    }
    for s in values {
        text(s, 1024)?;
    }
    Ok(())
}
pub fn timestamp(t: i64) -> Check<()> {
    if !(0..=253_402_300_799).contains(&t) {
        Err("INVALID_TIME")
    } else {
        Ok(())
    }
}
pub fn evidence(e: &Evidence) -> Check<()> {
    text(&e.source, 256)?;
    timestamp(e.at)
}
pub fn validate_work(w: &WorkItem) -> Check<()> {
    id(&w.id)?;
    id(&w.project_id)?;
    text(&w.objective, 2048)?;
    if w.objective.trim().is_empty() || w.priority > 4 {
        return Err("INVALID_WORK");
    }
    if let Some(v) = &w.workspace_id {
        id(v)?;
    }
    lines(&w.completed)?;
    lines(&w.remaining)?;
    lines(&w.blockers)?;
    evidence(&w.evidence)
}
pub fn validate_session(s: &Session) -> Check<()> {
    id(&s.id)?;
    id(&s.project_id)?;
    id(&s.agent)?;
    text(&s.external_id, 256)?;
    text(&s.adapter_version, 64)?;
    if !["worker", "reviewer", "observer"].contains(&s.role.as_str()) {
        return Err("INVALID_ROLE");
    }
    timestamp(s.last_observed)?;
    if let Some(w) = &s.workspace_id {
        id(w)?;
    }
    if let Some(t) = s.ended_at {
        timestamp(t)?;
    }
    if s.external_id.is_empty() {
        return Err("EMPTY_VENDOR_SESSION_ID");
    }
    if s.ended_at.is_some_and(|t| t < s.last_observed) {
        return Err("SESSION_TIME_ORDER");
    }
    Ok(())
}
pub fn validate_decision(d: &Decision) -> Check<()> {
    id(&d.id)?;
    id(&d.project_id)?;
    id(&d.topic)?;
    id(&d.scope)?;
    text(&d.statement, 4096)?;
    text(&d.rationale, 4096)?;
    if d.statement.trim().is_empty() {
        return Err("EMPTY_DECISION");
    }
    if d.predecessor.as_deref() == Some(d.id.as_str()) {
        return Err("DECISION_SELF_CYCLE");
    }
    lines(&d.alternatives)?;
    lines(&d.consequences)?;
    timestamp(d.effective_at)?;
    timestamp(d.recorded_at)?;
    evidence(&d.source)
}
pub fn safe_locator(value: &str, secret: bool) -> Check<()> {
    text(value, 2048)?;
    if value.trim() != value || value.contains(['\n', '\r', '\t', '\\', '%', '?', '#', '@']) {
        return Err("LOCATOR_UNSAFE");
    }
    if secret {
        if ![
            "local://",
            "doppler://",
            "op://",
            "infisical://",
            "bitwarden://",
        ]
        .iter()
        .any(|p| value.starts_with(p))
        {
            return Err("SECRET_REFERENCE_SCHEME");
        }
        let tail = value.split_once("://").map(|x| x.1).unwrap_or("");
        if tail.is_empty()
            || tail
                .split('/')
                .any(|c| c.is_empty() || c == "." || c == "..")
        {
            return Err("SECRET_REFERENCE_INVALID");
        }
    } else if !(value.starts_with("https://")
        || value.starts_with("http://localhost:")
        || value.starts_with("reference://"))
    {
        return Err("LOCATOR_SCHEME_UNSUPPORTED");
    }
    if !secret {
        let rest = value.split_once("://").map(|x| x.1).unwrap_or("");
        let authority = rest.split('/').next().unwrap_or("");
        if authority.is_empty() || authority.chars().any(char::is_whitespace) {
            return Err("LOCATOR_AUTHORITY_INVALID");
        }
    }
    Ok(())
}
pub fn validate_resource(r: &Resource) -> Check<()> {
    id(&r.id)?;
    id(&r.project_id)?;
    id(&r.environment)?;
    text(&r.name, 128)?;
    if let Some(provider) = &r.provider {
        text(provider, 128)?;
    }
    safe_locator(&r.locator, r.kind == ResourceKind::SecretReference)?;
    if r.fresh_for_secs == 0 || r.fresh_for_secs > 31_536_000 {
        return Err("RESOURCE_FRESHNESS_INVALID");
    }
    evidence(&r.evidence)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn no_abandoned_state() {
        assert!(serde_json::from_str::<WorkState>("\"abandoned\"").is_err());
    }
    #[test]
    fn done_is_terminal() {
        assert!(!WorkState::Done.can_transition(WorkState::Active));
    }
    #[test]
    fn active_requires_review_before_done() {
        assert!(!WorkState::Active.can_transition(WorkState::Done));
        assert!(WorkState::Active.can_transition(WorkState::Review));
    }
    #[test]
    fn unicode_text_works() {
        assert!(text("مشروع 日本語", 100).is_ok());
    }
    #[test]
    fn malicious_id_rejected() {
        assert!(id("x'; DROP TABLE projects;").is_err());
    }
    #[test]
    fn locator_credentials_rejected() {
        assert!(safe_locator("https://user:pass@host/db", false).is_err());
    }
    #[test]
    fn no_secret_value_uri() {
        assert!(safe_locator("secret-value", true).is_err());
    }
    #[test]
    fn dpapi_reference_is_metadata() {
        assert!(safe_locator("local://project/prod/database", true).is_ok());
    }
    #[test]
    fn token_query_rejected() {
        assert!(safe_locator("https://host/a?token=x", false).is_err());
    }
    #[test]
    fn secret_canary_rejected() {
        assert!(text("SECRET_CANARY_do_not_leak", 100).is_err());
    }
    #[test]
    fn bidi_rejected() {
        assert!(text("report\u{202e}secret", 100).is_err());
    }
}
