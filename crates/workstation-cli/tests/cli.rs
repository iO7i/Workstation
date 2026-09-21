//! These tests exercise the actual binary when `cargo test` is run.
//! They are not represented as executed in the source-only delivery.
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    time::{Duration, Instant},
};
use workstation_core::{FixtureMode, WorkerRequest};
use workstation_platform::runner;

fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_workstation"))
}
fn invoke(home: &Path, args: &[&str]) -> Output {
    Command::new(bin())
        .env_remove("WORKSTATION_HOME")
        .arg("--home")
        .arg(home)
        .arg("--json")
        .args(args)
        .output()
        .unwrap()
}
fn parsed(out: &Output) -> Value {
    serde_json::from_slice(&out.stdout).expect("stdout must be exactly one JSON envelope")
}
fn init(home: &Path) {
    let out = invoke(home, &["init"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
}

#[test]
fn initialization_is_explicit_and_never_overwrites() {
    let t = tempfile::tempdir().unwrap();
    let home = t.path().join("home");
    init(&home);
    let before = fs::read(home.join("config.json")).unwrap();
    assert!(!invoke(&home, &["init"]).status.success());
    assert_eq!(before, fs::read(home.join("config.json")).unwrap());
}

#[test]
fn missing_home_does_not_materialize_a_directory() {
    let t = tempfile::tempdir().unwrap();
    let home = t.path().join("missing");
    let out = invoke(&home, &["doctor"]);
    assert_eq!(parsed(&out)["errors"][0], "HOME_UNAVAILABLE");
    assert!(!home.exists());
}

#[test]
fn no_home_is_not_inferred_from_current_directory() {
    let t = tempfile::tempdir().unwrap();
    let out = Command::new(bin())
        .env_remove("WORKSTATION_HOME")
        .current_dir(t.path())
        .args(["--json", "doctor"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(4));
    assert_eq!(fs::read_dir(t.path()).unwrap().count(), 0);
}

#[test]
fn doctor_does_not_open_or_echo_scanned_file_contents() {
    let t = tempfile::tempdir().unwrap();
    let home = t.path().join("home");
    let data = t.path().join("private-root");
    fs::create_dir(&data).unwrap();
    let canary = b"R0_SECRET_CANARY_DO_NOT_PRINT_193884";
    fs::write(data.join(".env"), canary).unwrap();
    init(&home);
    let add = invoke(
        &home,
        &[
            "root",
            "add",
            "--id",
            "canary-root",
            "--path",
            data.to_str().unwrap(),
            "--kind",
            "other",
        ],
    );
    assert!(add.status.success());
    let out = invoke(&home, &["doctor"]);
    // Non-Windows native collectors are intentionally Unsupported, yielding exit 3.
    assert!([Some(0), Some(3)].contains(&out.status.code()));
    let v = parsed(&out);
    let snapshot = &v["data"];
    assert_eq!(snapshot["storage"][0]["logical_entry_bytes"], canary.len());
    assert_eq!(snapshot["storage"][0]["coverage"], "complete");
    assert_eq!(fs::read(data.join(".env")).unwrap(), canary);
    assert!(!String::from_utf8_lossy(&out.stdout).contains("R0_SECRET_CANARY"));
    assert!(snapshot["findings"]
        .as_array()
        .unwrap()
        .iter()
        .all(|f| f["repair_available"] == false));
    let share = invoke(&home, &["report", "--share"]);
    let text = String::from_utf8_lossy(&share.stdout);
    assert!(!text.contains("canary-root"));
    assert!(!text.contains("private-root"));
    assert!(!text.contains("R0_SECRET_CANARY"));
    assert!(!text.contains("installation_id"));
    let _ = parsed(&share);
}

#[test]
fn cached_report_does_not_invoke_a_new_scan() {
    let t = tempfile::tempdir().unwrap();
    let home = t.path().join("home");
    init(&home);
    let out = invoke(&home, &["doctor"]);
    let observed = parsed(&out)["data"]["observed_at"].clone();
    let report = invoke(&home, &["report"]);
    let v = parsed(&report);
    assert_eq!(v["data"]["observed_at"], observed);
    assert!(v["warnings"]
        .as_array()
        .unwrap()
        .contains(&Value::String("CACHED_NOT_LIVE".into())));
}

#[test]
fn no_save_does_not_create_a_saved_snapshot() {
    let t = tempfile::tempdir().unwrap();
    let home = t.path().join("home");
    init(&home);
    let _ = invoke(&home, &["doctor", "--no-save"]);
    let report = invoke(&home, &["report"]);
    assert_eq!(parsed(&report)["errors"][0], "NO_SAVED_SNAPSHOT");
}

#[test]
fn unicode_paths_remain_intact_and_private_html_is_escaped() {
    let t = tempfile::tempdir().unwrap();
    let home = t.path().join("home");
    init(&home);
    let data = t.path().join("مشروع 日本語 & files");
    fs::create_dir(&data).unwrap();
    assert!(invoke(
        &home,
        &[
            "root",
            "add",
            "--id",
            "unicode",
            "--path",
            data.to_str().unwrap(),
            "--kind",
            "workspace"
        ]
    )
    .status
    .success());
    let _ = invoke(&home, &["doctor"]);
    let out = Command::new(bin())
        .arg("--home")
        .arg(&home)
        .args(["report", "--html"])
        .output()
        .unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("مشروع 日本語 &amp; files"));
    assert!(text.contains("Content-Security-Policy"));
    assert!(!text.contains("<script"));
}

#[test]
fn repository_registration_requires_explicit_trust() {
    let t = tempfile::tempdir().unwrap();
    let home = t.path().join("home");
    init(&home);
    let out = invoke(
        &home,
        &[
            "project",
            "add",
            t.path().to_str().unwrap(),
            "--id",
            "demo",
            "--git",
            "not-selected",
        ],
    );
    assert_eq!(
        parsed(&out)["errors"][0],
        "EXPLICIT_REPOSITORY_TRUST_REQUIRED"
    );
}

#[test]
fn collector_success_uses_protocol_not_shell() {
    let t = tempfile::tempdir().unwrap();
    let out = runner::collect(
        &bin(),
        t.path(),
        &WorkerRequest::Fixture {
            mode: FixtureMode::Exit,
        },
        Duration::from_secs(3),
    )
    .unwrap();
    assert_eq!(out.exit_code, 0);
    assert_eq!(
        serde_json::from_slice::<Value>(&out.bytes).unwrap()["ok"],
        true
    );
}

#[test]
fn collector_timeout_is_bounded() {
    let t = tempfile::tempdir().unwrap();
    let start = Instant::now();
    let result = runner::collect(
        &bin(),
        t.path(),
        &WorkerRequest::Fixture {
            mode: FixtureMode::Sleep,
        },
        Duration::from_millis(300),
    );
    assert_eq!(result.err().unwrap().code, "WORKER_TIMEOUT");
    assert!(start.elapsed() < Duration::from_secs(5));
}

#[test]
fn collector_flood_is_rejected_and_drained() {
    let t = tempfile::tempdir().unwrap();
    let result = runner::collect(
        &bin(),
        t.path(),
        &WorkerRequest::Fixture {
            mode: FixtureMode::Flood,
        },
        Duration::from_secs(4),
    );
    assert_eq!(result.err().unwrap().code, "WORKER_OUTPUT_LIMIT");
}

#[cfg(windows)]
#[test]
fn collector_descendant_does_not_survive_its_job() {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, WAIT_TIMEOUT},
        System::Threading::{OpenProcess, WaitForSingleObject},
    };
    let t = tempfile::tempdir().unwrap();
    let out = runner::collect(
        &bin(),
        t.path(),
        &WorkerRequest::Fixture {
            mode: FixtureMode::Descendant,
        },
        Duration::from_secs(4),
    )
    .unwrap();
    let v: Value = serde_json::from_slice(&out.bytes).unwrap();
    let pid = v["descendant_pid"].as_u64().unwrap() as u32;
    // SAFETY: read-only wait handle; this test never kills a PID after the observation.
    unsafe {
        let h = OpenProcess(0x0010_0000, 0, pid);
        if !h.is_null() {
            let state = WaitForSingleObject(h, 2000);
            CloseHandle(h);
            assert_ne!(state, WAIT_TIMEOUT);
        }
    }
}

#[test]
fn all_incident_fixtures_are_non_destructive_replays() {
    let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/incidents");
    for file in fs::read_dir(folder).unwrap() {
        let path = file.unwrap().path();
        if path.extension().is_none_or(|x| x != "json") {
            continue;
        }
        let out = Command::new(bin())
            .args(["--json", "replay-fixture"])
            .arg(&path)
            .output()
            .unwrap();
        assert!(out.status.success());
        let v = parsed(&out);
        assert_eq!(v["data"]["synthetic_replay"], true);
        assert_eq!(v["data"]["finding"]["repair_available"], false);
    }
}
