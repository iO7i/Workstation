//! Typed, evidence-gated symptom classification. Explicit imports are reports, not host proof.
//! No recipe, process termination or cleanup permission follows from a match.
use crate::{
    control::{self, Check, EvidenceKind},
    Coverage,
};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiagnosticEvidence {
    pub product: String,
    pub reported_version: String,
    pub platform: String,
    pub observed_at: i64,
    pub coverage: Coverage,
    pub facts: Facts,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Facts {
    pub matched_processes: Option<u32>,
    pub parent_exit_observed: Option<bool>,
    pub live_session_dependency: Option<bool>,
    pub intentionally_shared_daemon: Option<bool>,
    pub repeated_runtime_staging: Option<u32>,
    pub bootstrap_running: Option<bool>,
    pub local_docker_endpoint: Option<bool>,
    pub current_start_error_matches_socket: Option<bool>,
    pub current_registry_parse_failed: Option<bool>,
    pub registry_layout_verified: Option<bool>,
    pub application_stopped: Option<bool>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FingerprintResult {
    pub id: String,
    pub product: String,
    pub platform: String,
    pub reported_version: String,
    pub tested_vendor_versions: Vec<String>,
    pub source_kind: EvidenceKind,
    pub coverage: Coverage,
    pub classification: String,
    pub evidence_conditions: Vec<String>,
    pub contradictory_evidence: Vec<String>,
    pub root_cause_status: String,
    pub repair_status: String,
    pub protected: bool,
    pub sources: Vec<String>,
}
fn row(
    e: &DiagnosticEvidence,
    id: &str,
    required: &[&str],
    contradictions: &[&str],
    sources: &[&str],
) -> FingerprintResult {
    FingerprintResult {
        id: id.into(),
        product: e.product.clone(),
        platform: e.platform.clone(),
        reported_version: e.reported_version.clone(),
        tested_vendor_versions: vec![],
        source_kind: EvidenceKind::AgentReported,
        coverage: Coverage::Partial,
        classification: "reported_symptom_candidate_not_verified_vendor_bug".into(),
        evidence_conditions: required.iter().map(|x| (*x).into()).collect(),
        contradictory_evidence: contradictions.iter().map(|x| (*x).into()).collect(),
        root_cause_status: "unconfirmed".into(),
        repair_status: "blocked_native_recipe_certification_required".into(),
        protected: true,
        sources: sources.iter().map(|x| (*x).into()).collect(),
    }
}
pub fn classify(e: &DiagnosticEvidence, now: i64) -> Check<Vec<FingerprintResult>> {
    control::id(&e.product)?;
    control::text(&e.reported_version, 64)?;
    control::timestamp(e.observed_at)?;
    control::timestamp(now)?;
    if !["windows", "linux", "macos"].contains(&e.platform.as_str()) {
        return Err("DIAGNOSTIC_PLATFORM_INVALID");
    }
    if e.observed_at > now || now - e.observed_at > 300 {
        return Err("DIAGNOSTIC_EVIDENCE_STALE");
    }
    if e.facts.matched_processes.is_some_and(|n| n > 100_000)
        || e.facts
            .repeated_runtime_staging
            .is_some_and(|n| n > 100_000)
    {
        return Err("DIAGNOSTIC_COUNT_LIMIT");
    }
    if e.coverage != Coverage::Complete {
        return Ok(vec![]);
    }
    let f = &e.facts;
    let mut results = vec![];
    if f.parent_exit_observed == Some(true)
        && f.matched_processes.is_some_and(|n| n > 0)
        && f.live_session_dependency == Some(false)
        && f.intentionally_shared_daemon == Some(false)
    {
        results.push(row(
            e,
            "lifecycle.reported_retained_children",
            &[
                "parent exit reported",
                "matching descendants reported",
                "no known live session dependency reported",
                "not an intentional shared daemon reported",
            ],
            &[
                "live session dependency",
                "intentional shared daemon",
                "unknown ownership",
            ],
            &["https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects"],
        ));
    }
    if f.repeated_runtime_staging.is_some_and(|n| n >= 3) && f.bootstrap_running == Some(false) {
        results.push(row(
            e,
            "runtime.reported_repeated_staging",
            &[
                "three or more staging directories reported",
                "bootstrap inactive reported",
            ],
            &["bootstrap is running", "unknown directory ownership"],
            &[],
        ));
    }
    if e.product == "docker"
        && e.platform == "windows"
        && f.local_docker_endpoint == Some(true)
        && f.current_start_error_matches_socket == Some(true)
        && f.application_stopped == Some(true)
    {
        results.push(row(
            e,
            "docker.reported_stale_socket_candidate",
            &[
                "local endpoint reported",
                "current startup socket error reported",
                "Docker stopped reported",
            ],
            &[
                "remote/unknown endpoint",
                "stale error file",
                "live application",
            ],
            &["https://docs.docker.com/desktop/troubleshoot-and-support/troubleshoot/"],
        ));
    }
    if e.product == "codex"
        && f.current_registry_parse_failed == Some(true)
        && f.registry_layout_verified == Some(true)
        && f.application_stopped == Some(true)
    {
        results.push(row(
            e,
            "codex.reported_registry_parse_failure",
            &[
                "layout checked reported",
                "current registry parse error reported",
                "application stopped reported",
            ],
            &[
                "unknown registry layout",
                "active state writer",
                "attachment payload dependency",
            ],
            &["https://github.com/openai/codex"],
        ));
    }
    Ok(results)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn e() -> DiagnosticEvidence {
        DiagnosticEvidence {
            product: "codex".into(),
            reported_version: "synthetic-only".into(),
            platform: "windows".into(),
            observed_at: 100,
            coverage: Coverage::Complete,
            facts: Facts {
                matched_processes: Some(3),
                parent_exit_observed: Some(true),
                live_session_dependency: Some(false),
                intentionally_shared_daemon: Some(false),
                ..Facts::default()
            },
        }
    }
    #[test]
    fn reported_match_never_observed_authority() {
        let r = classify(&e(), 100).unwrap();
        assert_eq!(r.len(), 1);
        assert!(r[0].protected);
        assert_eq!(r[0].source_kind, EvidenceKind::AgentReported);
        assert!(r[0].tested_vendor_versions.is_empty());
    }
    #[test]
    fn legitimate_daemon_suppresses_match() {
        let mut x = e();
        x.facts.intentionally_shared_daemon = Some(true);
        assert!(classify(&x, 100).unwrap().is_empty());
    }
    #[test]
    fn unknown_live_dependency_suppresses_match() {
        let mut x = e();
        x.facts.live_session_dependency = None;
        assert!(classify(&x, 100).unwrap().is_empty());
    }
    #[test]
    fn partial_coverage_does_not_promote_absence() {
        let mut x = e();
        x.coverage = Coverage::Partial;
        assert!(classify(&x, 100).unwrap().is_empty());
    }
    #[test]
    fn stale_error_is_rejected() {
        assert!(classify(&e(), 500).is_err());
    }
    #[test]
    fn active_bootstrap_not_abandoned() {
        let mut x = e();
        x.facts = Facts {
            repeated_runtime_staging: Some(9),
            bootstrap_running: Some(true),
            ..Facts::default()
        };
        assert!(classify(&x, 100).unwrap().is_empty());
    }
    #[test]
    fn remote_docker_not_repair_target() {
        let mut x = e();
        x.product = "docker".into();
        x.facts = Facts {
            local_docker_endpoint: Some(false),
            current_start_error_matches_socket: Some(true),
            application_stopped: Some(true),
            ..Facts::default()
        };
        assert!(classify(&x, 100).unwrap().is_empty());
    }
    #[test]
    fn registry_parse_failure_alone_not_enough() {
        let mut x = e();
        x.facts = Facts {
            current_registry_parse_failed: Some(true),
            ..Facts::default()
        };
        assert!(classify(&x, 100).unwrap().is_empty());
    }
}
