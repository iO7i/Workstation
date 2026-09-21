//! Authored implementation tests. These were NOT executed during the source-only delivery.
use serde_json::json;
use std::collections::BTreeMap;
use workstation_core::{
    effects::*, integrations::*, lifecycle, ownership::*, usage_import, Coverage,
};
fn profile() -> Integration {
    Integration {
        id: "codex-profile".into(),
        project_id: "p".into(),
        environment: "dev".into(),
        adapter: Adapter::Codex,
        executable: "D:/Tools/codex.exe".into(),
        executable_sha256: "a".repeat(64),
        version_text: "fixture-version".into(),
        inherit_env: vec!["PATH".into()],
        configuration: BTreeMap::new(),
        authentication: BTreeMap::new(),
        protocol_revision: None,
        auth_method: None,
        observed_at: 100,
    }
}
fn plan() -> EffectPlan {
    EffectPlan {
        id: "plan".into(),
        project_id: "p".into(),
        environment: "dev".into(),
        policy: EFFECT_POLICY.into(),
        created_at: 100,
        expires_at: 200,
        secret_generation_digest: "c".repeat(64),
        inherited_paths_digest: "a".repeat(64),
        effect: Effect::IntegrationQuery {
            integration_id: "codex-profile".into(),
            integration_digest: "b".repeat(64),
            query: QueryKind::Version,
        },
        warnings: vec![],
    }
}
fn fact(created: u64) -> ProcessFact {
    ProcessFact {
        key: ProcessKey {
            host: "h".into(),
            boot: "boot".into(),
            pid: 42,
            created,
        },
        parent: None,
        executable_digest: None,
        private_bytes: None,
        cpu_ticks: None,
        io_bytes: None,
        coverage: Coverage::Complete,
    }
}
fn ownership_input() -> OwnershipInput {
    let mut p = fact(10);
    p.executable_digest = Some("a".repeat(64));
    OwnershipInput {
        observed_at: 200,
        process_coverage: Coverage::Complete,
        processes: vec![p.clone()],
        bindings: vec![Binding {
            process: p.key.clone(),
            session_id: "s".into(),
            source: "observed_owned_child_and_protocol_session".into(),
            shared: false,
            seen_at: 90,
            expires_at: 300,
        }],
        ended_sessions: BTreeMap::from([("s".into(), 100)]),
        previous: vec![p],
    }
}
#[test]
fn plan_roundtrips_with_no_shell() {
    let p = plan();
    p.validate().unwrap();
    let b = serde_json::to_vec(&p).unwrap();
    let q: EffectPlan = serde_json::from_slice(&b).unwrap();
    assert_eq!(q.id, p.id);
}
#[test]
fn expired_plan_denied() {
    assert!(plan()
        .approve(&"a".repeat(64), &"a".repeat(64), 200)
        .is_err());
}
#[test]
fn approval_not_transferable() {
    assert!(plan()
        .approve(&"a".repeat(64), &"b".repeat(64), 150)
        .is_err());
}
#[test]
fn future_clock_denied() {
    assert!(plan()
        .approve(&"a".repeat(64), &"a".repeat(64), 99)
        .is_err());
}
#[test]
fn policy_drift_denied() {
    let mut p = plan();
    p.policy = "old".into();
    assert!(p.validate().is_err());
}
#[test]
fn unbounded_expiry_denied() {
    let mut p = plan();
    p.expires_at = 10000;
    assert!(p.validate().is_err());
}
#[test]
fn profile_auth_is_scoped_to_provider() {
    let mut i = profile();
    i.authentication.insert("GH_TOKEN".into(), "r".into());
    assert!(i.validate().is_err());
}
#[test]
fn loader_environment_cannot_be_inherited() {
    let mut i = profile();
    i.inherit_env.push("NODE_OPTIONS".into());
    assert!(i.validate().is_err());
}
#[test]
fn config_values_must_be_paths() {
    let mut i = profile();
    i.configuration
        .insert("CODEX_HOME".into(), "relative".into());
    assert!(i.validate().is_err());
}
#[test]
fn version_claim_not_certification() {
    assert_eq!(capabilities(Adapter::Claude).tested_versions.len(), 0);
}
#[test]
fn side_effect_override_rejected() {
    let v = json!({"operation":"shell","command":"del C:\\*"});
    assert!(serde_json::from_value::<Effect>(v).is_err());
}
#[test]
fn process_name_or_age_never_permission() {
    let mut i = ownership_input();
    i.bindings.clear();
    let r = correlate(&i).unwrap();
    assert!(!r[0].retained_candidate);
    assert!(r[0].protected);
}
#[test]
fn shared_service_stays_protected() {
    let mut i = ownership_input();
    i.bindings[0].shared = true;
    assert!(!correlate(&i).unwrap()[0].retained_candidate);
}
#[test]
fn partial_inventory_suppresses_stale_guess() {
    let mut i = ownership_input();
    i.process_coverage = Coverage::Partial;
    assert!(!correlate(&i).unwrap()[0].retained_candidate);
}
#[test]
fn explicit_repeat_is_candidate_only() {
    let r = correlate(&ownership_input()).unwrap();
    assert!(r[0].retained_candidate);
    assert!(r[0].protected);
}
#[test]
fn pid_reuse_breaks_repeat() {
    let mut i = ownership_input();
    i.previous = vec![fact(9)];
    assert!(!correlate(&i).unwrap()[0].retained_candidate);
}
#[test]
fn expired_binding_not_reassigned() {
    let mut i = ownership_input();
    i.bindings[0].expires_at = 199;
    let r = correlate(&i).unwrap();
    assert!(r[0].sessions.is_empty());
}
#[test]
fn invalid_parent_birth_rejected() {
    let mut i = ownership_input();
    i.processes[0].parent = Some(fact(999).key);
    assert!(correlate(&i).is_err());
}
#[test]
fn hook_stop_not_session_end() {
    let x = lifecycle::normalize(
        "claude-hook",
        &json!({"hook_event_name":"Stop","session_id":"abc"}),
        "e",
        "p",
        None,
        100,
        "fixture",
    )
    .unwrap();
    assert_eq!(x.event, "turn_completed");
}
#[test]
fn hook_does_not_read_transcript_path() {
    let x=lifecycle::normalize("claude-hook",&json!({"hook_event_name":"SessionStart","session_id":"abc","transcript_path":"PRIVATE_CANARY","prompt":"PRIVATE_CANARY"}),"e","p",None,100,"fixture").unwrap();
    assert!(!serde_json::to_string(&x)
        .unwrap()
        .contains("PRIVATE_CANARY"));
}
#[test]
fn unknown_hook_not_accepted() {
    assert!(lifecycle::normalize(
        "claude-hook",
        &json!({"hook_event_name":"RunShell","session_id":"abc"}),
        "e",
        "p",
        None,
        100,
        "fixture"
    )
    .is_err());
}
#[test]
fn duplicate_cost_window_rejected() {
    let b = usage_import::CostBucket {
        start: 1,
        end: 2,
        amount_usd_nanos: 10,
    };
    assert!(usage_import::cost_summary(&[b.clone(), b]).is_err());
}
#[test]
fn overlapping_cost_window_rejected() {
    assert!(usage_import::cost_summary(&[
        usage_import::CostBucket {
            start: 1,
            end: 3,
            amount_usd_nanos: 1
        },
        usage_import::CostBucket {
            start: 2,
            end: 4,
            amount_usd_nanos: 1
        }
    ])
    .is_err());
}
#[test]
fn high_precision_anthropic_cents_preserved() {
    assert_eq!(
        usage_import::decimal_fixed("123.78912", 7).unwrap(),
        1_237_891_200
    );
}
#[test]
fn tiny_scientific_usd_preserved() {
    assert_eq!(usage_import::decimal_fixed("1e-9", 9).unwrap(), 1);
}
#[test]
fn precision_loss_is_not_silent() {
    assert!(usage_import::decimal_fixed("1e-10", 9).is_err());
}
#[test]
fn extreme_exponent_rejected_without_abs_overflow() {
    assert!(usage_import::decimal_fixed("1e-2147483648", 9).is_err());
}
#[test]
fn cost_currency_must_match() {
    assert!(usage_import::cost_page("openai",&json!({"data":[{"start_time":1,"end_time":2,"results":[{"amount":{"value":1,"currency":"eur"}}]}]})).is_err());
}
#[test]
fn quota_csv_rejects_code_cells() {
    let text="provider,account,bucket,meter,unit,window,used,limit,reset_at,observed_at\nx,a,b,subscription,percent,w,=RUN(),100,1000,100";
    assert!(usage_import::quota_csv(text, 200).is_err());
}
#[test]
fn unknown_subscriptions_are_not_invented() {
    assert!(usage_import::copilot_quota(&json!({}), "a", 100, "fixture").is_err());
}
#[test]
fn task_refuses_generic_shell() {
    let t = ApprovedTask {
        id: "t".into(),
        project_id: "p".into(),
        environment: "dev".into(),
        executable: "D:/pwsh.exe".into(),
        executable_sha256: "a".repeat(64),
        cwd: "D:/Repo".into(),
        directory_identity: "identity".into(),
        args: vec![],
        script_pins: vec![],
        secret_bindings: BTreeMap::new(),
        timeout_seconds: 10,
        output_policy: "discard".into(),
    };
    assert!(t.validate().is_err());
}
#[test]
fn auth_environment_not_configuration() {
    let mut p = profile();
    p.configuration
        .insert("OPENAI_API_KEY".into(), "D:/something".into());
    assert!(p.validate().is_err());
}

// v4 strengthened the stale-candidate precondition. The positive fixture above now
// supplies exact owned protocol evidence; an explicit negative keeps untrusted input out.
#[test]
fn untrusted_import_does_not_become_retained_proof() {
    let mut i = ownership_input();
    i.bindings[0].source = "fixture_only".into();
    assert!(!correlate(&i).unwrap()[0].retained_candidate);
}
