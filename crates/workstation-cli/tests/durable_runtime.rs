//! Executes the real CLI + Win32 child transport against an offline fake provider.
#![cfg(all(windows, feature = "runtime-test-fixtures"))]
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Child, Command, Output, Stdio},
    time::{Duration, Instant},
};
use workstation_platform::{
    control_store::{epoch, sha},
    external::hash_file,
};
fn bin() -> PathBuf {
    env!("CARGO_BIN_EXE_workstation").into()
}
fn fixture_bin() -> PathBuf {
    env!("CARGO_BIN_EXE_workstation-protocol-fixture").into()
}
fn command(home: &Path, args: &[&str]) -> Command {
    let mut c = Command::new(bin());
    c.env_remove("WORKSTATION_HOME")
        .arg("--home")
        .arg(home)
        .arg("--json")
        .args(args);
    c
}
fn invoke(home: &Path, args: &[&str]) -> Output {
    command(home, args).output().unwrap()
}
fn value(o: &Output) -> Value {
    serde_json::from_slice(&o.stdout)
        .unwrap_or_else(|e| panic!("{e}: {}", String::from_utf8_lossy(&o.stdout)))
}
fn data(o: Output) -> Value {
    let v = value(&o);
    assert!(o.status.success(), "{}", v);
    v["data"].clone()
}
struct Lab {
    _temp: tempfile::TempDir,
    home: PathBuf,
    repo: PathBuf,
    agent: PathBuf,
}
impl Lab {
    fn file(&self, name: &str, v: &Value) -> PathBuf {
        let p = self._temp.path().join(name);
        fs::write(&p, serde_json::to_vec(v).unwrap()).unwrap();
        p
    }
    fn record(&self, v: Value) {
        let p = self.file(&format!("{}.json", workstation_platform::new_id()), &v);
        let bytes = fs::read(&p).unwrap();
        data(invoke(
            &self.home,
            &[
                "record",
                "--input",
                p.to_str().unwrap(),
                "--approve-sha256",
                &sha(&bytes),
            ],
        ));
    }
    fn new(mode: &str) -> Self {
        let t = tempfile::tempdir().unwrap();
        let home = t.path().join("home");
        let repo = t.path().join("repo");
        fs::create_dir(&repo).unwrap();
        let agent = t.path().join(format!("provider-{mode}.exe"));
        fs::copy(fixture_bin(), &agent).unwrap();
        let git = std::env::split_paths(&std::env::var_os("PATH").unwrap())
            .map(|p| p.join("git.exe"))
            .find(|p| p.is_file())
            .expect("Git is required for native fixtures");
        let g = |args: &[&str]| {
            let out = Command::new(&git)
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_CONFIG_GLOBAL", "NUL")
                .args(["-c", "core.hooksPath=", "-c", "core.fsmonitor=false", "-C"])
                .arg(&repo)
                .args(args)
                .output()
                .unwrap();
            assert!(
                out.status.success(),
                "{}",
                String::from_utf8_lossy(&out.stderr)
            );
        };
        g(&["init", "--template=", "-q"]);
        g(&["config", "user.name", "Fixture"]);
        g(&["config", "user.email", "fixture@example.invalid"]);
        for (name, text) in [
            ("result.txt", "initial"),
            ("prompt-count.txt", "0"),
            ("test_contract.txt", "do-not-modify"),
        ] {
            fs::write(repo.join(name), text).unwrap();
        }
        g(&["add", "."]);
        g(&["commit", "-qm", "offline fixture"]);
        data(invoke(&home, &["init"]));
        data(invoke(&home, &["upgrade"]));
        data(invoke(
            &home,
            &[
                "project",
                "add",
                "--id",
                "p",
                "--git",
                git.to_str().unwrap(),
                "--trust-repository",
                repo.to_str().unwrap(),
            ],
        ));
        data(invoke(
            &home,
            &[
                "workspace-register",
                "--project",
                "p",
                "--id",
                "ws",
                "--path",
                repo.to_str().unwrap(),
            ],
        ));
        let lab = Self {
            _temp: t,
            home,
            repo,
            agent,
        };
        lab.record(json!({"operation":"environment_add","project_id":"p","name":"dev"}));
        lab.record(json!({"operation":"work_create","id":"w","project_id":"p","objective":"offline protocol runtime exercise","priority":1,"workspace_id":"ws"}));
        let p=lab.file("integration.json",&json!({"id":"fake","project_id":"p","environment":"dev","adapter":"codex","executable":lab.agent,"executable_sha256":hash_file(&lab.agent).unwrap(),"version_text":"fixture-v1","inherit_env":[],"configuration":{},"authentication":{},"observed_at":epoch()}));
        let preview = data(invoke(
            &lab.home,
            &["integration", "register", "--input", p.to_str().unwrap()],
        ));
        data(invoke(
            &lab.home,
            &[
                "integration",
                "register",
                "--input",
                p.to_str().unwrap(),
                "--approve-sha256",
                preview["canonical_sha256"].as_str().unwrap(),
            ],
        ));
        lab
    }
    fn prepare(&self) -> (String, String) {
        let p=self.file("checkpoint.json",&json!({"session_id":null,"completed":[],"remaining":["Offline fixture action only"],"blockers":[],"tests":[],"decision_ids":[],"resource_ids":[]}));
        let cp = data(invoke(
            &self.home,
            &["checkpoint", "--work", "w", "--input", p.to_str().unwrap()],
        ));
        let p=self.file("continue.json",&json!({"operation":"continue","integration_id":"fake","checkpoint_id":cp["checkpoint"]["id"],"resume_external_id":null,"permission_mode":"allow_once","timeout_seconds":8,"deadlines":{"handshake_seconds":2,"session_seconds":2,"idle_seconds":2,"absolute_seconds":8,"cancellation_grace_seconds":1,"reconciliation_seconds":3}}));
        let v = data(invoke(
            &self.home,
            &[
                "effect",
                "prepare",
                "--project",
                "p",
                "--environment",
                "dev",
                "--input",
                p.to_str().unwrap(),
            ],
        ));
        (
            v["plan"]["id"].as_str().unwrap().into(),
            v["approve_sha256"].as_str().unwrap().into(),
        )
    }
    fn apply(&self, id: &str, digest: &str) -> Output {
        invoke(
            &self.home,
            &[
                "effect",
                "apply",
                "--id",
                id,
                "--approve-sha256",
                digest,
                "--acknowledge-uncertified-execution",
            ],
        )
    }
    fn start(&self, id: &str, digest: &str) -> Owned {
        Owned(
            command(
                &self.home,
                &[
                    "effect",
                    "apply",
                    "--id",
                    id,
                    "--approve-sha256",
                    digest,
                    "--acknowledge-uncertified-execution",
                ],
            )
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
        )
    }
    fn status(&self, id: &str) -> Value {
        data(invoke(
            &self.home,
            &["effect", "runtime", "status", "--id", id],
        ))
    }
    fn wait_for(&self, id: &str, pred: impl Fn(&Value) -> bool) -> Value {
        let end = Instant::now() + Duration::from_secs(12);
        loop {
            let v = self.status(id);
            if pred(&v) {
                return v;
            }
            assert!(Instant::now() < end, "status deadline: {v}");
            std::thread::sleep(Duration::from_millis(40));
        }
    }
    fn count(&self) -> u32 {
        fs::read_to_string(self.repo.join("prompt-count.txt"))
            .unwrap()
            .parse()
            .unwrap()
    }
    fn register_task(&self) {
        let task = json!({"id":"test","project_id":"p","environment":"dev","executable":self.agent,"executable_sha256":hash_file(&self.agent).unwrap(),"cwd":self.repo,"directory_identity":workstation_platform::control_workspace::directory_identity(&self.repo).unwrap(),"args":["--fixture-verify"],"script_pins":[],"secret_bindings":{},"timeout_seconds":5,"output_policy":"discard"});
        let p = self.file("task.json", &task);
        let v = data(invoke(
            &self.home,
            &["task", "register", "--input", p.to_str().unwrap()],
        ));
        data(invoke(
            &self.home,
            &[
                "task",
                "register",
                "--input",
                p.to_str().unwrap(),
                "--approve-sha256",
                v["canonical_sha256"].as_str().unwrap(),
            ],
        ));
    }
    fn baseline(&self, id: &str) {
        data(invoke(
            &self.home,
            &[
                "effect",
                "runtime",
                "baseline",
                "--id",
                id,
                "--acknowledge-content-hashing",
            ],
        ));
    }
    fn verify_plan(&self, id: &str) -> Value {
        let p=self.file("contract.json",&json!({"allowed_paths":["result.txt","prompt-count.txt"],"forbidden_paths":["test_contract.txt"],"task_ids":["test"]}));
        data(invoke(
            &self.home,
            &[
                "effect",
                "runtime",
                "verify",
                "--id",
                id,
                "--contract",
                p.to_str().unwrap(),
            ],
        ))
    }
}
struct Owned(Child);
impl Owned {
    fn wait(&mut self) {
        let end = Instant::now() + Duration::from_secs(15);
        loop {
            if self.0.try_wait().unwrap().is_some() {
                return;
            }
            assert!(Instant::now() < end, "fixture owner did not exit");
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}
impl Drop for Owned {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
        }
        let _ = self.0.wait();
    }
}
#[test]
fn real_cli_normal_completion_is_not_work_item_success() {
    let lab = Lab::new("normal");
    let (id, d) = lab.prepare();
    data(lab.apply(&id, &d));
    let s = lab.status(&id);
    assert_eq!(s["state"], "awaiting_verification");
    assert_eq!(s["run"]["session_id"], "fixture-session");
    assert_eq!(s["run"]["turn_id"], "fixture-turn");
    assert_eq!(lab.count(), 1);
    assert!(!lab.apply(&id, &d).status.success());
    assert_eq!(lab.count(), 1);
    let roster = data(invoke(&lab.home, &["roster", "--project", "p"]));
    assert_eq!(roster["roster"][0]["role"], "primary");
    let events = data(invoke(
        &lab.home,
        &["effect", "runtime", "events", "--id", &id],
    ));
    assert!(!events.to_string().contains("SECRET_CANARY"));
}
#[test]
fn fast_terminal_before_response_is_persisted() {
    let lab = Lab::new("early");
    let (id, d) = lab.prepare();
    data(lab.apply(&id, &d));
    let s = lab.status(&id);
    assert_eq!(s["run"]["provider"], "completed");
    assert_eq!(s["state"], "awaiting_verification");
}
#[test]
fn cancel_before_claim_prevents_prompt() {
    let lab = Lab::new("normal");
    let (id, d) = lab.prepare();
    data(invoke(
        &lab.home,
        &[
            "effect",
            "runtime",
            "cancel",
            "--id",
            &id,
            "--expected-revision",
            "0",
        ],
    ));
    assert!(!lab.apply(&id, &d).status.success());
    assert_eq!(lab.count(), 0);
    assert_eq!(lab.status(&id)["state"], "cancelled");
}
#[test]
fn live_cancel_is_observed_only_by_own_supervisor() {
    let lab = Lab::new("hang");
    let (id, d) = lab.prepare();
    let mut owner = lab.start(&id, &d);
    let s = lab.wait_for(&id, |v| v["run"]["turn_id"].is_string());
    data(invoke(
        &lab.home,
        &[
            "effect",
            "runtime",
            "cancel",
            "--id",
            &id,
            "--expected-revision",
            &s["run"]["revision"].to_string(),
        ],
    ));
    owner.wait();
    let s = lab.status(&id);
    assert_eq!(s["run"]["cancellation_requested"], true);
    assert_eq!(s["run"]["transport"], "cancelled");
    assert_eq!(lab.count(), 1);
}
#[test]
fn crashing_supervisor_is_recovered_without_replaying() {
    let lab = Lab::new("hang");
    let (id, d) = lab.prepare();
    let mut owner = lab.start(&id, &d);
    let s = lab.wait_for(&id, |v| v["run"]["turn_id"].is_string());
    let child = s["run"]["child"]["pid"].as_u64().unwrap() as u32;
    owner.0.kill().unwrap();
    owner.wait();
    std::thread::sleep(Duration::from_millis(150));
    data(invoke(
        &lab.home,
        &["effect", "runtime", "recover", "--project", "p"],
    ));
    let s = lab.status(&id);
    assert_eq!(s["run"]["transport"], "lost");
    assert_eq!(lab.count(), 1);
    assert!(!lab.apply(&id, &d).status.success());
    assert!(workstation_platform::durable_runtime::process_birth(child)
        .unwrap()
        .is_none());
}
#[test]
fn missing_terminal_is_reconciled_without_another_prompt() {
    let lab = Lab::new("lost");
    let (id, d) = lab.prepare();
    assert!(!lab.apply(&id, &d).status.success());
    let s = lab.status(&id);
    assert_eq!(s["run"]["transport"], "timed_out");
    let revision = s["run"]["revision"].to_string();
    let preview = data(invoke(
        &lab.home,
        &[
            "effect",
            "runtime",
            "reconcile",
            "--id",
            &id,
            "--expected-revision",
            &revision,
            "--provider",
        ],
    ));
    data(invoke(
        &lab.home,
        &[
            "effect",
            "runtime",
            "reconcile",
            "--id",
            &id,
            "--expected-revision",
            &revision,
            "--provider",
            "--approve-sha256",
            preview["approve_sha256"].as_str().unwrap(),
        ],
    ));
    let s = lab.status(&id);
    assert_eq!(s["run"]["provider"], "completed");
    assert_eq!(s["state"], "awaiting_verification");
    assert_eq!(lab.count(), 1);
}
fn check_deadline(mode: &str, expected: &str) {
    let lab = Lab::new(mode);
    let (id, d) = lab.prepare();
    let start = Instant::now();
    assert!(!lab.apply(&id, &d).status.success());
    assert!(start.elapsed() < Duration::from_secs(15));
    let s = lab.status(&id);
    assert_eq!(s["run"]["deadline_expired"], expected, "{s}");
    assert_eq!(s["run"]["transport"], "timed_out");
}
#[test]
fn handshake_has_own_deadline() {
    check_deadline("handshake", "handshake");
}
#[test]
fn session_start_has_own_deadline() {
    check_deadline("session-stall", "session");
}
#[test]
fn chatter_is_not_progress() {
    check_deadline("noise", "idle");
}
#[test]
fn repeated_item_is_not_progress() {
    check_deadline("duplicate", "idle");
}
#[test]
fn unique_progress_does_not_bypass_absolute_deadline() {
    check_deadline("progress", "absolute");
}
fn execute_verification(lab: &Lab, id: &str, plan: &Value) -> Output {
    invoke(
        &lab.home,
        &[
            "effect",
            "runtime",
            "verify",
            "--id",
            id,
            "--verification-id",
            plan["verification_id"].as_str().unwrap(),
            "--approve-sha256",
            plan["approve_sha256"].as_str().unwrap(),
            "--acknowledge-task-execution",
        ],
    )
}
#[test]
fn pinned_task_and_content_checks_produce_verified_state() {
    let lab = Lab::new("normal");
    lab.register_task();
    let (id, d) = lab.prepare();
    lab.baseline(&id);
    data(lab.apply(&id, &d));
    let p = lab.verify_plan(&id);
    assert_eq!(p["paths_allowed"], true);
    let r = data(execute_verification(&lab, &id, &p));
    assert_eq!(r["passed"], true);
    assert!(!r.to_string().contains("SECRET_CANARY"));
    assert_eq!(lab.status(&id)["state"], "verified");
    assert!(!execute_verification(&lab, &id, &p).status.success());
    let roster = data(invoke(&lab.home, &["roster", "--project", "p"]));
    assert_eq!(roster["roster"][0]["role"], "primary");
}
#[test]
fn changed_protected_test_file_cannot_pass_verification() {
    let lab = Lab::new("forbidden");
    lab.register_task();
    let (id, d) = lab.prepare();
    lab.baseline(&id);
    data(lab.apply(&id, &d));
    let p = lab.verify_plan(&id);
    assert_eq!(p["paths_allowed"], false);
    let r = data(execute_verification(&lab, &id, &p));
    assert_eq!(r["passed"], false);
    assert_eq!(r["checks"], json!([]));
    assert_eq!(lab.status(&id)["state"], "verification_failed");
}
#[test]
fn same_size_restored_mtime_invalidates_verification_approval() {
    let lab = Lab::new("normal");
    lab.register_task();
    let (id, d) = lab.prepare();
    lab.baseline(&id);
    data(lab.apply(&id, &d));
    let p = lab.verify_plan(&id);
    let path = lab.repo.join("result.txt");
    let metadata = fs::metadata(&path).unwrap();
    let old = fs::read(&path).unwrap();
    fs::write(&path, vec![b'X'; old.len()]).unwrap();
    fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_times(fs::FileTimes::new().set_modified(metadata.modified().unwrap()))
        .unwrap();
    let o = execute_verification(&lab, &id, &p);
    assert!(!o.status.success());
    assert!(
        String::from_utf8_lossy(&o.stdout).contains("VERIFICATION_CONTENT_CHANGED_SINCE_APPROVAL")
    );
}
#[test]
fn no_pre_run_baseline_cannot_be_retroactively_invented() {
    let lab = Lab::new("normal");
    lab.register_task();
    let (id, d) = lab.prepare();
    data(lab.apply(&id, &d));
    assert!(!invoke(
        &lab.home,
        &[
            "effect",
            "runtime",
            "baseline",
            "--id",
            &id,
            "--acknowledge-content-hashing"
        ]
    )
    .status
    .success());
}
#[test]
fn changed_task_executable_invalidates_verification() {
    let lab = Lab::new("normal");
    lab.register_task();
    let (id, d) = lab.prepare();
    lab.baseline(&id);
    data(lab.apply(&id, &d));
    let p = lab.verify_plan(&id);
    fs::write(&lab.agent, b"changed-executable-not-run").unwrap();
    assert!(!execute_verification(&lab, &id, &p).status.success());
}
#[test]
fn cached_watch_is_bounded_and_does_not_prompt() {
    let lab = Lab::new("normal");
    let (id, d) = lab.prepare();
    data(lab.apply(&id, &d));
    let o = invoke(
        &lab.home,
        &["effect", "runtime", "watch", "--id", &id, "--seconds", "1"],
    );
    assert!(o.status.success());
    assert!(o.stdout.len() < 32768);
    assert!(!String::from_utf8_lossy(&o.stdout).contains("SECRET_CANARY"));
    assert_eq!(lab.count(), 1);
}
