//! Exact Git workspace observations and approved lifecycle operations; no force or global prune.
use crate::{
    control_store::{epoch, sha},
    control_workspace::{self, command},
    storage::Store,
    Error, Result,
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    path::{Path, PathBuf},
    time::Duration,
};
use workstation_core::{
    integrations::Adapter, operations::*, workspace_policy::*, Coverage, Project,
};
/// Called in the contained collector. It does not run hooks or mutate Git state.
pub fn inspect(project: &Project, workspace: &str, path: &Path) -> Result<WorkspaceDetail> {
    let stamp = control_workspace::capture(project.clone(), workspace.into(), path.to_path_buf())?;
    let raw = command(
        project,
        path,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
        false,
    )?;
    let common = PathBuf::from(
        std::str::from_utf8(&raw)
            .map_err(|_| Error::new("GIT_PATH_ENCODING"))?
            .trim(),
    );
    crate::paths::directory(&common)?;
    let repository_identity = control_workspace::directory_identity(&common)?;
    let output = command(
        project,
        path,
        &["worktree", "list", "--porcelain", "-z"],
        false,
    )?;
    let items = crate::git::parse_worktrees(&output)?;
    // Main worktree is first in Git's authoritative list; never infer from branch name.
    let normalized = |s: &str| {
        let p = s.replace('\\', "/");
        if cfg!(windows) {
            p.to_ascii_lowercase()
        } else {
            p
        }
    };
    let own = normalized(&path.to_string_lossy());
    let item = items
        .iter()
        .find(|w| normalized(&w.path) == own)
        .ok_or(Error::new("WORKTREE_REGISTRATION_MISSING"))?;
    let mut changed = vec![];
    let mut ignored = vec![];
    let mut coverage = Coverage::Partial;
    if stamp.coverage == Coverage::Complete {
        let bytes = command(
            project,
            path,
            &[
                "status",
                "--porcelain=v1",
                "-z",
                "--untracked-files=all",
                "--ignored=matching",
                "--ignore-submodules=all",
            ],
            false,
        )?;
        let fields = bytes.split(|b| *b == 0).collect::<Vec<_>>();
        let mut i = 0;
        while i < fields.len() {
            let f = fields[i];
            i += 1;
            if f.is_empty() {
                continue;
            }
            if f.len() < 4 || f[2] != b' ' {
                return Err(Error::new("STATUS_FORMAT"));
            }
            let p = std::str::from_utf8(&f[3..]).map_err(|_| Error::new("GIT_PATH_ENCODING"))?;
            let p = p.strip_suffix('/').unwrap_or(p);
            relative_path(p).map_err(Error::new)?;
            if f.starts_with(b"!!") {
                ignored.push(p.into());
            } else {
                changed.push(p.into());
            }
            if f[0] == b'R' || f[1] == b'R' || f[0] == b'C' || f[1] == b'C' {
                let previous = fields.get(i).ok_or(Error::new("STATUS_RENAME_TRUNCATED"))?;
                i += 1;
                let old =
                    std::str::from_utf8(previous).map_err(|_| Error::new("GIT_PATH_ENCODING"))?;
                relative_path(old).map_err(Error::new)?;
                changed.push(old.into());
            }
            if changed.len() + ignored.len() > 512 {
                return Err(Error::new("WORKSPACE_PATH_BUDGET"));
            }
        }
        coverage = Coverage::Complete;
    }
    let checkout_filters = command(
        project,
        path,
        &["config", "--name-only", "--get-regexp", r"^filter\."],
        true,
    )?;
    Ok(WorkspaceDetail {
        checkout_filters_present: Some(!checkout_filters.is_empty()),
        stamp,
        repository_identity,
        main_worktree: items.first().is_some_and(|w| normalized(&w.path) == own),
        bare: item.bare,
        locked: item.locked,
        changed_paths: changed,
        ignored_paths: ignored,
        path_coverage: coverage,
        branch_exists: Some(item.branch.is_some()),
    })
}
pub fn collect(store: &Store, project: &str, workspace: &str) -> Result<WorkspaceDetail> {
    let p = store
        .projects()?
        .into_iter()
        .find(|p| p.id == project)
        .ok_or(Error::new("PROJECT_MISSING"))?;
    let request = workstation_core::WorkerRequest::WorkspaceDetail {
        project: p,
        workspace_id: workspace.into(),
        path: store.workspace_path(project, workspace)?.into(),
    };
    let exe = std::env::current_exe().map_err(|_| Error::new("EXECUTABLE_PATH"))?;
    let out = crate::runner::collect(&exe, &store.home, &request, Duration::from_secs(5))?;
    if out.exit_code != 0 {
        return Err(Error::new("WORKSPACE_DETAIL_FAILED"));
    }
    serde_json::from_slice(&out.bytes).map_err(|_| Error::new("WORKSPACE_DETAIL_SHAPE"))
}
/// Shell-free Git execution for approved actions. No prompts, fetch, hooks, fsmonitor or auto-GC.
fn git_effect(store: &Store, p: &Project, git_pin: &str, args: &[String]) -> Result<Value> {
    let mut env = crate::external::scoped_environment(&store.home, &[], &BTreeMap::new())?;
    let empty = store.home.join("git-empty");
    if !empty.exists() {
        std::fs::create_dir(&empty).map_err(|_| Error::new("GIT_EMPTY_CREATE"))?;
    }
    crate::paths::directory(&empty)?;
    #[cfg(windows)]
    let null = "NUL";
    #[cfg(not(windows))]
    let null = "/dev/null";
    for (k, v) in [
        ("GIT_CONFIG_NOSYSTEM", "1"),
        ("GIT_CONFIG_GLOBAL", null),
        ("GIT_CONFIG_SYSTEM", null),
        ("GIT_TERMINAL_PROMPT", "0"),
        ("GIT_OPTIONAL_LOCKS", "0"),
        ("GIT_NO_LAZY_FETCH", "1"),
        ("GIT_LFS_SKIP_SMUDGE", "1"),
    ] {
        env.insert(k.into(), v.into());
    }
    let mut a: Vec<OsString> = [
        "--no-pager",
        "--no-optional-locks",
        "-c",
        "core.hooksPath=",
        "-c",
        "core.fsmonitor=false",
        "-c",
        "gc.auto=0",
        "-c",
        "maintenance.auto=false",
        "-c",
        "protocol.allow=never",
        "-C",
    ]
    .iter()
    .map(|s| OsString::from(*s))
    .collect();
    a.push(p.path.as_os_str().into());
    a.extend(args.iter().map(OsString::from));
    let spec = crate::external::Launch {
        executable: p.git_executable.clone(),
        expected_sha256: git_pin.into(),
        cwd: p.path.clone(),
        args: a,
        env,
        timeout: Duration::from_secs(30),
        output_limit: 262144,
    };
    let out = crate::external::one_shot(spec, None)?;
    if out.exit_code != 0 {
        return Err(Error::new("GIT_LIFECYCLE_NONZERO_REVIEW_NO_RETRY"));
    }
    Ok(json!({"exit_code":0,"stdout":"discarded","stderr":"discarded"}))
}
pub fn prepare_cleanup(
    store: &Store,
    project: &str,
    workspace: &str,
    ack: bool,
    ref_id: &str,
) -> Result<CleanupTarget> {
    let detail = collect(store, project, workspace)?;
    let context = store.cleanup_context(detail.clone(), ack)?;
    let verdict = cleanup_eligibility(&context, epoch()).map_err(Error::new)?;
    if verdict.disposition != "eligible_for_exact_reviewed_plan" {
        return Err(Error::new("WORKSPACE_PROTECTED_SEE_ELIGIBILITY"));
    }
    let git_pin = store
        .projects()?
        .into_iter()
        .find(|p| p.id == project)
        .ok_or(Error::new("PROJECT_MISSING"))?
        .git_executable;
    let target = CleanupTarget {
        git_sha256: crate::external::hash_file(&git_pin)?,
        project_id: project.into(),
        workspace_id: workspace.into(),
        path: detail.stamp.path.clone(),
        directory_identity: detail.stamp.directory_identity.clone(),
        head: detail.stamp.head.clone(),
        branch: detail.stamp.branch.clone(),
        repository_identity: detail.repository_identity.clone(),
        state_digest: sha(&serde_json::to_vec(&state_projection(&detail))
            .map_err(|_| Error::new("STATE_ENCODING"))?),
        preservation_ref: format!("refs/workstation/retired/{ref_id}"),
        quiescence_acknowledged: ack,
    };
    target.validate().map_err(Error::new)?;
    Ok(target)
}
pub fn remove(store: &mut Store, run: &str, t: &CleanupTarget) -> Result<Value> {
    t.validate().map_err(Error::new)?;
    let detail = collect(store, &t.project_id, &t.workspace_id)?;
    let approved_status = detail.stamp.status_digest.clone();
    let d = sha(&serde_json::to_vec(&state_projection(&detail))
        .map_err(|_| Error::new("STATE_ENCODING"))?);
    if d != t.state_digest {
        return Err(Error::new("WORKSPACE_CHANGED_SINCE_PREVIEW"));
    }
    let eligibility =
        cleanup_eligibility(&store.cleanup_context(detail, true)?, epoch()).map_err(Error::new)?;
    if eligibility.disposition != "eligible_for_exact_reviewed_plan" {
        return Err(Error::new("WORKSPACE_NEW_PROTECTION"));
    }
    let p = store
        .projects()?
        .into_iter()
        .find(|p| p.id == t.project_id)
        .ok_or(Error::new("PROJECT_MISSING"))?;
    // Ref is explicit in the reviewed plan; zero-old means no overwrite of an existing ref.
    store.effect_step(
        run,
        "preserve_head",
        "intent",
        &json!({"ref":t.preservation_ref,"head":t.head}),
    )?;
    git_effect(
        store,
        &p,
        &t.git_sha256,
        &[
            "update-ref".into(),
            t.preservation_ref.clone(),
            t.head.clone(),
            "0".repeat(t.head.len()),
        ],
    )?;
    store.effect_step(
        run,
        "preserve_head",
        "completed",
        &json!({"ref":t.preservation_ref}),
    )?;
    // The preservation ref changes reachability but not HEAD or content. Capture again and check
    // all data blockers. No `--force`, branch deletion, broad prune, or idle/age-based approval.
    let last = collect(store, &t.project_id, &t.workspace_id)?;
    if last.stamp.head != t.head
        || last.stamp.directory_identity != t.directory_identity
        || last.stamp.branch != t.branch
        || last.repository_identity != t.repository_identity
        || last.stamp.path != t.path
        || last.stamp.status_digest != approved_status
    {
        return Err(Error::new("WORKSPACE_CHANGED_BEFORE_REMOVE"));
    }
    if cleanup_eligibility(&store.cleanup_context(last, true)?, epoch())
        .map_err(Error::new)?
        .disposition
        != "eligible_for_exact_reviewed_plan"
    {
        return Err(Error::new("WORKSPACE_PROTECTED_BEFORE_REMOVE"));
    }
    store.effect_step(run,"git_worktree_remove_exact","intent",&json!({"path":t.path,"no_force":true,"quiescence":"explicit_user_declaration_not_os_lock"}))?;
    git_effect(
        store,
        &p,
        &t.git_sha256,
        &[
            "worktree".into(),
            "remove".into(),
            "--".into(),
            t.path.clone(),
        ],
    )?;
    require_missing_entry(Path::new(&t.path))
        .map_err(|_| Error::new("WORKTREE_PATH_REMAINS_OR_UNREADABLE_REVIEW"))?;
    let receipt = json!({"workspace_id":t.workspace_id,"removed_path":t.path,"head_preserved_at":t.preservation_ref,"branch_deleted":false,"data_recovery":"HEAD_ref_only_no_uncommitted_data_allowed","race_boundary":"external_writers_must_remain_quiescent_not_an_OS_exclusive_lock"});
    store.record_retired(run, t, &receipt)?;
    store.effect_step(run, "git_worktree_remove_exact", "completed", &receipt)?;
    Ok(receipt)
}
#[expect(
    clippy::too_many_arguments,
    reason = "workspace creation binds exact repository, branch, base, executable pin and destination identity"
)]
pub fn create(
    store: &Store,
    project: &str,
    integration: Option<&workstation_core::integrations::Integration>,
    destination: &str,
    branch: &str,
    base: &str,
    git_pin: &str,
    preflight: &WorkspaceDetail,
    destination_parent_identity: &str,
) -> Result<Value> {
    branch_name(branch).map_err(Error::new)?;
    let path = Path::new(destination);
    require_missing_entry(path)?;
    let parent = path
        .parent()
        .ok_or(Error::new("DESTINATION_PARENT_REQUIRED"))?;
    crate::paths::directory(parent)?;
    if crate::control_workspace::directory_identity(parent)? != destination_parent_identity {
        return Err(Error::new("DESTINATION_PARENT_CHANGED"));
    }
    let mut p = store
        .projects()?
        .into_iter()
        .find(|p| p.id == project)
        .ok_or(Error::new("PROJECT_MISSING"))?;
    let primary_checkout = p.path.clone();
    p.path = PathBuf::from(&preflight.stamp.path);
    if crate::paths::overlaps(&store.home, path)
        || crate::paths::overlaps(&p.path, path)
        || crate::paths::overlaps(&primary_checkout, path)
    {
        return Err(Error::new("WORKTREE_DESTINATION_OVERLAP"));
    }
    if crate::external::hash_file(&p.git_executable)? != git_pin {
        return Err(Error::new("GIT_EXECUTABLE_CHANGED"));
    }
    // Checkout can run Git filters even when hooks are off. Reject configured filters.
    if preflight.checkout_filters_present != Some(false)
        || preflight.stamp.coverage != Coverage::Complete
        || epoch() - preflight.stamp.observed_at > 30
        || preflight.stamp.observed_at > epoch()
    {
        return Err(Error::new(
            "WORKTREE_CREATE_FRESH_FILTER_PREFLIGHT_REQUIRED",
        ));
    }
    if p.path.join(".gitmodules").exists() {
        return Err(Error::new("WORKTREE_CREATE_SUBMODULE_POLICY_REQUIRED"));
    }

    if let Some(i) = integration {
        if i.adapter != Adapter::Worktrunk {
            return Err(Error::new("WORKTRUNK_PROFILE_REQUIRED"));
        }
        // Exact template override, empty owned user config, --no-hooks and --no-cd.
        // No copy-ignored, --execute, clobber, PR lookup, --full or background removal.
        let cfg = crate::specialists::empty_worktrunk_config(store)?;
        let template = destination.replace('\\', "/");
        if template
            .chars()
            .any(|c| ['\'', '"', '{', '}', '\n', '\r'].contains(&c))
        {
            return Err(Error::new("WORKTRUNK_TEMPLATE_UNSAFE"));
        }
        let mut env = crate::tasks::environment(store, i)?;
        let git_parent = p
            .git_executable
            .parent()
            .ok_or(Error::new("GIT_PARENT_MISSING"))?;
        env.insert("PATH".into(), git_parent.as_os_str().into());
        env.insert("WORKTRUNK_WORKTREE_PATH".into(), template.clone().into());
        env.insert("GIT_NO_LAZY_FETCH".into(), "1".into());
        env.insert("GIT_TERMINAL_PROMPT".into(), "0".into());
        #[cfg(windows)]
        let null = "NUL";
        #[cfg(not(windows))]
        let null = "/dev/null";
        for (k, v) in [
            ("GIT_CONFIG_NOSYSTEM", "1"),
            ("GIT_CONFIG_GLOBAL", null),
            ("GIT_CONFIG_SYSTEM", null),
            ("GIT_CONFIG_COUNT", "3"),
            ("GIT_CONFIG_KEY_0", "core.hooksPath"),
            ("GIT_CONFIG_VALUE_0", ""),
            ("GIT_CONFIG_KEY_1", "protocol.allow"),
            ("GIT_CONFIG_VALUE_1", "never"),
            ("GIT_CONFIG_KEY_2", "core.fsmonitor"),
            ("GIT_CONFIG_VALUE_2", "false"),
        ] {
            env.insert(k.into(), v.into());
        }
        let args = vec![
            "--config".into(),
            cfg.as_os_str().into(),
            "--config-set".into(),
            format!("worktree-path = '{template}'").into(),
            "switch".into(),
            "--create".into(),
            branch.into(),
            "--base".into(),
            base.into(),
            "--no-hooks".into(),
            "--no-cd".into(),
            "--format=json".into(),
        ];
        let out = crate::external::one_shot(
            crate::external::Launch {
                executable: i.executable.clone().into(),
                expected_sha256: i.executable_sha256.clone(),
                cwd: p.path.clone(),
                args,
                env,
                timeout: Duration::from_secs(30),
                output_limit: 262144,
            },
            None,
        )?;
        if out.exit_code != 0 {
            return Err(Error::new("WORKTRUNK_CREATE_FAILED_REVIEW_PARTIAL"));
        }
    } else {
        git_effect(
            store,
            &p,
            git_pin,
            &[
                "worktree".into(),
                "add".into(),
                "-b".into(),
                branch.into(),
                "--".into(),
                destination.into(),
                base.into(),
            ],
        )?;
    }
    // Creation may run repository filters. This is an approved write operation, not a sandbox.
    crate::paths::directory(path)?;
    p.path = primary_checkout;
    let req = workstation_core::WorkerRequest::WorkspaceDetail {
        project: p,
        workspace_id: "created-unregistered".into(),
        path: path.to_path_buf(),
    };
    let executable = std::env::current_exe().map_err(|_| Error::new("EXECUTABLE_PATH"))?;
    let observed = crate::runner::collect(&executable, &store.home, &req, Duration::from_secs(5))?;
    if observed.exit_code != 0 {
        return Err(Error::new(
            "CREATED_WORKTREE_VERIFICATION_FAILED_NO_AUTODELETE",
        ));
    }
    let verified: WorkspaceDetail = serde_json::from_slice(&observed.bytes)
        .map_err(|_| Error::new("CREATED_WORKTREE_SHAPE"))?;
    let branch_ref = format!("refs/heads/{branch}");
    if verified.stamp.head != base
        || verified.stamp.branch.as_deref() != Some(branch_ref.as_str())
        || verified.repository_identity != preflight.repository_identity
        || verified.main_worktree
        || verified.stamp.coverage != Coverage::Complete
    {
        return Err(Error::new("CREATED_WORKTREE_POSTCONDITION_MISMATCH"));
    }
    Ok(
        json!({"path":destination,"branch":branch,"base_commit":base,"directory_identity":verified.stamp.directory_identity,"registered_in_workstation":false,"next":"explicitly_register_new_workspace","implementation":"not_native_certified"}),
    )
}

/// Only an actual NotFound is an unused entry. Broken symlinks and access errors block.
pub fn require_missing_entry(path: &Path) -> Result<()> {
    match std::fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Ok(_) => Err(Error::new("DESTINATION_ENTRY_ALREADY_EXISTS")),
        Err(_) => Err(Error::new("DESTINATION_ENTRY_UNREADABLE")),
    }
}
