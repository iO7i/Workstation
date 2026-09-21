//! Authoring-time regression specifications. NOT executed in the implementation-only delivery.
use serde_json::json;
use std::collections::BTreeMap;
use workstation_core::{
    control::*, health_rules::*, host_graph::*, integrations::*, operations::*, ownership::*,
    telemetry, wire, workspace_policy::*, Coverage,
};
fn detail() -> WorkspaceDetail {
    WorkspaceDetail {
        stamp: WorkspaceStamp {
            project_id: "p".into(),
            workspace_id: "w".into(),
            path: "D:/work/w".into(),
            directory_identity: "vol:1".into(),
            head: "a".repeat(40),
            branch: Some("refs/heads/feature".into()),
            status_digest: "b".repeat(64),
            dirty: Some(false),
            untracked: Some(0),
            ignored: Some(0),
            local_only_commits: Some(2),
            blockers: vec![],
            observed_at: 100,
            coverage: Coverage::Complete,
        },
        repository_identity: "common:1".into(),
        main_worktree: false,
        bare: false,
        locked: false,
        changed_paths: vec![],
        ignored_paths: vec![],
        path_coverage: Coverage::Complete,
        branch_exists: Some(true),
        checkout_filters_present: Some(false),
    }
}
fn ctx() -> CleanupContext {
    CleanupContext {
        detail: detail(),
        work_states: BTreeMap::from([("work".into(), WorkState::Done)]),
        unresolved_sessions: vec![],
        protection_reasons: vec![],
        observations_complete: true,
        quiescence_acknowledged: true,
        preserve_head_ref: true,
    }
}
fn protected(c: &CleanupContext) -> bool {
    cleanup_eligibility(c, 110).unwrap().disposition == "protected"
}
fn actor(id: &str, workspace: &str) -> WorkspaceActor {
    WorkspaceActor {
        session_id: id.into(),
        work_id: None,
        project_id: "p".into(),
        workspace_id: Some(workspace.into()),
        repository_identity: Some("repo".into()),
        branch: Some("refs/heads/main".into()),
        paths: vec!["src/auth.rs".into()],
        paths_complete: true,
        role: "primary".into(),
        ended: false,
        lease_until: Some(150),
        last_observed: 100,
    }
}
#[test]
fn cleanup_is_only_plan_eligibility() {
    let x = cleanup_eligibility(&ctx(), 110).unwrap();
    assert_eq!(x.disposition, "eligible_for_exact_reviewed_plan");
    assert!(!x.automatic_delete_authorized);
}
#[test]
fn ignored_file_blocks() {
    let mut c = ctx();
    c.detail.stamp.ignored = Some(1);
    assert!(protected(&c));
}
#[test]
fn ignored_unknown_blocks() {
    let mut c = ctx();
    c.detail.stamp.ignored = None;
    assert!(protected(&c));
}
#[test]
fn dirty_blocks() {
    let mut c = ctx();
    c.detail.stamp.dirty = Some(true);
    assert!(protected(&c));
}
#[test]
fn dirty_unknown_blocks() {
    let mut c = ctx();
    c.detail.stamp.dirty = None;
    assert!(protected(&c));
}
#[test]
fn untracked_blocks() {
    let mut c = ctx();
    c.detail.stamp.untracked = Some(1);
    assert!(protected(&c));
}
#[test]
fn main_checkout_blocks() {
    let mut c = ctx();
    c.detail.main_worktree = true;
    assert!(protected(&c));
}
#[test]
fn git_lock_blocks() {
    let mut c = ctx();
    c.detail.locked = true;
    assert!(protected(&c));
}
#[test]
fn unfinished_work_blocks() {
    let mut c = ctx();
    c.work_states.insert("x".into(), WorkState::Paused);
    assert!(protected(&c));
}
#[test]
fn uncertain_session_blocks() {
    let mut c = ctx();
    c.unresolved_sessions.push("possibly-live".into());
    assert!(protected(&c));
}
#[test]
fn manual_protection_blocks() {
    let mut c = ctx();
    c.protection_reasons.push("hold".into());
    assert!(protected(&c));
}
#[test]
fn no_external_quiescence_blocks() {
    let mut c = ctx();
    c.quiescence_acknowledged = false;
    assert!(protected(&c));
}
#[test]
fn no_preservation_blocks() {
    let mut c = ctx();
    c.preserve_head_ref = false;
    assert!(protected(&c));
}
#[test]
fn stale_evidence_blocks() {
    let mut c = ctx();
    c.detail.stamp.observed_at = 1;
    assert!(protected(&c));
}
#[test]
fn future_evidence_blocks() {
    let mut c = ctx();
    c.detail.stamp.observed_at = 120;
    assert!(protected(&c));
}
#[test]
fn partial_evidence_blocks() {
    let mut c = ctx();
    c.detail.path_coverage = Coverage::Partial;
    assert!(protected(&c));
}
#[test]
fn worktree_operation_blocker() {
    let mut c = ctx();
    c.detail.stamp.blockers.push("rebase".into());
    assert!(protected(&c));
}
#[test]
fn same_worktree_conflicts() {
    let c = conflicts(&[actor("a", "w"), actor("b", "w")], 110).unwrap();
    assert!(matches!(c[0].kind, ConflictKind::SameWorkspace));
}
#[test]
fn same_branch_conflicts_even_without_known_edits() {
    let mut a = actor("a", "wa");
    let mut b = actor("b", "wb");
    a.paths.clear();
    b.paths.clear();
    let c = conflicts(&[a, b], 110).unwrap();
    assert!(matches!(c[0].kind, ConflictKind::SameBranch));
}
#[test]
fn overlapping_paths_across_branches_conflict() {
    let a = actor("a", "wa");
    let mut b = actor("b", "wb");
    b.branch = Some("refs/heads/other".into());
    assert!(matches!(
        conflicts(&[a, b], 110).unwrap()[0].kind,
        ConflictKind::OverlappingPaths
    ));
}
#[test]
fn expired_lease_preserves_collision() {
    let c = conflicts(&[actor("a", "w"), actor("b", "w")], 500).unwrap();
    assert!(c[0].ownership_uncertain);
}
#[test]
fn unrelated_repositories_are_not_same_file() {
    let a = actor("a", "wa");
    let mut b = actor("b", "wb");
    b.repository_identity = Some("different".into());
    assert!(conflicts(&[a, b], 110).unwrap().is_empty());
}
#[test]
fn future_catalog_and_install_are_not_implicit_health() {
    assert!(capabilities(Adapter::Roo)
        .unsupported
        .contains(&"native_continuation".to_owned()));
}
#[test]
fn worktree_destination_parent_is_required() {
    let op = json!({"operation":"worktree_create","integration_id":null,"integration_digest":null,"destination":"D:/w","branch":"feature","base_commit":"a".repeat(40),"repository_identity":"repo","source_workspace_id":"w","git_sha256":"b".repeat(64)});
    assert!(serde_json::from_value::<Operation>(op).is_err());
}
#[test]
fn branch_option_injection_rejected() {
    for b in ["--force", "x..y", "x.lock", "x@{y}", "/x"] {
        assert!(branch_name(b).is_err());
    }
}
#[test]
fn archive_budget_rejected() {
    assert!(Operation::JournalArchive {
        before: 100,
        limit: 101,
        selection_digest: "a".repeat(64)
    }
    .validate()
    .is_err());
}
#[test]
fn secret_revoke_requires_exact_version() {
    let x = json!({"operation":"secret_revoke","resource_id":"s"});
    assert!(serde_json::from_value::<Operation>(x).is_err());
}
fn rule() -> Fingerprint {
    Fingerprint {
        id: "test".into(),
        product: "any".into(),
        platforms: vec!["windows".into()],
        applicable_versions: vec!["fixture-v1".into()],
        requires: vec!["growth".into()],
        contradicts: vec!["update".into()],
        freshness_seconds: 60,
        severity: "warning".into(),
        classification: "symptom_only".into(),
        sources: vec![],
        repair_recipe: None,
    }
}
fn signal(id: &str, value: bool) -> Signal {
    Signal {
        id: id.into(),
        value,
        source: "fixture".into(),
        observed_at: 100,
        coverage: Coverage::Complete,
    }
}
#[test]
fn negative_contradiction_requires_observation() {
    let m = evaluate(
        &[rule()],
        "x",
        "windows",
        "fixture-v1",
        &[signal("growth", true)],
        110,
    )
    .unwrap();
    assert_eq!(m[0].status, "insufficient_evidence");
}
#[test]
fn version_mismatch_stays_unmatched() {
    let m = evaluate(
        &[rule()],
        "x",
        "windows",
        "fixture-v2",
        &[signal("growth", true), signal("update", false)],
        110,
    )
    .unwrap();
    assert_eq!(m[0].status, "version_not_supported");
}
#[test]
fn matched_symptom_not_repair_authority() {
    let m = evaluate(
        &[rule()],
        "x",
        "windows",
        "fixture-v1",
        &[signal("growth", true), signal("update", false)],
        110,
    )
    .unwrap();
    assert_eq!(m[0].status, "matched_symptom");
    assert!(!m[0].repair_authorized);
    assert!(!m[0].root_cause_fixed);
}
#[test]
fn duplicate_signal_ids_rejected() {
    assert!(evaluate(
        &[rule()],
        "x",
        "windows",
        "fixture-v1",
        &[signal("growth", true), signal("growth", false)],
        110
    )
    .is_err());
}
#[test]
fn quota_buckets_not_inferred_from_gemini_tokens() {
    let x = telemetry::normalize(
        "gemini-headless-json-2026-09",
        &json!({"response":"SECRET_CANARY","stats":{"models":{"m":{"tokens":{"total":50}}}}}),
        "v",
        "a",
        100,
    )
    .unwrap();
    assert!(x.samples.is_empty());
    assert!(x.metrics["subscription_quota"].is_null());
    assert!(!serde_json::to_string(&x).unwrap().contains("SECRET_CANARY"));
}
#[test]
fn quota_oversize_count_rejected() {
    assert!(telemetry::normalize(
        "copilot-context-event-2026-09",
        &json!({"currentTokens":1000000000001u64}),
        "v",
        "a",
        100
    )
    .is_err());
}
#[test]
fn wire_does_not_accept_ndjson_as_framed() {
    let b = b"{\"jsonrpc\":\"2.0\",\"id\":1}\n";
    assert!(wire::decode(b).unwrap().is_none());
}
#[test]
fn wire_rejects_truncated_header_duplicate_length() {
    assert!(wire::decode(b"Content-Length: 10\r\ncontent-length: 2\r\n\r\n{}").is_err());
}
fn process(pid: u32, birth: u64) -> ProcessFact {
    ProcessFact {
        key: ProcessKey {
            host: "host".into(),
            boot: "boot".into(),
            pid,
            created: birth,
        },
        parent: None,
        executable_digest: Some("a".repeat(64)),
        private_bytes: None,
        cpu_ticks: None,
        io_bytes: None,
        coverage: Coverage::Complete,
    }
}
#[test]
fn hook_has_no_pid_supplied_in_payload() {
    let parent = process(10, 1);
    let mut child = process(11, 2);
    child.parent = Some(parent.key.clone());
    let g = HostGraph {
        observed_at: 100,
        uptime_ms: None,
        coverage: Coverage::Complete,
        processes: vec![parent.clone(), child],
        executables: vec![ExecutableObservation {
            process: parent.key.clone(),
            path: "D:/agent.exe".into(),
            disk_digest: parent.executable_digest.clone(),
            matching_integrations: vec!["profile".into()],
            loaded_image_identity_verified: false,
        }],
        denied: 0,
        notices: vec![],
    };
    assert_eq!(hook_ancestor(&g, 11, "profile").unwrap(), Some(parent.key));
    assert!(hook_ancestor(&g, 999, "profile").unwrap().is_none());
}
#[test]
fn partial_hook_ancestry_not_promoted() {
    let mut parent = process(10, 1);
    parent.coverage = Coverage::Partial;
    let mut child = process(11, 2);
    child.parent = Some(parent.key.clone());
    let g = HostGraph {
        observed_at: 100,
        uptime_ms: None,
        coverage: Coverage::Partial,
        processes: vec![parent, child],
        executables: vec![],
        denied: 1,
        notices: vec![],
    };
    assert!(hook_ancestor(&g, 11, "profile").unwrap().is_none());
}
#[test]
fn no_untrusted_hook_gets_owned_binding() {
    let p = process(1, 10);
    let x = OwnershipInput {
        observed_at: 200,
        process_coverage: Coverage::Complete,
        processes: vec![p.clone()],
        previous: vec![p.clone()],
        ended_sessions: BTreeMap::from([("s".into(), 100)]),
        bindings: vec![Binding {
            process: p.key,
            session_id: "s".into(),
            source: "observed_configured_hook_ancestry_not_exclusive".into(),
            shared: true,
            seen_at: 100,
            expires_at: 300,
        }],
    };
    let r = correlate(&x).unwrap();
    assert!(!r[0].retained_candidate);
    assert!(r[0].protected);
}
