//! Actual CLI/SQLite tests when cargo test is run. NOT executed in the Linux source-authoring host.
use serde_json::{json, Value};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};
use workstation_platform::{control_store::sha, storage::Store};
fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_workstation"))
}
fn invoke(home: &Path, args: &[&str]) -> Output {
    Command::new(binary())
        .env_remove("WORKSTATION_HOME")
        .arg("--home")
        .arg(home)
        .arg("--json")
        .args(args)
        .output()
        .unwrap()
}
fn data(o: &Output) -> Value {
    serde_json::from_slice::<Value>(&o.stdout).unwrap()["data"].clone()
}
fn setup() -> (tempfile::TempDir, PathBuf) {
    let t = tempfile::tempdir().unwrap();
    let home = t.path().join("home");
    assert!(invoke(&home, &["init"]).status.success());
    let repo = t.path().join("repository");
    fs::create_dir(&repo).unwrap();
    let mut store = Store::open(&home).unwrap();
    // Only metadata registration. This fixture executable is NEVER called as Git.
    store
        .register_project(&workstation_core::Project {
            id: "p".into(),
            path: repo,
            git_executable: binary(),
        })
        .unwrap();
    drop(store);
    assert!(invoke(&home, &["upgrade"]).status.success());
    (t, home)
}
fn write(t: &Path, v: &Value) -> (PathBuf, String) {
    let p = t.join(format!("input-{}.json", workstation_platform::new_id()));
    let bytes = serde_json::to_vec(v).unwrap();
    fs::write(&p, &bytes).unwrap();
    (p, sha(&bytes))
}
fn record(home: &Path, t: &Path, v: &Value) -> Output {
    let (p, h) = write(t, v);
    invoke(
        home,
        &[
            "record",
            "--input",
            p.to_str().unwrap(),
            "--approve-sha256",
            &h,
        ],
    )
}
fn work() -> Value {
    json!({"operation":"work_create","id":"w","project_id":"p","objective":"Finish a synthetic failing test","priority":2,"workspace_id":null})
}
#[test]
fn control_requires_explicit_upgrade() {
    let t = tempfile::tempdir().unwrap();
    let home = t.path().join("home");
    assert!(invoke(&home, &["init"]).status.success());
    assert!(!invoke(&home, &["context", "--project", "p"])
        .status
        .success());
}
#[test]
fn upgrade_is_idempotent_and_preserves_registered_project() {
    let (t, h) = setup();
    let r = invoke(&h, &["upgrade"]);
    assert!(r.status.success());
    assert_eq!(data(&r)["changed"], false);
    let c = invoke(&h, &["context", "--project", "p"]);
    assert!(c.status.success());
    assert_eq!(data(&c)["project_id"], "p");
    drop(t);
}
#[test]
fn record_preview_does_not_create_work() {
    let (t, h) = setup();
    let (p, _) = write(t.path(), &work());
    let r = invoke(&h, &["record", "--input", p.to_str().unwrap()]);
    assert!(r.status.success());
    assert_eq!(data(&r)["status"], "preview_only");
    let c = data(&invoke(&h, &["context", "--project", "p"]));
    assert!(c["work_items"].as_array().unwrap().is_empty());
}
#[test]
fn mismatched_record_digest_does_not_create_work() {
    let (t, h) = setup();
    let (p, _) = write(t.path(), &work());
    let r = invoke(
        &h,
        &[
            "record",
            "--input",
            p.to_str().unwrap(),
            "--approve-sha256",
            &"a".repeat(64),
        ],
    );
    assert!(!r.status.success());
    assert!(
        data(&invoke(&h, &["context", "--project", "p"]))["work_items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}
#[test]
fn approved_metadata_is_visible_without_agent_calls() {
    let (t, h) = setup();
    assert!(record(&h, t.path(), &work()).status.success());
    let c = data(&invoke(&h, &["context", "--project", "p"]));
    assert_eq!(c["work_items"][0]["state"], "planned");
    assert_eq!(c["network_calls"], 0);
    assert_eq!(
        c["continuity"]["handoff"],
        "packet_or_explicit_continuation_effect"
    );
}
#[test]
fn unaccepted_decision_is_not_current() {
    let (t, h) = setup();
    let d = json!({"operation":"decision_propose","id":"d1","project_id":"p","topic":"storage","scope":"all","statement":"Use local SQLite","rationale":"Bounded local metadata","alternatives":[],"consequences":[],"predecessor":null,"effective_at":0});
    assert!(record(&h, t.path(), &d).status.success());
    assert!(
        data(&invoke(&h, &["decisions", "--project", "p"]))["decisions"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(record(
        &h,
        t.path(),
        &json!({"operation":"decision_accept","decision_id":"d1"})
    )
    .status
    .success());
    assert_eq!(
        data(&invoke(&h, &["decisions", "--project", "p"]))["decisions"][0]["id"],
        "d1"
    );
}
#[test]
fn explicit_environment_has_no_fallback() {
    let (t, h) = setup();
    assert!(record(
        &h,
        t.path(),
        &json!({"operation":"environment_add","project_id":"p","name":"dev"})
    )
    .status
    .success());
    assert!(record(&h,t.path(),&json!({"operation":"resource_add","id":"r","project_id":"p","environment":"dev","kind":"endpoint","name":"Example","locator":"https://example.org","provider":null,"fresh_for_secs":3600})).status.success());
    assert!(!invoke(
        &h,
        &["resources", "--project", "p", "--environment", "prod"]
    )
    .status
    .success());
    assert_eq!(
        data(&invoke(
            &h,
            &["resources", "--project", "p", "--environment", "dev"]
        ))["resources"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}
#[test]
fn malformed_manifest_cannot_gain_execution_authority() {
    let (t, h) = setup();
    let mut v = work();
    v["shell"] = json!("echo unsafe");
    assert!(!record(&h, t.path(), &v).status.success());
}
#[test]
fn obvious_secret_canary_is_not_accepted() {
    let (t, h) = setup();
    let mut v = work();
    v["objective"] = json!("SECRET_CANARY_DO_NOT_STORE");
    let r = record(&h, t.path(), &v);
    assert!(!r.status.success());
    assert!(!String::from_utf8_lossy(&r.stdout).contains("SECRET_CANARY"));
}
#[test]
fn share_context_omits_project_and_objective() {
    let (t, h) = setup();
    let mut v = work();
    v["objective"] = json!("private-objective-fixture-123");
    assert!(record(&h, t.path(), &v).status.success());
    let r = invoke(&h, &["context", "--project", "p", "--share"]);
    assert!(r.status.success());
    let text = String::from_utf8(r.stdout).unwrap();
    assert!(!text.contains("private-objective-fixture-123"));
    assert!(!text.contains("project_id"));
}
#[test]
fn mcp_initialize_and_read_cannot_claim_work() {
    let (_t, h) = setup();
    let mut child = Command::new(binary())
        .arg("--home")
        .arg(&h)
        .args(["mcp", "--project", "p"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let messages = [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"test","version":"fixture"}}}),
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"project_context","arguments":{}}}),
        json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"assign","arguments":{}}}),
    ];
    let mut input = child.stdin.take().unwrap();
    for m in messages {
        writeln!(input, "{}", m).unwrap();
    }
    drop(input);
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    let lines = String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[1]["result"]["isError"], false);
    assert!(lines[2].get("error").is_some());
}
#[test]
fn pure_model_analysis_requires_no_home() {
    let t = tempfile::tempdir().unwrap();
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap()
        .join("fixtures/control/models.json");
    let o = Command::new(binary())
        .env_remove("WORKSTATION_HOME")
        .args(["--json", "model-frontier", "--input"])
        .arg(fixtures)
        .current_dir(t.path())
        .output()
        .unwrap();
    assert!(
        o.status.success(),
        "status={:?}\nstdout={}\nstderr={}",
        o.status.code(),
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    );
    assert_eq!(fs::read_dir(t.path()).unwrap().count(), 0);
}
#[test]
fn discovery_requires_focus_not_popularity() {
    let (_t, h) = setup();
    let c = data(&invoke(&h, &["capabilities", "--project", "p"]));
    assert!(c["useful_discoveries"].as_array().unwrap().is_empty());
}
