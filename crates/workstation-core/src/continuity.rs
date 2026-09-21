//! Cross-agent work packets, not proprietary transcript conversion or agent orchestration.
use crate::control::*;
use crate::Coverage;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RosterEntry {
    pub session: Session,
    pub work_id: Option<String>,
    pub role: String,
    pub lease_until: Option<i64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Collision {
    pub first: String,
    pub second: String,
    pub reason: String,
    pub disposition: String,
}
/// An expired lease makes ownership uncertain, not abandoned. It still participates in warnings.
pub fn collisions(roster: &[RosterEntry]) -> Vec<Collision> {
    let mut out = Vec::new();
    for (i, a) in roster.iter().enumerate() {
        if a.session.ended_at.is_some() {
            continue;
        }
        for b in &roster[i + 1..] {
            if b.session.ended_at.is_some()
                || a.session.id == b.session.id
                || a.session.project_id != b.session.project_id
            {
                continue;
            }
            let same_work = a.work_id.is_some() && a.work_id == b.work_id;
            let same_workspace = a.session.workspace_id.is_some()
                && a.session.workspace_id == b.session.workspace_id;
            if (same_work || same_workspace) && a.role == "primary" && b.role == "primary" {
                out.push(Collision {
                    first: a.session.id.clone(),
                    second: b.session.id.clone(),
                    reason: if same_workspace {
                        "same_worktree"
                    } else {
                        "same_work_item"
                    }
                    .into(),
                    disposition: "coordinate_no_automatic_termination".into(),
                });
            }
        }
    }
    out
}
pub fn lease_state(entry: &RosterEntry, now: i64) -> &'static str {
    if timestamp(now).is_err()
        || timestamp(entry.session.last_observed).is_err()
        || entry.session.last_observed > now
        || entry.lease_until.is_some_and(|t| timestamp(t).is_err())
        || entry
            .session
            .ended_at
            .is_some_and(|t| timestamp(t).is_err() || t > now)
    {
        return "ownership_uncertain";
    }
    if entry.session.ended_at.is_some() {
        "reported_ended"
    } else if entry.lease_until.is_some_and(|t| t <= now) || now - entry.session.last_observed > 900
    {
        "ownership_uncertain"
    } else {
        "recently_reported_not_exclusive_lock"
    }
}
pub fn validate_stamp(stamp: &WorkspaceStamp) -> Check<()> {
    id(&stamp.project_id)?;
    id(&stamp.workspace_id)?;
    text(&stamp.path, 32700)?;
    if ![40, 64].contains(&stamp.head.len()) || !stamp.head.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("HEAD_INVALID");
    }
    if stamp.directory_identity.is_empty()
        || stamp.status_digest.len() != 64
        || !stamp.status_digest.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err("STAMP_IDENTITY_INVALID");
    }
    timestamp(stamp.observed_at)?;
    lines(&stamp.blockers)
}
pub fn revalidate_checkpoint(cp: &Checkpoint, live: &WorkspaceStamp, now: i64) -> Check<()> {
    timestamp(now)?;
    timestamp(cp.recorded_at)?;
    validate_stamp(&cp.workspace)?;
    validate_stamp(live)?;
    if now < cp.recorded_at || now - live.observed_at > 60 || live.observed_at > now {
        return Err("LIVE_OBSERVATION_REQUIRED");
    }
    if cp.workspace.coverage != Coverage::Complete || live.coverage != Coverage::Complete {
        return Err("HANDOFF_COVERAGE_INCOMPLETE");
    }
    if cp.workspace.project_id != live.project_id
        || cp.workspace.workspace_id != live.workspace_id
        || cp.workspace.directory_identity != live.directory_identity
        || cp.workspace.path != live.path
        || cp.workspace.head != live.head
        || cp.workspace.branch != live.branch
        || cp.workspace.status_digest != live.status_digest
    {
        return Err("HANDOFF_STALE");
    }
    if !live.blockers.is_empty() {
        return Err("WORKSPACE_OPERATION_BLOCKED");
    }
    Ok(())
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HandoffPacket {
    pub schema_version: String,
    pub checkpoint_id: String,
    pub work_id: String,
    pub project_id: String,
    pub target_agent: String,
    pub mode: String,
    pub goal: String,
    pub state: WorkState,
    pub workspace: WorkspaceStamp,
    pub completed: Vec<String>,
    pub remaining: Vec<String>,
    pub blockers: Vec<String>,
    pub tests: Vec<TestObservation>,
    pub decision_ids: Vec<String>,
    pub resource_ids: Vec<String>,
    pub roster: Vec<RosterEntry>,
    pub collisions: Vec<Collision>,
    pub content_revalidation_required: bool,
    pub freshness_basis: String,
    pub created_at: i64,
    pub next_action: String,
    pub privacy_notice: String,
    pub authority: String,
}
/// Default fallback is deliberately useful without live vendor SDKs.
/// It never changes the primary worker and never creates a vendor session.
pub fn handoff(
    cp: &Checkpoint,
    live: &WorkspaceStamp,
    target: &str,
    roster: &[RosterEntry],
    now: i64,
) -> Check<HandoffPacket> {
    id(target)?;
    revalidate_checkpoint(cp, live, now)?;
    validate_work(&cp.work)?;
    if roster.len() > 50
        || cp.tests.len() > 16
        || cp.decision_ids.len() > 32
        || cp.resource_ids.len() > 32
    {
        return Err("HANDOFF_COUNT_LIMIT");
    }
    for collection in [&cp.completed, &cp.remaining, &cp.blockers] {
        lines(collection)?;
    }
    for test in &cp.tests {
        text(&test.name, 256)?;
        text(&test.result, 256)?;
        evidence(&test.evidence)?;
    }
    let p=HandoffPacket{schema_version:CONTROL_SCHEMA.into(),checkpoint_id:cp.id.clone(),work_id:cp.work.id.clone(),project_id:cp.work.project_id.clone(),
        target_agent:target.into(),mode:"packet_only".into(),content_revalidation_required:true,freshness_basis:"Git HEAD, status and changed-path metadata; not byte-identical file contents".into(),goal:cp.work.objective.clone(),state:cp.work.state,workspace:live.clone(),
        completed:cp.completed.clone(),remaining:cp.remaining.clone(),blockers:cp.blockers.clone(),tests:cp.tests.clone(),
        decision_ids:cp.decision_ids.clone(),resource_ids:cp.resource_ids.clone(),roster:roster.to_vec(),collisions:collisions(roster),created_at:now,
        next_action:"Verify repository identity, HEAD, working changes and the reported test state before continuing. Coordinate overlapping work.".into(),
        privacy_notice:"Private operator-reviewed continuation packet; no transcripts or credential values resolved. User-entered text still requires disclosure review.".into(),
        authority:"context_only_no_session_launch_no_primary_transfer_no_plan_approval".into()};
    let bytes = serde_json::to_vec(&p).map_err(|_| "HANDOFF_ENCODING")?;
    if bytes.len() > HANDOFF_LIMIT {
        return Err("HANDOFF_TOO_LARGE");
    }
    Ok(p)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn session(id: &str) -> RosterEntry {
        RosterEntry {
            session: Session {
                id: id.into(),
                project_id: "p".into(),
                agent: "test".into(),
                external_id: id.into(),
                adapter_version: "fixture".into(),
                role: "worker".into(),
                workspace_id: Some("wt".into()),
                last_observed: 100,
                ended_at: None,
                coverage: Coverage::Partial,
            },
            work_id: Some(id.into()),
            role: "primary".into(),
            lease_until: Some(110),
        }
    }
    #[test]
    fn expired_lease_still_protects() {
        assert_eq!(lease_state(&session("a"), 200), "ownership_uncertain");
    }
    #[test]
    fn simultaneous_workers_warn() {
        assert_eq!(collisions(&[session("a"), session("b")]).len(), 1);
    }
    #[test]
    fn reviewers_not_second_writer() {
        let mut b = session("b");
        b.role = "reviewer".into();
        assert!(collisions(&[session("a"), b]).is_empty());
    }
    #[test]
    fn ended_session_not_active_collision() {
        let mut b = session("b");
        b.session.ended_at = Some(105);
        assert!(collisions(&[session("a"), b]).is_empty());
    }
    #[test]
    fn other_project_not_collision() {
        let mut b = session("b");
        b.session.project_id = "other".into();
        assert!(collisions(&[session("a"), b]).is_empty());
    }
}

#[cfg(test)]
mod audit_lease_tests {
    use super::*;
    fn entry() -> RosterEntry {
        RosterEntry {
            session: Session {
                id: "s".into(),
                project_id: "p".into(),
                agent: "a".into(),
                external_id: "vendor".into(),
                adapter_version: "fixture".into(),
                role: "worker".into(),
                workspace_id: None,
                last_observed: 100,
                ended_at: None,
                coverage: Coverage::Partial,
            },
            work_id: None,
            role: "primary".into(),
            lease_until: Some(200),
        }
    }
    #[test]
    fn future_activity_is_not_current_ownership() {
        assert_eq!(lease_state(&entry(), 99), "ownership_uncertain");
    }
    #[test]
    fn exact_expiry_is_uncertain() {
        assert_eq!(lease_state(&entry(), 200), "ownership_uncertain");
    }
    #[test]
    fn extreme_activity_cannot_overflow() {
        let mut e = entry();
        e.session.last_observed = i64::MIN;
        assert_eq!(lease_state(&e, 100), "ownership_uncertain");
    }
    #[test]
    fn future_end_does_not_release_current_work() {
        let mut e = entry();
        e.session.ended_at = Some(201);
        assert_eq!(lease_state(&e, 100), "ownership_uncertain");
    }
}
