//! Authored only. Run after the user's explicit certification authorization.
use std::path::Path;
use workstation_platform::{project_tools::ProjectManifest, storage::Store};
fn home() -> (tempfile::TempDir, Store) {
    let d = tempfile::tempdir().unwrap();
    let mut s = Store::init(&d.path().join("home")).unwrap();
    s.upgrade().unwrap();
    (d, s)
}
#[test]
fn migration_reaches_current_schema_v5() {
    let (_d, s) = home();
    assert_eq!(s.schema_version().unwrap(), 5);
}
#[test]
fn same_schema_upgrade_is_idempotent() {
    let (_d, mut s) = home();
    assert_eq!(s.upgrade().unwrap()["changed"], false);
}
#[test]
fn no_profiles_for_unknown_schema() {
    let d = tempfile::tempdir().unwrap();
    let s = Store::init(&d.path().join("home")).unwrap();
    assert!(s.integrations("p").is_err());
}
#[test]
fn manifest_forbids_execution_fields() {
    let v = serde_json::json!({"schema_version":1,"project_id":"p","environments":["dev"],"resources":[],"command":"execute"});
    assert!(serde_json::from_value::<ProjectManifest>(v).is_err());
}
#[test]
fn duplicate_manifest_environments_rejected() {
    let m = ProjectManifest {
        schema_version: 1,
        project_id: "p".into(),
        environments: vec!["dev".into(), "dev".into()],
        resources: vec![],
    };
    assert!(m.validate().is_err());
}
#[test]
fn secret_plaintext_locator_is_invalid() {
    let r = serde_json::json!({"id":"x","environment":"dev","kind":"secret_reference","name":"x","locator":"not-a-reference","provider":null,"fresh_for_secs":3600});
    let v = serde_json::json!({"schema_version":1,"project_id":"p","environments":["dev"],"resources":[r]});
    let m: ProjectManifest = serde_json::from_value(v).unwrap();
    assert!(m.validate().is_err());
}
#[test]
fn effect_prepare_does_not_enable_unknown_environment() {
    let (_d, mut s) = home();
    let p = workstation_platform::engine::Prepare::Query {
        integration_id: "missing".into(),
        query: workstation_core::effects::QueryKind::Version,
    };
    assert!(workstation_platform::engine::prepare(&mut s, "p", "prod", p).is_err());
}
#[test]
fn effect_apply_needs_prerelease_ack() {
    let (_d, mut s) = home();
    assert!(workstation_platform::engine::apply(&mut s, "absent", &"a".repeat(64), false).is_err());
}
#[test]
fn no_secret_resolution_public_mcp_method() {
    let (_d, s) = home();
    let unknown = s.read_tool(
        "missing",
        Some("prod"),
        "secret_resolve",
        &serde_json::json!({"id":"r"}),
    );
    assert!(unknown.is_err());
}
#[test]
fn referenced_task_cannot_bypass_directory_identity() {
    let task = workstation_core::integrations::ApprovedTask {
        id: "x".into(),
        project_id: "p".into(),
        environment: "dev".into(),
        executable: "D:/missing.exe".into(),
        executable_sha256: "a".repeat(64),
        cwd: "D:/missing".into(),
        directory_identity: "wrong".into(),
        args: vec![],
        script_pins: vec![],
        secret_bindings: std::collections::BTreeMap::new(),
        timeout_seconds: 5,
        output_policy: "discard".into(),
    };
    assert!(workstation_platform::tasks::validate_files(&task).is_err());
}
#[test]
fn relative_pin_target_rejected() {
    assert!(workstation_platform::external::hash_file(Path::new("relative")).is_err());
}
