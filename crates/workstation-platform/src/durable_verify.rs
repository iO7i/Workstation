//! Explicit content verification. Hashes are evidence, not permission to delete files.
use crate::{
    control_store::{epoch, sha},
    storage::Store,
    Error, Result,
};
use rusqlite::{params, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::File,
    io::Read,
    path::Path,
    time::{Duration, Instant},
};
use workstation_core::{durable::*, integrations::ApprovedTask};
fn db(_: rusqlite::Error) -> Error {
    Error::new("VERIFICATION_DATABASE_CONFLICT")
}
fn encoded<T: Serialize>(v: &T) -> Result<Vec<u8>> {
    serde_json::to_vec(v).map_err(|_| Error::new("VERIFICATION_ENCODING"))
}
fn git_paths(store: &Store, r: &RunRecord, ignored: bool) -> Result<Vec<String>> {
    let project = store
        .projects()?
        .into_iter()
        .find(|p| p.id == r.project_id)
        .ok_or(Error::new("PROJECT_MISSING"))?;
    let path = store.workspace_path(&r.project_id, &r.workspace_id)?;
    let mut args: Vec<std::ffi::OsString> = [
        "--no-pager",
        "--no-optional-locks",
        "-c",
        "core.fsmonitor=false",
        "-c",
        "core.hooksPath=",
        "-C",
        &path,
        "ls-files",
        "-z",
        "--others",
        "--exclude-standard",
    ]
    .iter()
    .map(|s| std::ffi::OsString::from(*s))
    .collect();
    args.push(if ignored { "--ignored" } else { "--cached" }.into());
    let mut env = crate::external::scoped_environment(&store.home, &[], &BTreeMap::new())?;
    env.insert("GIT_CONFIG_NOSYSTEM".into(), "1".into());
    env.insert("GIT_TERMINAL_PROMPT".into(), "0".into());
    #[cfg(windows)]
    let null = "NUL";
    #[cfg(not(windows))]
    let null = "/dev/null";
    env.insert("GIT_CONFIG_GLOBAL".into(), null.into());
    env.insert("GIT_OPTIONAL_LOCKS".into(), "0".into());
    let output = crate::external::one_shot(
        crate::external::Launch {
            executable: project.git_executable.clone(),
            expected_sha256: crate::external::hash_file(&project.git_executable)?,
            cwd: store.home.clone(),
            args,
            env,
            timeout: Duration::from_secs(5),
            output_limit: 1024 * 1024,
        },
        None,
    )?;
    if output.exit_code != 0 {
        return Err(Error::new("VERIFICATION_GIT_FAILED"));
    }
    let b = &output.bytes.0;
    if !b.is_empty() && b.last() != Some(&0) {
        return Err(Error::new("VERIFICATION_PATH_STREAM_TRUNCATED"));
    }
    let mut paths: Vec<String> = vec![];
    for raw in b.split(|b| *b == 0).filter(|s| !s.is_empty()) {
        if paths.len() >= 4096 {
            return Err(Error::new("VERIFICATION_FILE_BUDGET"));
        }
        let p = std::str::from_utf8(raw).map_err(|_| Error::new("VERIFICATION_PATH_ENCODING"))?;
        relative(p).map_err(Error::new)?;
        paths.push(p.into());
    }
    paths.sort();
    paths.dedup();
    let folded: std::collections::BTreeSet<_> = paths.iter().map(|s| s.to_lowercase()).collect();
    if folded.len() != paths.len() {
        return Err(Error::new("VERIFICATION_PATH_CASE_COLLISION"));
    }
    Ok(paths)
}
pub fn content_manifest(store: &Store, id: &str) -> Result<ContentManifest> {
    let r = store.durable_run(id)?;
    if !git_paths(store, &r, true)?.is_empty() {
        return Err(Error::new("VERIFICATION_IGNORED_CONTENT_UNSUPPORTED"));
    }
    let names = git_paths(store, &r, false)?;
    if names.iter().any(|s| s == ".gitmodules") {
        return Err(Error::new("VERIFICATION_SUBMODULES_UNSUPPORTED"));
    }
    let root = store.workspace_path(&r.project_id, &r.workspace_id)?;
    let root = Path::new(&root);
    let identity = crate::control_workspace::directory_identity(root)?;
    let before = crate::engine::capture(store, &r.project_id, &r.workspace_id)?;
    if before.coverage != workstation_core::Coverage::Complete {
        return Err(Error::new("VERIFICATION_WORKSPACE_PARTIAL"));
    }
    let start = Instant::now();
    let mut files = BTreeMap::new();
    let mut bytes = 0u64;
    for p in &names {
        if start.elapsed() > Duration::from_secs(15) {
            return Err(Error::new("VERIFICATION_HASH_TIMEOUT"));
        }
        let file = root.join(p);
        if !crate::paths::entry_exists(&file)? {
            continue;
        } // tracked deletion remains visible by missing manifest key
        crate::paths::regular_file(&file)?;
        let mut input =
            File::open(&file).map_err(|_| Error::new("VERIFICATION_FILE_UNREADABLE"))?;
        let initial = input
            .metadata()
            .map_err(|_| Error::new("VERIFICATION_METADATA_FAILED"))?;
        if initial.len() > 16 * 1024 * 1024 {
            return Err(Error::new("VERIFICATION_FILE_TOO_LARGE"));
        }
        let fid = crate::external::path_identity(&file)?;
        let mut h = Sha256::new();
        let mut b = [0u8; 65536];
        let mut size = 0u64;
        loop {
            let n = input
                .read(&mut b)
                .map_err(|_| Error::new("VERIFICATION_FILE_UNREADABLE"))?;
            if n == 0 {
                break;
            }
            size += n as u64;
            bytes += n as u64;
            if size > 16 * 1024 * 1024
                || bytes > 64 * 1024 * 1024
                || start.elapsed() > Duration::from_secs(15)
            {
                return Err(Error::new("VERIFICATION_HASH_BUDGET"));
            }
            h.update(&b[..n]);
        }
        let final_m = input
            .metadata()
            .map_err(|_| Error::new("VERIFICATION_METADATA_FAILED"))?;
        if size != initial.len()
            || final_m.len() != initial.len()
            || initial.modified().ok() != final_m.modified().ok()
            || crate::external::path_identity(&file)? != fid
        {
            return Err(Error::new("VERIFICATION_FILE_CHANGED_DURING_HASH"));
        }
        files.insert(p.clone(), format!("{:x}", h.finalize()));
    }
    let after = crate::engine::capture(store, &r.project_id, &r.workspace_id)?;
    if before.head != after.head
        || before.branch != after.branch
        || before.status_digest != after.status_digest
        || after.coverage != workstation_core::Coverage::Complete
        || git_paths(store, &r, false)? != names
    {
        return Err(Error::new("VERIFICATION_WORKSPACE_CHANGED_DURING_HASH"));
    }
    Ok(ContentManifest {
        directory_identity: identity,
        files,
        bytes,
    })
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Plan {
    run_id: String,
    run_revision: u64,
    work_version: u64,
    workspace: workstation_core::control::WorkspaceStamp,
    baseline_digest: String,
    observed_manifest: ContentManifest,
    contract: VerificationContract,
    tasks: Vec<ApprovedTask>,
}
pub fn prepare(store: &mut Store, id: &str, contract: VerificationContract) -> Result<Value> {
    contract.validate().map_err(Error::new)?;
    let r = store.durable_run(id)?;
    if !r.owner_released || crate::durable_runtime::supervisor_alive(store, &r)? {
        return Err(Error::new("VERIFICATION_SUPERVISOR_STILL_ACTIVE"));
    }
    let baseline = store
        .durable_baseline(id)?
        .ok_or(Error::new("VERIFICATION_PRE_RUN_BASELINE_REQUIRED"))?;
    let manifest = content_manifest(store, id)?;
    let changes = changed_paths(&baseline, &manifest).map_err(Error::new)?;
    let workspace = crate::engine::capture(store, &r.project_id, &r.workspace_id)?;
    let mut tasks = vec![];
    for task_id in &contract.task_ids {
        let task = store.task(task_id)?;
        if task.project_id != r.project_id
            || task.environment != r.environment
            || Path::new(&task.cwd) != Path::new(&workspace.path)
            || !task.secret_bindings.is_empty()
        {
            return Err(Error::new("VERIFICATION_TASK_SCOPE_OR_SECRETS"));
        }
        crate::tasks::validate_files(&task)?;
        tasks.push(task);
    }
    let plan = Plan {
        run_id: id.into(),
        run_revision: r.revision,
        work_version: store.work(&r.work_id)?.version,
        workspace,
        baseline_digest: sha(&encoded(&baseline)?),
        observed_manifest: manifest,
        contract,
        tasks,
    };
    let payload = encoded(&plan)?;
    let digest = sha(&payload);
    let vid = crate::new_id();
    let count: i64 = store
        .conn
        .query_row(
            "SELECT count(*) FROM external_run_verifications WHERE run_id=?1",
            [id],
            |r| r.get(0),
        )
        .map_err(db)?;
    if count >= 32 {
        return Err(Error::new("VERIFICATION_PLAN_BUDGET"));
    }
    store
        .conn
        .execute(
            "INSERT INTO external_run_verifications VALUES(?1,?2,?3,?4,?5,?6,NULL,NULL)",
            params![
                vid,
                id,
                epoch(),
                epoch() + 600,
                digest,
                String::from_utf8(payload).map_err(|_| Error::new("ENCODING_FAILED"))?
            ],
        )
        .map_err(db)?;
    Ok(
        json!({"verification_id":vid,"approve_sha256":digest,"expires_at":epoch()+600,"changed_paths":changes,"paths_allowed":permitted_changes(&changes,&plan.contract),"task_ids":plan.contract.task_ids,"tests_executed":false,"no_secrets":true}),
    )
}
pub fn execute(store: &mut Store, run_id: &str, vid: &str, approval: &str) -> Result<Value> {
    let(d,p,expires,executed):(String,String,i64,Option<i64>)=store.conn.query_row("SELECT digest,payload,expires_at,executed_at FROM external_run_verifications WHERE id=?1",[vid],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).map_err(db)?;
    if sha(p.as_bytes()) != d || d != approval || epoch() >= expires || executed.is_some() {
        return Err(Error::new("VERIFICATION_APPROVAL_STALE_OR_USED"));
    }
    let plan: Plan =
        serde_json::from_str(&p).map_err(|_| Error::new("VERIFICATION_PLAN_CORRUPT"))?;
    if plan.run_id != run_id {
        return Err(Error::new("VERIFICATION_RUN_SCOPE_MISMATCH"));
    }
    let r = store.durable_run(&plan.run_id)?;
    if r.revision != plan.run_revision
        || !r.owner_released
        || crate::durable_runtime::supervisor_alive(store, &r)?
        || store.work(&r.work_id)?.version != plan.work_version
    {
        return Err(Error::new("VERIFICATION_RUN_CHANGED"));
    }
    let baseline = store
        .durable_baseline(&r.id)?
        .ok_or(Error::new("VERIFICATION_BASELINE_MISSING"))?;
    if sha(&encoded(&baseline)?) != plan.baseline_digest
        || content_manifest(store, &r.id)? != plan.observed_manifest
    {
        return Err(Error::new("VERIFICATION_CONTENT_CHANGED_SINCE_APPROVAL"));
    }
    for task in &plan.tasks {
        if encoded(&store.task(&task.id)?)? != encoded(task)? {
            return Err(Error::new("VERIFICATION_TASK_CHANGED"));
        }
        crate::tasks::validate_files(task)?;
    }
    let lock_path = store.home.join("effects.lock");
    if crate::paths::entry_exists(&lock_path)? {
        crate::paths::local_existing(&lock_path)?;
    }
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(lock_path)
        .map_err(|_| Error::new("EFFECT_LOCK_OPEN"))?;
    lock.try_lock()
        .map_err(|_| Error::new("ANOTHER_EFFECT_RUNNING"))?;
    let tx = store
        .conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(db)?;
    let revision: i64 = tx
        .query_row(
            "SELECT revision FROM external_runs WHERE id=?1",
            [&plan.run_id],
            |r| r.get(0),
        )
        .map_err(db)?;
    if u64::try_from(revision).ok() != Some(plan.run_revision) {
        return Err(Error::new("VERIFICATION_RUN_CHANGED"));
    }
    if tx.execute("UPDATE external_run_verifications SET executed_at=?1 WHERE id=?2 AND executed_at IS NULL AND expires_at>?1",params![epoch(),vid]).map_err(db)?!=1{return Err(Error::new("VERIFICATION_ALREADY_EXECUTED"));}
    tx.commit().map_err(db)?;
    let changes = changed_paths(&baseline, &plan.observed_manifest).map_err(Error::new)?;
    let allowed = permitted_changes(&changes, &plan.contract);
    let mut checks = vec![];
    let mut pass = allowed;
    if allowed {
        for task in &plan.tasks {
            let result = crate::tasks::execute(store, task);
            checks.push(json!({"task_id":task.id,"passed":result.is_ok(),"error_code":result.as_ref().err().map(|e|e.code)}));
            if result.is_err() {
                pass = false;
                break;
            }
        }
    }
    // Verification tasks may not alter even an allowed application file. A fresh contract is needed afterward.
    let unchanged = content_manifest(store, &r.id)
        .map(|m| m == plan.observed_manifest)
        .unwrap_or(false);
    pass &= unchanged;
    let result = json!({"verification_id":vid,"run_id":r.id,"passed":pass,"allowed_path_changes":allowed,"checks":checks,"workspace_unchanged_during_tests":unchanged,"work_item_marked_done":false,"assignment_released":false,"provider_completed":r.provider==ProviderState::Completed,"stdout_retained":false,"stderr_retained":false});
    store
        .conn
        .execute(
            "UPDATE external_run_verifications SET receipt=?1 WHERE id=?2 AND receipt IS NULL",
            params![
                String::from_utf8(encoded(&result)?).map_err(|_| Error::new("ENCODING_FAILED"))?,
                vid
            ],
        )
        .map_err(db)?;
    store.durable_event(
        &r.id,
        RunEvent::Verification {
            state: if pass {
                VerificationState::Passed
            } else {
                VerificationState::Failed
            },
        },
    )?;
    Ok(result)
}
