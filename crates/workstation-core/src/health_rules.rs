//! Fixed diagnostic vocabulary; rule matching never confers mutation authority.
use crate::{
    control::{self, Check},
    Coverage,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Signal {
    pub id: String,
    pub value: bool,
    pub source: String,
    pub observed_at: i64,
    pub coverage: Coverage,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fingerprint {
    pub id: String,
    pub product: String,
    pub platforms: Vec<String>,
    pub applicable_versions: Vec<String>,
    pub requires: Vec<String>,
    pub contradicts: Vec<String>,
    pub freshness_seconds: i64,
    pub severity: String,
    pub classification: String,
    pub sources: Vec<String>,
    pub repair_recipe: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Match {
    pub id: String,
    pub status: String,
    pub classification: String,
    pub missing: Vec<String>,
    pub contradictions: Vec<String>,
    pub repair_authorized: bool,
    pub root_cause_fixed: bool,
}
pub fn evaluate(
    rules: &[Fingerprint],
    product: &str,
    platform: &str,
    version: &str,
    signals: &[Signal],
    now: i64,
) -> Check<Vec<Match>> {
    control::timestamp(now)?;
    if rules.len() > 100 || signals.len() > 256 {
        return Err("FINGERPRINT_BUDGET");
    }
    let mut index = BTreeMap::new();
    for s in signals {
        control::id(&s.id)?;
        control::text(&s.source, 200)?;
        control::timestamp(s.observed_at)?;
        if index.insert(&s.id, s).is_some() {
            return Err("CONFLICTING_SIGNAL_IDENTITY");
        }
    }
    let mut ids = BTreeSet::new();
    let mut out = vec![];
    for r in rules {
        control::id(&r.id)?;
        if !ids.insert(&r.id)
            || r.requires.is_empty()
            || r.requires.len() > 32
            || r.contradicts.len() > 32
            || !(1..=3600).contains(&r.freshness_seconds)
        {
            return Err("INVALID_FINGERPRINT");
        }
        control::text(&r.product, 100)?;
        control::text(&r.classification, 256)?;
        if r.platforms.len() > 8 || r.applicable_versions.len() > 100 || r.sources.len() > 16 {
            return Err("FINGERPRINT_METADATA_BUDGET");
        }
        for value in r.platforms.iter().chain(&r.applicable_versions) {
            control::text(value, 128)?;
        }
        for value in r.requires.iter().chain(&r.contradicts) {
            control::id(value)?;
        }
        if r.product != product && r.product != "any" {
            continue;
        }
        if !r.platforms.iter().any(|p| p == platform) {
            continue;
        }
        // Empty applicability is a generic symptom rule, never proof of a specific vendor defect.
        let applicable =
            r.applicable_versions.is_empty() || r.applicable_versions.iter().any(|v| v == version);
        let trusted = |id: &String| {
            index.get(id).filter(|s| {
                s.coverage == Coverage::Complete
                    && s.observed_at <= now
                    && now - s.observed_at <= r.freshness_seconds
            })
        };
        let mut missing = r
            .requires
            .iter()
            .filter(|id| !trusted(id).is_some_and(|s| s.value))
            .cloned()
            .collect::<Vec<_>>();
        for id in &r.contradicts {
            if trusted(id).is_none() {
                missing.push(format!("contradiction_unknown:{id}"));
            }
        }
        let contradictions = r
            .contradicts
            .iter()
            .filter(|id| trusted(id).is_some_and(|s| s.value))
            .cloned()
            .collect::<Vec<_>>();
        let status = if !applicable {
            "version_not_supported"
        } else if !contradictions.is_empty() {
            "contradicted"
        } else if !missing.is_empty() {
            "insufficient_evidence"
        } else {
            "matched_symptom"
        };
        out.push(Match {
            id: r.id.clone(),
            status: status.into(),
            classification: r.classification.clone(),
            missing,
            contradictions,
            repair_authorized: false,
            root_cause_fixed: false,
        });
    }
    Ok(out)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn rule() -> Fingerprint {
        Fingerprint {
            id: "state_growth".into(),
            product: "any".into(),
            platforms: vec!["windows".into()],
            applicable_versions: vec![],
            requires: vec!["growth".into()],
            contradicts: vec!["migration".into()],
            freshness_seconds: 60,
            severity: "warning".into(),
            classification: "resource_symptom_not_root_cause".into(),
            sources: vec![],
            repair_recipe: None,
        }
    }
    #[test]
    fn absent_signal_not_negative_proof() {
        let x = evaluate(&[rule()], "codex", "windows", "v1", &[], 10).unwrap();
        assert_eq!(x[0].status, "insufficient_evidence");
    }
    #[test]
    fn contradictory_signal_blocks_match() {
        let s = |id: &str| Signal {
            id: id.into(),
            value: true,
            source: "observation".into(),
            observed_at: 10,
            coverage: Coverage::Complete,
        };
        let x = evaluate(
            &[rule()],
            "codex",
            "windows",
            "v1",
            &[s("growth"), s("migration")],
            10,
        )
        .unwrap();
        assert_eq!(x[0].status, "contradicted");
    }
    #[test]
    fn future_signal_not_current_evidence() {
        let s = Signal {
            id: "growth".into(),
            value: true,
            source: "observation".into(),
            observed_at: 11,
            coverage: Coverage::Complete,
        };
        assert_eq!(
            evaluate(&[rule()], "codex", "windows", "v1", &[s], 10).unwrap()[0].status,
            "insufficient_evidence"
        );
    }
}

#[cfg(test)]
mod audit_signal_tests {
    use super::*;
    #[test]
    fn extreme_signal_time_rejected_before_subtraction() {
        let signal = Signal {
            id: "growth".into(),
            value: true,
            source: "reported".into(),
            observed_at: i64::MIN,
            coverage: Coverage::Complete,
        };
        assert!(evaluate(&[], "codex", "windows", "v1", &[signal], 10).is_err());
    }
}
