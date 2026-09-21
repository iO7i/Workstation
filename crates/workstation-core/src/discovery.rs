//! Local, bounded, reference-only resource discovery. No installation interface.
use crate::control::{self, Check, Focus};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capability {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub source: String,
    pub needs: Vec<String>,
    pub platforms: Vec<String>,
    pub cost: String,
    pub privacy: String,
    pub equivalence: Option<String>,
    pub purpose: String,
    pub limitations: Vec<String>,
    pub review_status: String,
    pub review_revision: String,
    pub reviewed_at: i64,
    pub fresh_for_seconds: u32,
    pub license_scope: String,
    pub adoption_mode: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Feedback {
    pub id: String,
    pub project_id: String,
    pub resource_id: String,
    pub need: String,
    pub disposition: String,
    pub until: Option<i64>,
    pub outcome: String,
    pub evidence_ref: Option<String>,
    pub revision: Option<String>,
    pub recorded_at: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiscoveryCard {
    pub resource_id: String,
    pub title: String,
    pub source: String,
    pub kind: String,
    pub why_now: String,
    pub potential_benefit: String,
    pub limitations: Vec<String>,
    pub already_available: Option<bool>,
    pub compatibility: String,
    pub freshness: String,
    pub benefit_status: String,
    pub next_step: String,
    pub changes_authorized: bool,
}
impl Capability {
    pub fn validate(&self) -> Check<()> {
        control::id(&self.id)?;
        control::text(&self.title, 128)?;
        control::safe_locator(&self.source, false)?;
        control::text(&self.purpose, 600)?;
        if ![
            "existing_capability",
            "tool",
            "integration",
            "mcp_server",
            "skill",
            "workflow",
            "documentation",
            "reference_implementation",
            "dataset",
            "benchmark",
        ]
        .contains(&self.kind.as_str())
        {
            return Err("RESOURCE_KIND_INVALID");
        }
        control::text(&self.review_revision, 128)?;
        control::text(&self.license_scope, 400)?;
        if self.title.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.license_scope.trim().is_empty()
            || self.review_revision.trim().is_empty()
        {
            return Err("CATALOG_MEANINGFUL_METADATA_REQUIRED");
        }
        if let Some(e) = &self.equivalence {
            control::id(e)?;
        }
        if self.platforms.len() > 4
            || self
                .platforms
                .iter()
                .any(|p| !["windows", "linux", "macos", "platform_neutral"].contains(&p.as_str()))
        {
            return Err("PLATFORM_METADATA_INVALID");
        }
        control::lines(&self.needs)?;
        control::lines(&self.limitations)?;
        control::timestamp(self.reviewed_at)?;
        if self.adoption_mode != "reference_only" {
            return Err("NO_EXECUTABLE_CATALOG_ACTION");
        }
        if !["reviewed", "unverified", "rejected"].contains(&self.review_status.as_str())
            || !["free", "paid", "unknown", "not_applicable"].contains(&self.cost.as_str())
            || !["local", "external", "unknown"].contains(&self.privacy.as_str())
        {
            return Err("CATALOG_STATUS_INVALID");
        }
        if self.needs.is_empty()
            || self.limitations.is_empty()
            || self.fresh_for_seconds == 0
            || self.fresh_for_seconds > 31_536_000
        {
            return Err("CATALOG_METADATA_REQUIRED");
        }
        Ok(())
    }
}
pub fn validate_feedback(f: &Feedback) -> Check<()> {
    for s in [&f.id, &f.project_id, &f.resource_id, &f.need] {
        control::id(s)?;
    }
    control::timestamp(f.recorded_at)?;
    if let Some(t) = f.until {
        control::timestamp(t)?;
    }
    for value in [&f.evidence_ref, &f.revision].into_iter().flatten() {
        control::text(value, 512)?;
        if value.trim().is_empty() {
            return Err("TRIAL_EVIDENCE_REQUIRED");
        }
    }
    if !["saved", "dismissed", "snoozed", "evaluated", "adopted"].contains(&f.disposition.as_str())
    {
        return Err("FEEDBACK_STATE");
    }
    if f.disposition == "snoozed" && !f.until.is_some_and(|t| t > f.recorded_at) {
        return Err("SNOOZE_TIME_REQUIRED");
    }
    if f.outcome != "not_recorded"
        && (f.disposition != "evaluated" || f.evidence_ref.is_none() || f.revision.is_none())
    {
        return Err("TRIAL_EVIDENCE_REQUIRED");
    }
    if !["not_recorded", "useful", "not_useful", "inconclusive"].contains(&f.outcome.as_str()) {
        return Err("FEEDBACK_OUTCOME");
    }
    Ok(())
}
fn canonical_source(source: &str) -> String {
    match source.split_once("://") {
        Some((scheme, rest)) => {
            let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
            format!(
                "{}://{}/{}",
                scheme.to_ascii_lowercase(),
                host.to_ascii_lowercase(),
                path.trim_end_matches('/')
            )
        }
        None => source.into(),
    }
}
pub fn recommend(
    project: &str,
    focus: &Focus,
    catalog: &[Capability],
    feedback: &[Feedback],
    now: i64,
) -> Check<Vec<DiscoveryCard>> {
    control::id(project)?;
    control::timestamp(now)?;
    control::lines(&focus.needs)?;
    control::text(&focus.evidence_ref, 256)?;
    if catalog.len() > 100 || feedback.len() > 1000 {
        return Err("DISCOVERY_LIMIT");
    }
    if focus.evidence_ref.is_empty() || focus.needs.is_empty() {
        return Ok(vec![]);
    }
    let mut unique = std::collections::BTreeSet::new();
    for c in catalog {
        c.validate()?;
        if !unique.insert(&c.id) {
            return Err("CATALOG_DUPLICATE");
        }
    }
    for f in feedback {
        validate_feedback(f)?;
    }
    let mut candidates = vec![];
    for c in catalog {
        if c.review_status != "reviewed"
            || c.reviewed_at > now
            || focus.blocked.contains(&c.id)
            || catalog.iter().any(|a| {
                focus.blocked.contains(&a.id)
                    && canonical_source(&a.source) == canonical_source(&c.source)
            })
            || (focus.free_only && c.cost == "paid")
            || (focus.local_only && c.privacy == "external")
            || (!c.platforms.is_empty()
                && !c.platforms.contains(&focus.platform)
                && !c.platforms.iter().any(|p| p == "platform_neutral"))
        {
            continue;
        }
        let mut needs: Vec<_> = c.needs.iter().filter(|n| focus.needs.contains(n)).collect();
        needs.sort();
        let canonical = canonical_source(&c.source);
        needs.retain(|need| {
            let f = feedback
                .iter()
                .filter(|f| {
                    f.project_id == project
                        && f.need.as_str() == need.as_str()
                        && f.recorded_at <= now
                        && catalog.iter().any(|alias| {
                            alias.id == f.resource_id
                                && canonical_source(&alias.source) == canonical
                        })
                })
                .max_by_key(|f| (f.recorded_at, &f.id));
            !f.is_some_and(|f| {
                f.disposition == "dismissed"
                    || (f.disposition == "snoozed" && f.until.is_some_and(|t| t > now))
            })
        });
        let Some(need) = needs.first() else {
            continue;
        };
        let stale = now - c.reviewed_at > i64::from(c.fresh_for_seconds);
        let unknown = (focus.free_only && c.cost == "unknown")
            || (focus.local_only && c.privacy == "unknown")
            || c.platforms.is_empty()
            || stale;
        let existing = focus.existing.contains(&c.id);
        let f = feedback
            .iter()
            .filter(|f| {
                f.project_id == project
                    && f.resource_id == c.id
                    && f.need.as_str() == need.as_str()
                    && f.recorded_at <= now
            })
            .max_by_key(|f| (f.recorded_at, &f.id));
        let benefit = match f.filter(|f| {
            f.disposition == "evaluated"
                && f.revision.as_deref() == Some(c.review_revision.as_str())
        }) {
            Some(f) if f.outcome == "useful" => "observed_useful_here",
            Some(f) if f.outcome == "not_useful" => "evaluated_not_useful_here",
            Some(_) => "inconclusive_here",
            None => "potential_only",
        };
        let card = DiscoveryCard {
            resource_id: c.id.clone(),
            title: c.title.clone(),
            source: c.source.clone(),
            kind: c.kind.clone(),
            why_now: format!(
                "Matches approved focus need {need}; evidence {}",
                focus.evidence_ref
            ),
            potential_benefit: c.purpose.clone(),
            limitations: c.limitations.clone(),
            already_available: existing.then_some(true),
            compatibility: if unknown {
                "needs_review"
            } else {
                "purpose_reviewed_runtime_not_certified"
            }
            .into(),
            freshness: if stale {
                "stale"
            } else {
                "current_source_review"
            }
            .into(),
            benefit_status: benefit.into(),
            next_step: if existing && !unknown {
                "use_existing_reference"
            } else {
                "read_and_evaluate_without_activation"
            }
            .into(),
            changes_authorized: false,
        };
        candidates.push((
            (unknown, !existing, stale, c.id.clone()),
            canonical.to_owned(),
            c.equivalence.clone(),
            card,
        ));
    }
    candidates.sort_by(|a, b| a.0.cmp(&b.0));
    let mut seen = std::collections::BTreeSet::new();
    let mut equivalents = std::collections::BTreeSet::new();
    let mut result = vec![];
    for (_, source, equivalent, card) in candidates {
        if seen.contains(&source) || equivalent.as_ref().is_some_and(|e| equivalents.contains(e)) {
            continue;
        }
        seen.insert(source);
        if let Some(e) = equivalent {
            equivalents.insert(e);
        }
        result.push(card);
        if result.len() == 3 {
            break;
        }
    }
    Ok(result)
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateInput {
    pub title: String,
    pub kind: String,
    pub source: String,
    pub needs: Vec<String>,
    pub purpose: String,
    pub limitations: Vec<String>,
}
impl CandidateInput {
    pub fn into_unverified(self, id: String, now: i64) -> Check<Capability> {
        let c = Capability {
            id,
            title: self.title,
            kind: self.kind,
            source: self.source,
            needs: self.needs,
            platforms: vec![],
            cost: "unknown".into(),
            privacy: "unknown".into(),
            equivalence: None,
            purpose: self.purpose,
            limitations: self.limitations,
            review_status: "unverified".into(),
            review_revision: "unreviewed".into(),
            reviewed_at: now,
            fresh_for_seconds: 30 * 86400,
            license_scope: "Unknown; must be reviewed per resource".into(),
            adoption_mode: "reference_only".into(),
        };
        c.validate()?;
        Ok(c)
    }
}
pub const PUBLIC_NEEDS: &[&str] = &[
    "accessibility_review",
    "context_runway",
    "continuity",
    "credential_hygiene",
    "credential_location",
    "current_documentation",
    "decision_context",
    "integration_debugging",
    "interface_review",
    "model_selection",
    "parallel_work",
    "quota_visibility",
    "reproducible_evaluation",
    "runtime_recovery",
    "session_navigation",
    "skill_discovery",
    "storage_attribution",
];
pub fn research_brief(focus: &Focus) -> serde_json::Value {
    serde_json::json!({"schema_version":1,"purpose":"find_relevant_resources","needs":focus.needs.iter().filter(|n|PUBLIC_NEEDS.contains(&n.as_str())).collect::<Vec<_>>(),"platform":if ["windows","linux","macos"].contains(&focus.platform.as_str()){focus.platform.as_str()}else{"unknown"},
 "constraints":{"free_only":focus.free_only,"local_only":focus.local_only},"custom_needs_omitted":focus.needs.iter().any(|n|!PUBLIC_NEEDS.contains(&n.as_str())),"max_candidates":5,"disclosure_status":"review_required_no_export_or_agent_launch_performed"})
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn candidate_cannot_supply_approval() {
        assert!(serde_json::from_value::<CandidateInput>(serde_json::json!({"title":"x","kind":"tool","source":"https://example.org","needs":["test"],"purpose":"x","limitations":["x"],"review_status":"reviewed"})).is_err());
    }
    #[test]
    fn custom_private_need_excluded_from_export() {
        let f = Focus {
            needs: vec![
                "private-project-contract-name".into(),
                "interface_review".into(),
            ],
            free_only: true,
            local_only: true,
            platform: "windows".into(),
            existing: vec![],
            blocked: vec![],
            evidence_ref: "private".into(),
        };
        let s = research_brief(&f).to_string();
        assert!(!s.contains("contract-name"));
        assert!(s.contains("interface_review"));
    }
    #[test]
    fn plaintext_token_not_catalog() {
        let c = CandidateInput {
            title: "test".into(),
            kind: "doc".into(),
            source: "https://token@example.org".into(),
            needs: vec!["a".into()],
            purpose: "a".into(),
            limitations: vec!["b".into()],
        };
        assert!(c.into_unverified("id".into(), 1).is_err());
    }
    #[test]
    fn handoff_brief_omits_evidence_and_private_inventory() {
        let f = Focus {
            needs: vec!["tests".into()],
            free_only: true,
            local_only: true,
            platform: "windows".into(),
            existing: vec!["private-project".into()],
            blocked: vec![],
            evidence_ref: "private-evidence".into(),
        };
        let s = research_brief(&f).to_string();
        assert!(!s.contains("private"));
    }
}

#[cfg(test)]
mod audit_feedback_tests {
    use super::*;
    #[test]
    fn empty_trial_reference_is_not_evidence() {
        let f = Feedback {
            id: "f".into(),
            project_id: "p".into(),
            resource_id: "r".into(),
            need: "n".into(),
            disposition: "evaluated".into(),
            until: None,
            outcome: "useful".into(),
            evidence_ref: Some("".into()),
            revision: Some("v1".into()),
            recorded_at: 1,
        };
        assert!(validate_feedback(&f).is_err());
    }
}
