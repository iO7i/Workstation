//! Executes only under cargo test. Python evidence does not claim these ran.
use serde::Deserialize;
use serde_json::Value;
use workstation_core::{continuity, control::*, discovery, economics, plans};
#[derive(Deserialize)]
struct Vector {
    name: String,
    samples: Vec<economics::QuotaSample>,
    now: i64,
    expected: Value,
}
#[test]
fn economics_shared_vectors() {
    let cases: Vec<Vector> = serde_json::from_str(include_str!(
        "../../../fixtures/control/economics-vectors.json"
    ))
    .unwrap();
    for c in cases {
        let r = economics::forecast(&c.samples, c.now);
        if let Some(error) = c.expected.get("error").and_then(Value::as_str) {
            assert_eq!(r.err().unwrap(), error, "{}", c.name);
            continue;
        }
        let r = r.unwrap();
        if c.expected.get("remaining").is_some() {
            let expected = c.expected["remaining"].as_f64();
            assert_eq!(r.remaining, expected, "{}", c.name);
        }
        if let Some(n) = c.expected.get("horizons").and_then(Value::as_u64) {
            assert_eq!(r.horizons.len() as u64, n, "{}", c.name);
        }
        if let Some(x) = c.expected.get("blended_seconds").and_then(Value::as_f64) {
            let h = r.horizons.iter().find(|h| h.label == "blended").unwrap();
            assert!(
                (h.exhaustion_after_seconds.unwrap() - x).abs() < 1e-7,
                "{}",
                c.name
            );
        }
        if c.expected["all_eta_null"] == true {
            assert!(
                r.horizons
                    .iter()
                    .all(|h| h.exhaustion_after_seconds.is_none()),
                "{}",
                c.name
            );
        }
        if let Some(x) = c.expected.get("confidence").and_then(Value::as_str) {
            assert_eq!(r.confidence, x, "{}", c.name);
        }
    }
}
#[test]
fn vendor_wire_fixtures_stay_import_evidence() {
    for (format, wire) in [
        (
            "codex-rate-limits-2026-09",
            include_str!("../../../fixtures/control/codex-rate-limits.json"),
        ),
        (
            "claude-statusline-2026-09",
            include_str!("../../../fixtures/control/claude-statusline.json"),
        ),
    ] {
        let payload: Value = serde_json::from_str(wire).unwrap();
        let rows = economics::normalize_vendor(format, &payload, "synthetic", "demo", 100).unwrap();
        assert!(rows
            .iter()
            .all(|s| s.evidence.kind == EvidenceKind::AgentReported));
    }
}
#[test]
fn licensed_synthetic_frontier() {
    let m: Vec<economics::ModelMetric> =
        serde_json::from_str(include_str!("../../../fixtures/control/models.json")).unwrap();
    let v = economics::pareto(&m).unwrap();
    assert!(!v["frontier"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v == "dominated"));
}
#[test]
fn plan_profile_never_switches() {
    for mode in [
        economics::PreferenceProfile::MaximumIntelligence,
        economics::PreferenceProfile::Balanced,
        economics::PreferenceProfile::Stretch,
        economics::PreferenceProfile::Economy,
    ] {
        assert_eq!(mode.advisory()["automatic_switching"], false);
    }
}
#[test]
fn synthetic_cycle_fit() {
    let c: Vec<economics::Cycle> =
        serde_json::from_str(include_str!("../../../fixtures/control/plan-cycles.json")).unwrap();
    assert_eq!(economics::plan_fit(&c).unwrap()["status"], "underutilized");
}
#[test]
fn catalog_entries_are_reference_only() {
    let c: Vec<discovery::Capability> =
        serde_json::from_str(include_str!("../../../catalog/resources.json")).unwrap();
    assert!((20..=30).contains(&c.len()));
    for r in c {
        r.validate().unwrap();
        assert_eq!(r.adoption_mode, "reference_only");
    }
}
fn stamp() -> WorkspaceStamp {
    WorkspaceStamp {
        project_id: "p".into(),
        workspace_id: "ws".into(),
        path: "D:/fixture".into(),
        directory_identity: "file-id".into(),
        head: "a".repeat(40),
        branch: Some("refs/heads/main".into()),
        status_digest: "b".repeat(64),
        dirty: Some(true),
        untracked: Some(1),
        ignored: Some(1),
        local_only_commits: Some(1),
        blockers: vec![],
        observed_at: 100,
        coverage: workstation_core::Coverage::Complete,
    }
}
fn checkpoint() -> Checkpoint {
    Checkpoint {
        id: "cp".into(),
        work: WorkItem {
            id: "w".into(),
            project_id: "p".into(),
            objective: "finish test".into(),
            state: WorkState::Paused,
            priority: 1,
            version: 1,
            workspace_id: Some("ws".into()),
            completed: vec![],
            remaining: vec!["fix assertion".into()],
            blockers: vec![],
            evidence: Evidence {
                kind: EvidenceKind::AgentReported,
                source: "fixture".into(),
                at: 100,
                coverage: workstation_core::Coverage::Partial,
            },
        },
        session_id: None,
        workspace: stamp(),
        completed: vec![],
        remaining: vec!["fix assertion".into()],
        blockers: vec![],
        tests: vec![],
        decision_ids: vec![],
        resource_ids: vec![],
        recorded_at: 100,
        source: Evidence {
            kind: EvidenceKind::AgentReported,
            source: "fixture".into(),
            at: 100,
            coverage: workstation_core::Coverage::Partial,
        },
    }
}
#[test]
fn dirty_checkpoint_can_make_packet_without_cleanup() {
    let cp = checkpoint();
    let p = continuity::handoff(&cp, &stamp(), "grok", &[], 100).unwrap();
    assert_eq!(p.mode, "packet_only");
    assert_eq!(p.workspace.dirty, Some(true));
}
#[test]
fn changed_head_blocks_handoff() {
    let cp = checkpoint();
    let mut s = stamp();
    s.head = "c".repeat(40);
    assert_eq!(
        continuity::revalidate_checkpoint(&cp, &s, 100),
        Err("HANDOFF_STALE")
    );
}
#[test]
fn changed_directory_blocks_handoff() {
    let cp = checkpoint();
    let mut s = stamp();
    s.directory_identity = "replacement".into();
    assert!(continuity::revalidate_checkpoint(&cp, &s, 100).is_err());
}
#[test]
fn changed_dirty_evidence_blocks_handoff() {
    let cp = checkpoint();
    let mut s = stamp();
    s.status_digest = "c".repeat(64);
    assert!(continuity::revalidate_checkpoint(&cp, &s, 100).is_err());
}
#[test]
fn old_stamp_blocks_handoff() {
    assert!(continuity::revalidate_checkpoint(&checkpoint(), &stamp(), 1000).is_err());
}
#[test]
fn hidden_secret_cannot_enter_summary() {
    let mut cp = checkpoint();
    cp.remaining = vec!["SECRET_CANARY".into()];
    assert!(continuity::handoff(&cp, &stamp(), "grok", &[], 100).is_err());
}
fn plan() -> plans::RepairPlan {
    plans::RepairPlan {
        id: "p".into(),
        project_id: "project".into(),
        policy_version: "v1".into(),
        action: plans::TypedAction::DockerRuntimeQuarantine,
        created_at: 100,
        expires_at: 200,
        target: plans::TargetIdentity {
            object_id: "socket".into(),
            identity_digest: "x".repeat(64),
            coverage: workstation_core::Coverage::Complete,
            active_dependencies: false,
        },
        evidence_refs: vec!["observation".into()],
        backup_required: true,
    }
}
#[test]
fn expired_plan_blocked() {
    let p = plan();
    let a = plans::Approval {
        plan_id: "p".into(),
        digest: "digest".into(),
        approved_at: 100,
    };
    assert!(p.revalidate(&a, "digest", &p.target, 200, "v1").is_err());
}
#[test]
fn changed_target_invalidates_plan() {
    let p = plan();
    let a = plans::Approval {
        plan_id: "p".into(),
        digest: "digest".into(),
        approved_at: 100,
    };
    let mut t = p.target.clone();
    t.identity_digest = "changed".into();
    assert!(p.revalidate(&a, "digest", &t, 101, "v1").is_err());
}
#[test]
fn active_dependencies_stay_protected() {
    let mut p = plan();
    p.target.active_dependencies = true;
    let a = plans::Approval {
        plan_id: "p".into(),
        digest: "digest".into(),
        approved_at: 100,
    };
    assert_eq!(
        p.revalidate(&a, "digest", &p.target, 101, "v1"),
        Err("PROTECTED_TARGET")
    );
}
#[test]
fn valid_plan_still_not_certified() {
    assert_eq!(
        plan().execution_status(),
        "blocked_pending_native_recipe_certification"
    );
}
