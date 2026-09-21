//! Transactional control-plane operations. Caller-approved metadata only; no vendor mutations.
use crate::{new_id, storage::Store, Error, Result};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::Path;
use workstation_core::{
    continuity::{self, RosterEntry},
    control::*,
    discovery::{self, CandidateInput, Capability, Feedback},
    economics::{self, QuotaSample},
    plans::*,
    Coverage,
};
const MIGRATION: &str = include_str!("../../../schema/002_control_plane.sql");
const CURRENT_DECISIONS: &str = include_str!("../../../schema/current_decisions.sql");
pub fn epoch() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp()
}
pub fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn err(_: rusqlite::Error) -> Error {
    Error::new("CONTROL_DATABASE_REJECTED")
}
fn check<T>(r: Check<T>) -> Result<T> {
    r.map_err(Error::new)
}
fn sql_u64(value: u64) -> Result<i64> {
    i64::try_from(value).map_err(|_| Error::new("SQL_INTEGER_OVERFLOW"))
}
fn encode<T: Serialize>(x: &T) -> Result<String> {
    serde_json::to_string(x).map_err(|_| Error::new("CONTROL_ENCODING"))
}
fn decode<T: serde::de::DeserializeOwned>(x: &str) -> Result<T> {
    serde_json::from_str(x).map_err(|_| Error::new("CONTROL_RECORD_INVALID"))
}
fn optional_context<T: Serialize>(
    name: &str,
    result: Result<T>,
    failures: &mut Vec<Value>,
) -> Value {
    match result.and_then(|value| {
        serde_json::to_value(value).map_err(|_| Error::new("CONTEXT_SECTION_ENCODING"))
    }) {
        Ok(value) => value,
        Err(error) => {
            failures.push(json!({"section":name,"coverage":"partial","error_code":error.code}));
            json!({"coverage":"partial","available":false,"error_code":error.code,"query_separately":true})
        }
    }
}
fn report_evidence(source: &str, at: i64) -> Evidence {
    Evidence {
        kind: EvidenceKind::AgentReported,
        source: source.into(),
        at,
        coverage: Coverage::Partial,
    }
}
fn require_project(c: &Connection, project: &str) -> Result<()> {
    check(id(project))?;
    if !c
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM projects WHERE id=?1)",
            [project],
            |r| r.get::<_, bool>(0),
        )
        .map_err(err)?
    {
        return Err(Error::new("PROJECT_NOT_REGISTERED"));
    }
    Ok(())
}
fn bump_work(tx: &Transaction<'_>, work: &mut WorkItem, expected: u64, at: i64) -> Result<()> {
    work.version = expected
        .checked_add(1)
        .ok_or(Error::new("VERSION_OVERFLOW"))?;
    let next_version = sql_u64(work.version)?;
    let expected_version = sql_u64(expected)?;
    let changed=tx.execute("UPDATE work_items SET state=?1,version=?2,workspace_id=?3,payload=?4,updated_at=?5 WHERE id=?6 AND version=?7",
 params![work.state.as_str(),next_version,work.workspace_id,encode(work)?,at,work.id,expected_version]).map_err(err)?;
    if changed != 1 {
        return Err(Error::new("WORK_VERSION_CONFLICT"));
    }
    Ok(())
}
fn load_work(c: &Connection, work: &str) -> Result<WorkItem> {
    let s: String = c
        .query_row("SELECT payload FROM work_items WHERE id=?1", [work], |r| {
            r.get(0)
        })
        .map_err(err)?;
    decode(&s)
}
fn audit(
    tx: &Transaction<'_>,
    project: Option<&str>,
    kind: &str,
    at: i64,
    source: &str,
    data: &Value,
) -> Result<()> {
    tx.execute("INSERT INTO audit_events(event_id,project_id,kind,recorded_at,source_kind,payload) VALUES(?1,?2,?3,?4,?5,?6)",params![new_id(),project,kind,at,source,encode(data)?]).map_err(err)?;
    Ok(())
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum Record {
    EnvironmentAdd {
        project_id: String,
        name: String,
    },
    EventRecord {
        project_id: String,
        event_kind: String,
        title: String,
        details: String,
        evidence_refs: Vec<String>,
    },
    FocusSet {
        project_id: String,
        focus: Focus,
    },
    EconomicsPreference {
        project_id: String,
        profile: economics::PreferenceProfile,
    },
    WorkCreate {
        id: String,
        project_id: String,
        objective: String,
        priority: u8,
        workspace_id: Option<String>,
    },
    WorkUpdate {
        id: String,
        expected_version: u64,
        state: WorkState,
        completed: Vec<String>,
        remaining: Vec<String>,
        blockers: Vec<String>,
    },
    SessionReport {
        id: String,
        project_id: String,
        agent: String,
        external_id: String,
        adapter_version: String,
        role: String,
        workspace_id: Option<String>,
        ended: bool,
    },
    Assign {
        work_id: String,
        session_id: String,
        role: String,
        expected_version: u64,
        lease_seconds: u32,
    },
    ReleaseAssignment {
        assignment_id: String,
        work_id: String,
        expected_version: u64,
    },
    DecisionPropose {
        id: String,
        project_id: String,
        topic: String,
        scope: String,
        statement: String,
        rationale: String,
        alternatives: Vec<String>,
        consequences: Vec<String>,
        predecessor: Option<String>,
        effective_at: i64,
    },
    DecisionAccept {
        decision_id: String,
    },
    DecisionDispose {
        decision_id: String,
        disposition: String,
    },
    ResourceAdd {
        id: String,
        project_id: String,
        environment: String,
        kind: ResourceKind,
        name: String,
        locator: String,
        provider: Option<String>,
        fresh_for_secs: u32,
    },
    ResourceConfirm {
        resource_id: String,
    },
    QuotaImport {
        project_id: String,
        samples: Vec<QuotaSample>,
    },
    Feedback {
        feedback: Feedback,
    },
    CandidateImport {
        candidates: Vec<CandidateInput>,
    },
    CandidateReview {
        id: String,
        expected_digest: String,
        reviewed: Capability,
    },
    PlanPropose {
        plan: RepairPlan,
    },
    PlanApprove {
        plan_id: String,
        digest: String,
    },
    HandoffResult {
        handoff_id: String,
        state: String,
    },
}
impl Store {
    pub fn schema_version(&self) -> Result<i64> {
        self.conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .map_err(err)
    }
    pub fn require_control(&self) -> Result<()> {
        if ![2, 3, 4, 5].contains(&self.schema_version()?) {
            return Err(Error::new("EXPLICIT_UPGRADE_REQUIRED"));
        }
        Ok(())
    }
    /// Backup is verified before the transaction. Another concurrent upgrader loses safely.
    pub fn upgrade_control_v2(&mut self) -> Result<Value> {
        let version = self.schema_version()?;
        if version == 2 {
            return Ok(json!({"from":2,"to":2,"changed":false}));
        }
        if version != 1 {
            return Err(Error::new("UNSUPPORTED_MIGRATION_SOURCE"));
        }
        let backup = self.backup()?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(err)?;
        let v: i64 = tx
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .map_err(err)?;
        if v != 1 {
            return Err(Error::new("MIGRATION_CONCURRENT_CHANGE"));
        }
        tx.execute_batch(MIGRATION).map_err(err)?;
        audit(
            &tx,
            None,
            "schema_upgraded",
            epoch(),
            "observed",
            &json!({"from":1,"to":2}),
        )?;
        tx.commit().map_err(err)?;
        Ok(
            json!({"from":1,"to":2,"changed":true,"backup":backup,"native_certification":"not_claimed"}),
        )
    }
    pub fn record(&mut self, command: Record, approval_sha: &str) -> Result<Value> {
        self.require_control()?;
        if approval_sha.len() != 64 || !approval_sha.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(Error::new("APPROVAL_DIGEST_REQUIRED"));
        }
        let at = epoch();
        let source = format!("local-record:{}", &approval_sha[..16]);
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(err)?;
        let mut project: Option<String> = None;
        let kind: &str;
        let outcome = match command {
            Record::EventRecord {
                project_id,
                event_kind,
                title,
                details,
                evidence_refs,
            } => {
                kind = "project_event_reported";
                require_project(&tx, &project_id)?;
                if ![
                    "milestone",
                    "experiment",
                    "discovery",
                    "incident",
                    "proposal_note",
                    "rejected_direction",
                ]
                .contains(&event_kind.as_str())
                {
                    return Err(Error::new("EVENT_KIND_UNSUPPORTED"));
                }
                check(text(&title, 256))?;
                check(text(&details, 2048))?;
                check(lines(&evidence_refs))?;
                project = Some(project_id);
                json!({"kind":event_kind,"title":title,"details":details,"evidence_refs":evidence_refs,"semantic_evidence":"agent_reported","decision_authority":false})
            }
            Record::EnvironmentAdd { project_id, name } => {
                kind = "environment_registered";
                check(id(&name))?;
                require_project(&tx, &project_id)?;
                tx.execute(
                    "INSERT INTO environments VALUES(?1,?2)",
                    params![project_id, name],
                )
                .map_err(err)?;
                project = Some(project_id);
                json!({"environment":name})
            }
            Record::EconomicsPreference {
                project_id,
                profile,
            } => {
                kind = "economics_preference_updated";
                require_project(&tx, &project_id)?;
                let mode = enum_name(&profile)?;
                tx.execute("UPDATE project_profiles SET economics_mode=?1,updated_at=?2 WHERE project_id=?3",params![mode,at,project_id]).map_err(err)?;
                project = Some(project_id);
                profile.advisory()
            }
            Record::FocusSet { project_id, focus } => {
                kind = "focus_updated";
                require_project(&tx, &project_id)?;
                check(lines(&focus.needs))?;
                check(lines(&focus.existing))?;
                check(lines(&focus.blocked))?;
                check(text(&focus.evidence_ref, 256))?;
                if !["windows", "linux", "macos"].contains(&focus.platform.as_str()) {
                    return Err(Error::new("FOCUS_PLATFORM_INVALID"));
                }
                tx.execute(
                    "UPDATE project_profiles SET focus_json=?1,updated_at=?2 WHERE project_id=?3",
                    params![encode(&focus)?, at, project_id],
                )
                .map_err(err)?;
                project = Some(project_id);
                json!({"focus_set":true})
            }
            Record::WorkCreate {
                id: work_id,
                project_id,
                objective,
                priority,
                workspace_id,
            } => {
                kind = "work_created";
                require_project(&tx, &project_id)?;
                if let Some(w) = &workspace_id {
                    require_workspace(&tx, &project_id, w)?;
                }
                let w = WorkItem {
                    id: work_id,
                    project_id: project_id.clone(),
                    objective,
                    state: WorkState::Planned,
                    priority,
                    version: 0,
                    workspace_id,
                    completed: vec![],
                    remaining: vec![],
                    blockers: vec![],
                    evidence: report_evidence(&source, at),
                };
                check(validate_work(&w))?;
                tx.execute(
                    "INSERT INTO work_items VALUES(?1,?2,?3,?4,?5,?6,?7)",
                    params![
                        w.id,
                        w.project_id,
                        w.state.as_str(),
                        0i64,
                        w.workspace_id,
                        encode(&w)?,
                        at
                    ],
                )
                .map_err(err)?;
                project = Some(project_id);
                json!({"work_id":w.id,"version":0})
            }
            Record::WorkUpdate {
                id: work_id,
                expected_version,
                state,
                completed,
                remaining,
                blockers,
            } => {
                kind = "work_updated";
                let mut w = load_work(&tx, &work_id)?;
                if w.version != expected_version {
                    return Err(Error::new("WORK_VERSION_CONFLICT"));
                }
                if !w.state.can_transition(state) {
                    return Err(Error::new("INVALID_WORK_TRANSITION"));
                }
                w.state = state;
                w.completed = completed;
                w.remaining = remaining;
                w.blockers = blockers;
                w.evidence = report_evidence(&source, at);
                check(validate_work(&w))?;
                bump_work(&tx, &mut w, expected_version, at)?;
                project = Some(w.project_id);
                json!({"work_id":work_id,"version":w.version,"state":state})
            }
            Record::SessionReport {
                id: session_id,
                project_id,
                agent,
                external_id,
                adapter_version,
                role,
                workspace_id,
                ended,
            } => {
                kind = "session_reported";
                require_project(&tx, &project_id)?;
                if let Some(w) = &workspace_id {
                    require_workspace(&tx, &project_id, w)?;
                }
                let s = Session {
                    id: session_id,
                    project_id: project_id.clone(),
                    agent,
                    external_id,
                    adapter_version,
                    role,
                    workspace_id,
                    last_observed: at,
                    ended_at: ended.then_some(at),
                    coverage: Coverage::Partial,
                };
                check(validate_session(&s))?;
                let previous: Option<String> = tx
                    .query_row("SELECT payload FROM sessions WHERE id=?1", [&s.id], |r| {
                        r.get(0)
                    })
                    .optional()
                    .map_err(err)?;
                if let Some(old) = previous {
                    let old: Session = decode(&old)?;
                    if old.project_id != s.project_id
                        || old.agent != s.agent
                        || old.external_id != s.external_id
                        || old.ended_at.is_some()
                    {
                        return Err(Error::new("SESSION_IDENTITY_OR_END_CONFLICT"));
                    }
                }
                tx.execute("INSERT INTO sessions VALUES(?1,?2,?3,?4,?5,?6,?7,?8) ON CONFLICT(id) DO UPDATE SET last_observed=excluded.last_observed,ended_at=excluded.ended_at,payload=excluded.payload,workspace_id=excluded.workspace_id",
      params![s.id,s.project_id,s.agent,s.external_id,s.workspace_id,s.last_observed,s.ended_at,encode(&s)?]).map_err(err)?;
                project = Some(project_id);
                json!({"session_id":s.id,"evidence":"agent_reported","coverage":"partial"})
            }
            Record::Assign {
                work_id,
                session_id,
                role,
                expected_version,
                lease_seconds,
            } => {
                kind = "worker_assigned";
                let mut w = load_work(&tx, &work_id)?;
                if !["primary", "reviewer", "observer"].contains(&role.as_str())
                    || !(30..=3600).contains(&lease_seconds)
                {
                    return Err(Error::new("ASSIGNMENT_INVALID"));
                }
                if matches!(w.state, WorkState::Done | WorkState::Cancelled) {
                    return Err(Error::new("TERMINAL_WORK_NOT_ASSIGNABLE"));
                }
                let s:String=tx.query_row("SELECT payload FROM sessions WHERE id=?1 AND project_id=?2 AND ended_at IS NULL",params![session_id,w.project_id],|r|r.get(0)).map_err(err)?;
                let s: Session = decode(&s)?;
                if w.workspace_id.is_some() && s.workspace_id != w.workspace_id {
                    return Err(Error::new("ASSIGNMENT_WORKSPACE_MISMATCH"));
                }
                let assignment = new_id();
                tx.execute(
                    "INSERT INTO assignments VALUES(?1,?2,?3,?4,?5,?6,?7,NULL)",
                    params![
                        assignment,
                        work_id,
                        w.project_id,
                        session_id,
                        role,
                        at + i64::from(lease_seconds),
                        at
                    ],
                )
                .map_err(err)?;
                bump_work(&tx, &mut w, expected_version, at)?;
                project = Some(w.project_id);
                json!({"assignment_id":assignment,"work_version":w.version,"exclusivity":"advisory_only"})
            }
            Record::ReleaseAssignment {
                assignment_id,
                work_id,
                expected_version,
            } => {
                kind = "assignment_released";
                let mut w = load_work(&tx, &work_id)?;
                let n=tx.execute("UPDATE assignments SET released_at=?1 WHERE id=?2 AND work_id=?3 AND released_at IS NULL",params![at,assignment_id,work_id]).map_err(err)?;
                if n != 1 {
                    return Err(Error::new("ASSIGNMENT_RELEASE_CONFLICT"));
                }
                bump_work(&tx, &mut w, expected_version, at)?;
                project = Some(w.project_id);
                json!({"released":assignment_id,"work_version":w.version})
            }
            Record::DecisionPropose {
                id: decision_id,
                project_id,
                topic,
                scope,
                statement,
                rationale,
                alternatives,
                consequences,
                predecessor,
                effective_at,
            } => {
                kind = "decision_proposed";
                require_project(&tx, &project_id)?;
                let d = Decision {
                    id: decision_id,
                    project_id: project_id.clone(),
                    topic,
                    scope,
                    statement,
                    rationale,
                    alternatives,
                    consequences,
                    predecessor,
                    effective_at,
                    recorded_at: at,
                    source: report_evidence(&source, at),
                };
                check(validate_decision(&d))?;
                tx.execute(
                    "INSERT INTO decisions VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
                    params![
                        d.id,
                        d.project_id,
                        d.topic,
                        d.scope,
                        d.predecessor,
                        d.effective_at,
                        d.recorded_at,
                        encode(&d)?
                    ],
                )
                .map_err(err)?;
                project = Some(project_id);
                json!({"decision_id":d.id,"status":"proposed"})
            }
            Record::DecisionAccept { decision_id } => {
                kind = "decision_accepted";
                let p: String = tx
                    .query_row(
                        "SELECT payload FROM decisions WHERE id=?1",
                        [&decision_id],
                        |r| r.get(0),
                    )
                    .map_err(err)?;
                let d: Decision = decode(&p)?;
                tx.execute(
                    "INSERT INTO decision_acceptances VALUES(?1,?2,?3,?4,?5,?6,?7)",
                    params![
                        d.id,
                        d.project_id,
                        d.topic,
                        d.scope,
                        d.predecessor,
                        at,
                        approval_sha
                    ],
                )
                .map_err(err)?;
                project = Some(d.project_id);
                json!({"decision_id":d.id,"effective_at":d.effective_at,"accepted_at":at})
            }
            Record::DecisionDispose {
                decision_id,
                disposition,
            } => {
                kind = "decision_disposition";
                if !["rejected", "withdrawn"].contains(&disposition.as_str()) {
                    return Err(Error::new("DISPOSITION_INVALID"));
                }
                let p: String = tx
                    .query_row(
                        "SELECT project_id FROM decisions WHERE id=?1",
                        [&decision_id],
                        |r| r.get(0),
                    )
                    .map_err(err)?;
                tx.execute(
                    "INSERT INTO decision_dispositions VALUES(?1,?2,?3,?4)",
                    params![decision_id, disposition, at, approval_sha],
                )
                .map_err(err)?;
                project = Some(p);
                json!({"decision_id":decision_id,"status":disposition})
            }
            Record::ResourceAdd {
                id: resource_id,
                project_id,
                environment,
                kind: resource_kind,
                name,
                locator,
                provider,
                fresh_for_secs,
            } => {
                kind = "resource_registered";
                let r = Resource {
                    id: resource_id,
                    project_id: project_id.clone(),
                    environment,
                    kind: resource_kind,
                    name,
                    locator,
                    provider,
                    evidence: report_evidence(&source, at),
                    fresh_for_secs,
                };
                check(validate_resource(&r))?;
                tx.execute(
                    "INSERT INTO resources VALUES(?1,?2,?3,?4,?5,?6)",
                    params![
                        r.id,
                        r.project_id,
                        r.environment,
                        enum_name(&r.kind)?,
                        encode(&r)?,
                        at
                    ],
                )
                .map_err(err)?;
                project = Some(project_id);
                json!({"resource_id":r.id,"verification":"unverified_registered_reference"})
            }
            Record::ResourceConfirm { resource_id } => {
                kind = "resource_confirmed";
                let p: String = tx
                    .query_row(
                        "SELECT project_id FROM resources WHERE id=?1",
                        [&resource_id],
                        |r| r.get(0),
                    )
                    .map_err(err)?;
                let v = Verification {
                    resource_id: resource_id.clone(),
                    claim: "user_confirmed".into(),
                    observed_at: at,
                    source: Evidence {
                        kind: EvidenceKind::UserApproved,
                        source: source.clone(),
                        at,
                        coverage: Coverage::Complete,
                    },
                };
                tx.execute(
                    "INSERT INTO resource_verifications VALUES(?1,?2,'user_confirmed',?3,?4)",
                    params![new_id(), resource_id, at, encode(&v)?],
                )
                .map_err(err)?;
                project = Some(p);
                json!({"resource_id":resource_id,"claim":"user_confirmed","http_and_provider_identity":"not_checked"})
            }
            Record::QuotaImport {
                project_id,
                samples,
            } => {
                kind = "quota_imported";
                require_project(&tx, &project_id)?;
                if samples.is_empty() || samples.len() > 256 {
                    return Err(Error::new("QUOTA_IMPORT_LIMIT"));
                }
                let count = samples.len();
                for mut s in samples {
                    s.id = new_id();
                    check(s.validate())?;
                    if s.observed_at > at + 60 {
                        return Err(Error::new("FUTURE_QUOTA_SAMPLE"));
                    }
                    s.evidence.kind = EvidenceKind::AgentReported;
                    s.evidence.coverage = Coverage::Partial;
                    s.evidence.source = source.clone();
                    tx.execute("INSERT INTO quota_samples VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",params![s.id,project_id,s.provider,s.account_alias,s.bucket,enum_name(&s.meter)?,s.unit,s.window_id,s.used,s.limit,s.reset_at,s.observed_at,encode(&s)?]).map_err(err)?;
                }
                // Only optional numeric observations are pruned. No decisions/work/checkpoints are pruned.
                tx.execute("DELETE FROM quota_samples WHERE project_id=?1 AND id NOT IN(SELECT id FROM quota_samples WHERE project_id=?1 ORDER BY observed_at DESC,id DESC LIMIT 10000)",[&project_id]).map_err(err)?;
                project = Some(project_id);
                json!({"imported":count,"provenance":"explicit_import_not_provider_verified"})
            }
            Record::Feedback { mut feedback } => {
                kind = "capability_feedback";
                feedback.recorded_at = at;
                check(discovery::validate_feedback(&feedback))?;
                require_project(&tx, &feedback.project_id)?;
                tx.execute(
                    "INSERT INTO capability_feedback VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
                    params![
                        feedback.id,
                        feedback.project_id,
                        feedback.resource_id,
                        feedback.need,
                        feedback.disposition,
                        feedback.until,
                        feedback.outcome,
                        feedback.evidence_ref,
                        feedback.revision,
                        at
                    ],
                )
                .map_err(err)?;
                project = Some(feedback.project_id);
                json!({"saved_feedback":feedback.id,"changes_authorized":false})
            }
            Record::CandidateImport { candidates } => {
                kind = "capability_leads_imported";
                if candidates.is_empty() || candidates.len() > 50 {
                    return Err(Error::new("CANDIDATE_IMPORT_LIMIT"));
                }
                let count = candidates.len();
                for c in candidates {
                    let c = check(c.into_unverified(new_id(), at))?;
                    let payload = encode(&c)?;
                    let digest = sha(payload.as_bytes());
                    tx.execute(
                        "INSERT INTO capability_candidates VALUES(?1,?2,?3,'unverified',?4,?5)",
                        params![c.id, digest, c.source, payload, at],
                    )
                    .map_err(err)?;
                }
                json!({"imported":count,"review_status":"unverified","activated":false})
            }
            Record::CandidateReview {
                id: candidate_id,
                expected_digest,
                mut reviewed,
            } => {
                kind = "capability_reviewed";
                let old: (String, String) = tx
                    .query_row(
                        "SELECT source_digest,canonical_url FROM capability_candidates WHERE id=?1",
                        [&candidate_id],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )
                    .map_err(err)?;
                if old.0 != expected_digest
                    || reviewed.id != candidate_id
                    || reviewed.source != old.1
                {
                    return Err(Error::new("CANDIDATE_REVIEW_CHANGED"));
                }
                let builtins: Vec<Capability> =
                    decode(include_str!("../../../catalog/resources.json"))?;
                let reviewed_others:i64=tx.query_row("SELECT count(*) FROM capability_candidates WHERE review_status='reviewed' AND id!=?1",[&candidate_id],|r|r.get(0)).map_err(err)?;
                if builtins.len() as i64 + reviewed_others >= 100 {
                    return Err(Error::new("CATALOG_REVIEW_CAPACITY"));
                }
                reviewed.review_status = "reviewed".into();
                reviewed.reviewed_at = at;
                check(reviewed.validate())?;
                tx.execute("UPDATE capability_candidates SET review_status='reviewed',payload=?1,source_digest=?2 WHERE id=?3 AND source_digest=?4",params![encode(&reviewed)?,sha(encode(&reviewed)?.as_bytes()),candidate_id,expected_digest]).map_err(err)?;
                json!({"reviewed":candidate_id,"adoption_mode":"reference_only"})
            }
            Record::PlanPropose { plan } => {
                kind = "repair_plan_proposed";
                check(plan.validate())?;
                require_project(&tx, &plan.project_id)?;
                if (plan.created_at - at).abs() > 60 {
                    return Err(Error::new("PLAN_CREATION_STALE"));
                }
                let payload = encode(&plan)?;
                let digest = sha(payload.as_bytes());
                tx.execute(
                    "INSERT INTO plans VALUES(?1,?2,?3,?4,?5,?6,'proposed',?7)",
                    params![
                        plan.id,
                        plan.project_id,
                        digest,
                        plan.created_at,
                        plan.expires_at,
                        plan.policy_version,
                        payload
                    ],
                )
                .map_err(err)?;
                project = Some(plan.project_id);
                json!({"plan_id":plan.id,"digest":digest,"execution":"blocked_pending_native_recipe_certification"})
            }
            Record::PlanApprove { plan_id, digest } => {
                kind = "repair_plan_approved_but_blocked";
                let p:(String,String,i64)=tx.query_row("SELECT project_id,digest,expires_at FROM plans WHERE id=?1 AND state='proposed'",[&plan_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(err)?;
                if p.1 != digest || p.2 <= at {
                    return Err(Error::new("PLAN_CHANGED_OR_EXPIRED"));
                }
                tx.execute(
                    "INSERT INTO approval_events VALUES(?1,?2,?3,?4,?5)",
                    params![new_id(), plan_id, digest, at, source],
                )
                .map_err(err)?;
                tx.execute("UPDATE plans SET state='blocked' WHERE id=?1", [&plan_id])
                    .map_err(err)?;
                project = Some(p.0);
                json!({"plan_id":plan_id,"state":"blocked","reason":"native_recipe_not_certified","external_changes":false})
            }
            Record::HandoffResult { handoff_id, state } => {
                kind = "handoff_result_reported";
                if !["delivered_reported", "failed_reported", "stale"].contains(&state.as_str()) {
                    return Err(Error::new("HANDOFF_STATE_INVALID"));
                }
                let p: String = tx
                    .query_row(
                        "SELECT project_id FROM handoffs WHERE id=?1 AND state='prepared'",
                        [&handoff_id],
                        |r| r.get(0),
                    )
                    .map_err(err)?;
                if tx.execute("UPDATE handoffs SET state=?1,approval_ref=?2 WHERE id=?3 AND state='prepared'",params![state,approval_sha,handoff_id]).map_err(err)?!=1{return Err(Error::new("HANDOFF_CONCURRENT_CHANGE"));}
                project = Some(p);
                json!({"handoff_id":handoff_id,"state":state,"primary_worker_changed":false})
            }
        };
        audit(
            &tx,
            project.as_deref(),
            kind,
            at,
            "user_approved",
            &json!({"result":outcome,"approval_sha256":approval_sha,"authority_boundary":"local_CLI_not_same_user_sandbox"}),
        )?;
        tx.commit().map_err(err)?;
        Ok(outcome)
    }
    pub fn work(&self, id: &str) -> Result<WorkItem> {
        self.require_control()?;
        load_work(&self.conn, id)
    }
    pub fn work_items(&self, project: &str) -> Result<Vec<WorkItem>> {
        self.require_control()?;
        require_project(&self.conn, project)?;
        load_json(&self.conn,"SELECT payload FROM work_items WHERE project_id=?1 ORDER BY updated_at DESC,id LIMIT 101",project,100)
    }
    pub fn roster(&self, project: &str) -> Result<Vec<RosterEntry>> {
        self.require_control()?;
        require_project(&self.conn, project)?;
        let mut q=self.conn.prepare("SELECT s.payload,a.work_id,COALESCE(a.role,'observer'),a.lease_until FROM sessions s LEFT JOIN assignments a ON a.session_id=s.id AND a.released_at IS NULL WHERE s.project_id=?1 ORDER BY s.last_observed DESC,s.id LIMIT 101").map_err(err)?;
        let rows = q
            .query_map([project], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, Option<i64>>(3)?,
                ))
            })
            .map_err(err)?;
        let mut out = vec![];
        for row in rows {
            let (s, work_id, role, lease_until) = row.map_err(err)?;
            out.push(RosterEntry {
                session: decode(&s)?,
                work_id,
                role,
                lease_until,
            });
        }
        if out.len() > 100 {
            return Err(Error::new("ROSTER_LIMIT_NARROW_SCOPE"));
        }
        Ok(out)
    }
    pub fn decisions(&self, project: &str, valid_at: i64, known_at: i64) -> Result<Vec<Decision>> {
        self.require_control()?;
        require_project(&self.conn, project)?;
        check(timestamp(valid_at))?;
        check(timestamp(known_at))?;
        let mut q = self.conn.prepare(CURRENT_DECISIONS).map_err(err)?;
        let rows = q
            .query_map(params![project, valid_at, known_at], |r| {
                r.get::<_, String>(0)
            })
            .map_err(err)?;
        let mut out = vec![];
        let mut scopes = std::collections::BTreeSet::new();
        for r in rows {
            let d: Decision = decode(&r.map_err(err)?)?;
            if !scopes.insert((d.topic.clone(), d.scope.clone())) {
                return Err(Error::new("DECISION_CONFLICT"));
            }
            out.push(d);
        }
        if out.len() > 100 {
            return Err(Error::new("DECISION_CONTEXT_LIMIT"));
        }
        Ok(out)
    }
    pub fn resources(&self, project: &str, environment: &str) -> Result<Vec<Resource>> {
        self.require_control()?;
        require_project(&self.conn, project)?;
        check(id(environment))?;
        let exists: bool = self
            .conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM environments WHERE project_id=?1 AND name=?2)",
                params![project, environment],
                |r| r.get(0),
            )
            .map_err(err)?;
        if !exists {
            return Err(Error::new("ENVIRONMENT_NOT_REGISTERED_NO_FALLBACK"));
        }
        let mut q=self.conn.prepare("SELECT payload FROM resources WHERE project_id=?1 AND environment=?2 ORDER BY id LIMIT 101").map_err(err)?;
        let rows = q
            .query_map(params![project, environment], |r| r.get::<_, String>(0))
            .map_err(err)?;
        let mut out = vec![];
        for r in rows {
            out.push(decode(&r.map_err(err)?)?);
        }
        if out.len() > 100 {
            return Err(Error::new("RESOURCE_CONTEXT_LIMIT"));
        }
        Ok(out)
    }
    pub fn timeline(&self, project: &str, before_seq: Option<i64>) -> Result<Value> {
        self.require_control()?;
        require_project(&self.conn, project)?;
        let mut q=self.conn.prepare("SELECT seq,event_id,kind,recorded_at,source_kind,payload FROM audit_events WHERE project_id=?1 AND seq<?2 ORDER BY seq DESC LIMIT 51").map_err(err)?;
        let rows = q
            .query_map(params![project, before_seq.unwrap_or(i64::MAX)], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, String>(5)?,
                ))
            })
            .map_err(err)?;
        let mut out = vec![];
        for r in rows {
            let (s, i, k, t, e, p) = r.map_err(err)?;
            out.push(json!({"seq":s,"id":i,"kind":k,"recorded_at":t,"source_kind":e,"data":decode::<Value>(&p)?}));
        }
        let more = out.len() > 50;
        if more {
            out.truncate(50);
        }
        let next = if more {
            out.last().and_then(|v| v["seq"].as_i64())
        } else {
            None
        };
        Ok(json!({"events":out,"next_before_seq":next}))
    }
    pub fn quota_rows(&self, project: &str) -> Result<Vec<QuotaSample>> {
        self.require_control()?;
        require_project(&self.conn, project)?;
        load_json(&self.conn,"SELECT payload FROM quota_samples WHERE project_id=?1 ORDER BY observed_at DESC,id LIMIT 10001",project,10000)
    }
    pub fn economics(&self, project: &str, at: i64) -> Result<Value> {
        self.require_control()?;
        require_project(&self.conn, project)?;
        let mode: String = self
            .conn
            .query_row(
                "SELECT economics_mode FROM project_profiles WHERE project_id=?1",
                [project],
                |r| r.get(0),
            )
            .map_err(err)?;
        let samples = self.quota_rows(project)?;
        let mut buckets: std::collections::BTreeMap<String, Vec<QuotaSample>> =
            std::collections::BTreeMap::new();
        for s in samples {
            buckets
                .entry(format!(
                    "{}:{}:{}:{:?}:{}",
                    s.provider, s.account_alias, s.bucket, s.meter, s.unit
                ))
                .or_default()
                .push(s);
        }
        let mut out = vec![];
        for (key, rows) in buckets {
            match economics::forecast(&rows, at) {
                Ok(r) => out.push(json!({"stream":key,"runway":r})),
                Err(e) => out.push(json!({"stream":key,"status":"unknown","reason":e})),
            }
        }
        let billing = self.cost_reports(project)?;
        Ok(
            json!({"reported_telemetry":self.telemetry_summary(project)?,"organization_cost_reports":billing,"preference_mode":mode,"automatic_switching":false,"streams":out,"live_provider_collection":"explicit_effect_only_no_polling","independent_meters":true,"as_of":at}),
        )
    }
    pub fn suggestions(&self, project: &str, at: i64) -> Result<Value> {
        self.require_control()?;
        require_project(&self.conn, project)?;
        let focus: Option<String> = self
            .conn
            .query_row(
                "SELECT focus_json FROM project_profiles WHERE project_id=?1",
                [project],
                |r| r.get(0),
            )
            .map_err(err)?;
        let Some(focus) = focus else {
            return Ok(json!({"useful_discoveries":[],"reason":"approved_focus_missing"}));
        };
        let mut focus: Focus = decode(&focus)?;
        let observed_availability = self.available_capabilities(project)?;
        for item in &observed_availability {
            if item.get("state").and_then(Value::as_str) == Some("available") {
                if let Some(id) = item.get("resource_id").and_then(Value::as_str) {
                    if !focus.existing.iter().any(|v| v == id) {
                        focus.existing.push(id.into());
                    }
                }
            }
        }
        let mut catalog: Vec<Capability> = decode(include_str!("../../../catalog/resources.json"))?;
        let capacity = 100usize
            .checked_sub(catalog.len())
            .ok_or(Error::new("BUILTIN_CATALOG_LIMIT"))?;
        let mut query=self.conn.prepare("SELECT payload FROM capability_candidates WHERE review_status='reviewed' ORDER BY id LIMIT ?1").map_err(err)?;
        let rows = query
            .query_map([(capacity + 1) as i64], |row| row.get::<_, String>(0))
            .map_err(err)?;
        let mut extra = Vec::<Capability>::new();
        for row in rows {
            extra.push(decode(&row.map_err(err)?)?);
        }
        let catalog_omitted = extra.len() > capacity;
        extra.truncate(capacity);
        catalog.extend(extra);
        let feedback = self.feedback_rows(project)?;
        let cards = check(discovery::recommend(
            project, &focus, &catalog, &feedback, at,
        ))?;
        Ok(
            json!({"useful_discoveries":cards,"source":"local_catalog","catalog_capacity_exceeded":catalog_omitted,"cached_inventory":observed_availability,"network_calls":0,"research_brief":discovery::research_brief(&focus)}),
        )
    }
    fn feedback_rows(&self, project: &str) -> Result<Vec<Feedback>> {
        let mut q=self.conn.prepare("SELECT id,project_id,resource_id,need,disposition,until_time,outcome,evidence_ref,revision,recorded_at FROM capability_feedback WHERE project_id=?1 ORDER BY recorded_at DESC,id LIMIT 1001").map_err(err)?;
        let rows = q
            .query_map([project], |r| {
                Ok(Feedback {
                    id: r.get(0)?,
                    project_id: r.get(1)?,
                    resource_id: r.get(2)?,
                    need: r.get(3)?,
                    disposition: r.get(4)?,
                    until: r.get(5)?,
                    outcome: r.get(6)?,
                    evidence_ref: r.get(7)?,
                    revision: r.get(8)?,
                    recorded_at: r.get(9)?,
                })
            })
            .map_err(err)?;
        let out = rows
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(err)?;
        if out.len() > 1000 {
            return Err(Error::new("FEEDBACK_BUDGET_REVIEW_REQUIRED"));
        }
        Ok(out)
    }
    pub fn context(&self, project: &str, environment: Option<&str>) -> Result<Value> {
        self.require_control()?;
        require_project(&self.conn, project)?;
        let tx = self.conn.unchecked_transaction().map_err(err)?;
        let at = epoch();
        let works = self.work_items(project)?;
        let roster = self.roster(project)?;
        let decisions = self.decisions(project, at, at)?;
        let resources = match environment {
            Some(e) => self.resources(project, e)?,
            None => vec![],
        };
        let mut optional_failures = Vec::new();
        let discovery = optional_context(
            "useful_discoveries",
            self.suggestions(project, at),
            &mut optional_failures,
        );
        let economics = optional_context(
            "economics",
            self.economics(project, at),
            &mut optional_failures,
        );
        let integrations = optional_context(
            "integrations",
            self.integration_summary(project),
            &mut optional_failures,
        );
        let health_profiles = if self.schema_version()? >= 4 {
            optional_context(
                "health_profiles",
                self.cached_observation(project, "health-v4"),
                &mut optional_failures,
            )
        } else {
            Value::Null
        };
        let graph = optional_context(
            "workspace_graph",
            self.graph_context(project),
            &mut optional_failures,
        );
        let events = optional_context(
            "recent_events",
            self.timeline(project, None),
            &mut optional_failures,
        );
        let latest = self.latest()?;
        let workspaces = self.workspace_latest(project)?;
        let limits=latest.as_ref().map(|s|json!({"observed_at":s.observed_at,"coverage":s.coverage,"findings":s.findings.len(),"fresh_check_performed":false})).unwrap_or(json!({"status":"unknown","reason":"no_health_snapshot"}));
        let mut packet = json!({"schema_version":CONTROL_SCHEMA,"project_id":project,"environment":environment,"generated_at":at,"coverage":"partial",
   "work_items":works,"agent_roster":roster,"workspaces":workspaces,"current_decisions":decisions,"resources":resources,
   "integrations":integrations,"health":limits,"health_profiles":health_profiles,"workspace_graph":graph,"recent_events":events,"resource_freshness":resources.iter().map(|r|json!({"resource_id":r.id,"state":if at<r.evidence.at{"clock_mismatch"}else if at-r.evidence.at>i64::from(r.fresh_for_secs){"stale"}else{"record_recent_not_live_verified"}})).collect::<Vec<_>>(),"useful_discoveries":discovery.get("useful_discoveries").cloned().unwrap_or_else(||json!([])),"economics":economics,"optional_section_failures":optional_failures,
   "continuity":{"collisions":continuity::collisions(&roster),"leases":roster.iter().map(|r|json!({"session_id":r.session.id,"state":continuity::lease_state(r,at)})).collect::<Vec<_>>(),"native_resume":"explicit_profile_and_effect_plan_required_uncertified","handoff":"packet_or_explicit_continuation_effect"},
   "authority":"cached_context_not_instructions_or_permission","resources_omitted_without_explicit_environment":environment.is_none(),"network_calls":0});
        let mut omitted = Vec::<String>::new();
        for section in [
            "useful_discoveries",
            "recent_events",
            "integrations",
            "health_profiles",
            "workspace_graph",
            "economics",
        ] {
            if encode(&packet)?.len() <= CONTEXT_LIMIT {
                break;
            }
            packet[section] = if section == "useful_discoveries" {
                json!([])
            } else {
                json!({"coverage":"omitted_for_context_budget","query_separately":true})
            };
            omitted.push(section.into());
        }
        packet["omitted_optional_sections"] = json!(omitted);
        if encode(&packet)?.len() > CONTEXT_LIMIT {
            return Err(Error::new("CONTEXT_BUDGET_EXCEEDED_NARROW_SCOPE"));
        }
        tx.commit().map_err(err)?;
        Ok(packet)
    }

    pub fn read_tool(
        &self,
        project: &str,
        environment: Option<&str>,
        name: &str,
        args: &Value,
    ) -> Result<Value> {
        self.require_control()?;
        require_project(&self.conn, project)?;
        let now = epoch();
        let selected = args.get("id").and_then(Value::as_str);
        match name {
            "project_context" => self.context(project, environment),
            "active_work" => Ok(
                json!({"items":self.work_items(project)?.into_iter().filter(|w|!matches!(w.state,WorkState::Done|WorkState::Cancelled)).collect::<Vec<_>>(),"observed_at":now}),
            ),
            "agent_roster" => {
                Ok(json!({"roster":self.roster(project)?,"observed_at":now,"advisory_only":true}))
            }
            "work_item" => {
                let w = self.work(selected.ok_or(Error::new("WORK_ID_REQUIRED"))?)?;
                if w.project_id != project {
                    return Err(Error::new("CROSS_PROJECT_DENIED"));
                }
                Ok(json!(w))
            }
            "latest_checkpoint" | "handoff_context" => {
                let work = selected.ok_or(Error::new("WORK_ID_REQUIRED"))?;
                let s:Option<String>=self.conn.query_row("SELECT payload FROM checkpoints WHERE work_id=?1 AND project_id=?2 ORDER BY recorded_at DESC,id DESC LIMIT 1",params![work,project],|r|r.get(0)).optional().map_err(err)?;
                let cp = match s {
                    Some(v) => Some(decode::<Checkpoint>(&v)?),
                    None => None,
                };
                Ok(
                    json!({"checkpoint":cp,"fresh_git_check":false,"continuation_permission":false,"mode":"cached_review_only"}),
                )
            }
            "current_decisions" => Ok(
                json!({"decisions":self.decisions(project,args.get("valid_at").and_then(Value::as_i64).unwrap_or(now),args.get("known_at").and_then(Value::as_i64).unwrap_or(now))?}),
            ),
            "project_timeline" => {
                self.timeline(project, args.get("before").and_then(Value::as_i64))
            }
            "resource_lookup" => {
                let env = environment.ok_or(Error::new("EXPLICIT_ENVIRONMENT_REQUIRED"))?;
                let rows = self.resources(project, env)?;
                Ok(
                    json!({"resources":rows.into_iter().filter(|r|selected.is_none_or(|id|id==r.id)).collect::<Vec<_>>(),"secret_values_resolved":false}),
                )
            }
            "economics_summary" => self.economics(project, now),
            "capability_explain" => self.suggestions(project, now),
            "workspace_status" => Ok(
                json!({"workspaces":self.workspace_latest(project)?,"all_protected":true,"fresh_check":false}),
            ),
            "health_latest" => {
                let s = self.latest()?;
                Ok(
                    json!({"snapshot_time":s.as_ref().map(|s|&s.observed_at),"finding_count":s.as_ref().map(|s|s.findings.len()),"coverage":s.as_ref().map(|s|s.coverage),"fresh_check":false,"machine_details_omitted_from_project_scope":true}),
                )
            }
            _ => Err(Error::new("READ_ONLY_TOOL_UNSUPPORTED")),
        }
    }
    pub fn register_workspace(
        &mut self,
        project: &str,
        workspace: &str,
        path: &Path,
        directory_identity: &str,
    ) -> Result<()> {
        self.require_control()?;
        check(id(workspace))?;
        require_project(&self.conn, project)?;
        self.conn
            .execute(
                "INSERT INTO workspace_registrations VALUES(?1,?2,?3,?4)",
                params![
                    project,
                    workspace,
                    path.to_string_lossy(),
                    directory_identity
                ],
            )
            .map_err(err)?;
        Ok(())
    }
    pub fn record_workspace_observation(&mut self, stamp: &WorkspaceStamp) -> Result<()> {
        self.require_control()?;
        check(continuity::validate_stamp(stamp))?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(err)?;
        require_workspace(&tx, &stamp.project_id, &stamp.workspace_id)?;
        tx.execute(
            "INSERT INTO workspace_observations VALUES(?1,?2,?3,?4,'observed',?5)",
            params![
                new_id(),
                stamp.project_id,
                stamp.workspace_id,
                stamp.observed_at,
                encode(stamp)?
            ],
        )
        .map_err(err)?;
        tx.execute("DELETE FROM workspace_observations WHERE project_id=?1 AND id NOT IN(SELECT id FROM workspace_observations WHERE project_id=?1 ORDER BY observed_at DESC,id DESC LIMIT 256)",[&stamp.project_id]).map_err(err)?;
        tx.commit().map_err(err)?;
        Ok(())
    }
    pub fn workspace_latest(&self, project: &str) -> Result<Vec<Value>> {
        self.require_control()?;
        require_project(&self.conn, project)?;
        let mut q=self.conn.prepare("SELECT w.id,w.path,(SELECT o.payload FROM workspace_observations o WHERE o.project_id=w.project_id AND o.workspace_id=w.id ORDER BY o.observed_at DESC,o.id DESC LIMIT 1) FROM workspace_registrations w WHERE w.project_id=?1 ORDER BY w.id LIMIT 257").map_err(err)?;
        let rows = q
            .query_map([project], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?,
                ))
            })
            .map_err(err)?;
        let mut out = vec![];
        for r in rows {
            let (id, path, payload) = r.map_err(err)?;
            let observation = payload.map(|p| decode::<Value>(&p)).transpose()?;
            out.push(json!({"workspace_id":id,"path":path,"observation":observation,"protection":"protected","fresh_check":false}));
        }
        if out.len() > 256 {
            return Err(Error::new("WORKSPACE_QUERY_BUDGET"));
        }
        Ok(out)
    }
    pub fn workspace_path(&self, project: &str, workspace: &str) -> Result<String> {
        self.require_control()?;
        self.conn
            .query_row(
                "SELECT path FROM workspace_registrations WHERE project_id=?1 AND id=?2",
                params![project, workspace],
                |r| r.get(0),
            )
            .map_err(err)
    }
    pub fn save_checkpoint(&mut self, cp: &Checkpoint) -> Result<()> {
        self.require_control()?;
        check(continuity::validate_stamp(&cp.workspace))?;
        check(validate_work(&cp.work))?;
        check(lines(&cp.completed))?;
        check(lines(&cp.remaining))?;
        check(lines(&cp.blockers))?;
        if cp.tests.len() > 16 || cp.decision_ids.len() > 32 || cp.resource_ids.len() > 32 {
            return Err(Error::new("CHECKPOINT_COUNT_LIMIT"));
        }
        for t in &cp.tests {
            check(text(&t.name, 256))?;
            check(text(&t.result, 256))?;
        }
        if cp.workspace.project_id != cp.work.project_id
            || cp.work.workspace_id.as_deref() != Some(cp.workspace.workspace_id.as_str())
        {
            return Err(Error::new("CHECKPOINT_PROJECT_WORKSPACE_MISMATCH"));
        }
        let payload = encode(cp)?;
        if payload.len() > HANDOFF_LIMIT {
            return Err(Error::new("CHECKPOINT_SIZE_LIMIT"));
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(err)?;
        let work = load_work(&tx, &cp.work.id)?;
        if work.version != cp.work.version {
            return Err(Error::new("WORK_CHANGED_DURING_CHECKPOINT"));
        }
        for r in &cp.resource_ids {
            let valid: bool = tx
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM resources WHERE id=?1 AND project_id=?2)",
                    params![r, cp.work.project_id],
                    |q| q.get(0),
                )
                .map_err(err)?;
            if !valid {
                return Err(Error::new("CROSS_PROJECT_RESOURCE"));
            }
        }
        for d in &cp.decision_ids {
            let valid: bool = tx
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM decisions WHERE id=?1 AND project_id=?2)",
                    params![d, cp.work.project_id],
                    |q| q.get(0),
                )
                .map_err(err)?;
            if !valid {
                return Err(Error::new("CROSS_PROJECT_DECISION"));
            }
        }
        tx.execute(
            "INSERT INTO checkpoints VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![
                cp.id,
                cp.work.id,
                cp.work.project_id,
                cp.session_id,
                cp.workspace.workspace_id,
                cp.workspace.head,
                sha(encode(&cp.workspace)?.as_bytes()),
                cp.recorded_at,
                payload
            ],
        )
        .map_err(err)?;
        audit(
            &tx,
            Some(&cp.work.project_id),
            "checkpoint_created",
            cp.recorded_at,
            "observed",
            &json!({"checkpoint_id":cp.id,"progress_evidence":"agent_reported","workspace_evidence":"observed"}),
        )?;
        tx.commit().map_err(err)?;
        Ok(())
    }
    pub fn checkpoint(&self, id: &str) -> Result<Checkpoint> {
        self.require_control()?;
        let s: String = self
            .conn
            .query_row("SELECT payload FROM checkpoints WHERE id=?1", [id], |r| {
                r.get(0)
            })
            .map_err(err)?;
        decode(&s)
    }
    pub fn save_handoff(&mut self, packet: &continuity::HandoffPacket) -> Result<Value> {
        let digest = sha(encode(packet)?.as_bytes());
        let id = new_id();
        let at = epoch();
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(err)?;
        tx.execute(
            "INSERT INTO handoffs VALUES(?1,?2,?3,?4,?5,NULL,?6,'prepared')",
            params![
                id,
                packet.checkpoint_id,
                packet.project_id,
                packet.target_agent,
                digest,
                at
            ],
        )
        .map_err(err)?;
        audit(
            &tx,
            Some(&packet.project_id),
            "handoff_prepared",
            at,
            "observed",
            &json!({"handoff_id":id,"packet_digest":digest,"mode":"packet_only"}),
        )?;
        tx.commit().map_err(err)?;
        Ok(json!({"handoff_id":id,"packet_sha256":digest,"packet":packet,"launch_performed":false}))
    }
}
fn enum_name<T: Serialize>(value: &T) -> Result<String> {
    serde_json::to_value(value)
        .ok()
        .and_then(|x| x.as_str().map(str::to_owned))
        .ok_or(Error::new("ENUM_ENCODING"))
}
fn require_workspace(c: &Connection, p: &str, w: &str) -> Result<()> {
    check(id(w))?;
    if !c
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM workspace_registrations WHERE project_id=?1 AND id=?2)",
            params![p, w],
            |r| r.get::<_, bool>(0),
        )
        .map_err(err)?
    {
        return Err(Error::new("WORKSPACE_NOT_REGISTERED"));
    }
    Ok(())
}
fn load_json<T: serde::de::DeserializeOwned>(
    c: &Connection,
    sql: &str,
    project: &str,
    max: usize,
) -> Result<Vec<T>> {
    let mut q = c.prepare(sql).map_err(err)?;
    let rows = q
        .query_map([project], |r| r.get::<_, String>(0))
        .map_err(err)?;
    let mut out = vec![];
    for row in rows {
        out.push(decode(&row.map_err(err)?)?);
    }
    if out.len() > max {
        return Err(Error::new("CONTROL_QUERY_BUDGET"));
    }
    Ok(out)
}
#[cfg(test)]
mod audit_context_tests {
    use super::*;
    #[test]
    fn optional_failure_preserves_typed_reason_without_error_dump() {
        let mut failures = vec![];
        let result: Result<Value> = Err(Error::new("CATALOG_INVALID"));
        let value = optional_context("discoveries", result, &mut failures);
        assert_eq!(value["error_code"], "CATALOG_INVALID");
        assert_eq!(failures.len(), 1);
        assert_eq!(value["available"], false);
    }
    #[test]
    fn optional_success_preserves_value() {
        let mut failures = vec![];
        let value = optional_context("events", Ok(json!({"events":[]})), &mut failures);
        assert!(value["events"].is_array());
        assert!(failures.is_empty());
    }
}
