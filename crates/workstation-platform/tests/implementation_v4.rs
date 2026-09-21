//! Native/runtime tests are authored, not executed until the user's certification approval.
use std::path::Path;
use workstation_platform::{storage::Store, workspace_lifecycle::require_missing_entry};
fn store() -> (tempfile::TempDir, Store) {
    let d = tempfile::tempdir().unwrap();
    let mut s = Store::init(&d.path().join("home")).unwrap();
    s.upgrade().unwrap();
    (d, s)
}
#[test]
fn upgrade_reaches_current_schema_v5() {
    let (_d, s) = store();
    assert_eq!(s.schema_version().unwrap(), 5);
}
#[test]
fn upgrade_is_idempotent_at_schema_v5() {
    let (_d, mut s) = store();
    assert_eq!(s.upgrade().unwrap()["changed"], false);
}
#[test]
fn missing_destination_not_unreadable() {
    let d = tempfile::tempdir().unwrap();
    assert!(require_missing_entry(&d.path().join("missing")).is_ok());
}
#[test]
fn directory_not_unused_destination() {
    let d = tempfile::tempdir().unwrap();
    assert!(require_missing_entry(d.path()).is_err());
}
#[test]
fn file_not_unused_destination() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("data");
    std::fs::write(&p, b"keep").unwrap();
    assert!(require_missing_entry(&p).is_err());
    assert_eq!(std::fs::read(&p).unwrap(), b"keep");
}
#[cfg(unix)]
#[test]
fn dangling_symlink_not_unused_destination() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("link");
    std::os::unix::fs::symlink("missing-target", &p).unwrap();
    assert!(require_missing_entry(&p).is_err());
}
#[test]
fn secret_recovery_wrong_environment_rejected() {
    let (_d, s) = store();
    assert!(s.recovery_inventory("missing", "prod").is_err());
}
#[test]
fn atlas_never_falls_back() {
    let (_d, s) = store();
    assert!(s.atlas_status("missing", "prod").is_err());
}
#[test]
fn descriptor_is_not_credential_access() {
    let (_d, s) = store();
    assert!(s.secret_history("missing", "prod", "s").is_err());
}
#[test]
fn root_cleanup_not_implicit_plan() {
    let (_d, s) = store();
    let op = workstation_platform::operation_runtime::Request::WorkspaceCleanup {
        workspace_id: "missing".into(),
        quiescence_acknowledged: false,
    };
    assert!(workstation_platform::operation_runtime::prepare(&s, "missing", "prod", op).is_err());
}
#[test]
fn no_uncertified_external_application() {
    let (_d, mut s) = store();
    assert!(
        workstation_platform::engine::apply(&mut s, "anything", &"a".repeat(64), false).is_err()
    );
}
#[test]
fn opaque_relative_executable_rejected() {
    assert!(workstation_platform::external::hash_file(Path::new("relative.exe")).is_err());
}
