//! v4 operation preparation and dispatch. Only the common effect engine invokes execute.
use crate::{
    control_store::{epoch, sha},
    storage::Store,
    Error, Result,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;
use workstation_core::{
    effects::EffectPlan,
    integrations::{Adapter, Integration},
    operations::*,
};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    WorkspaceCleanup {
        workspace_id: String,
        quiescence_acknowledged: bool,
    },
    WorktreeCreate {
        source_workspace_id: String,
        destination: String,
        branch: String,
        integration_id: Option<String>,
    },
    SecretRotate {
        resource_id: String,
    },
    SecretRevoke {
        resource_id: String,
    },
    JournalArchive {
        before: i64,
        limit: u32,
    },
}
fn pin<T: Serialize>(x: &T) -> Result<String> {
    Ok(sha(
        &serde_json::to_vec(x).map_err(|_| Error::new("PIN_ENCODING"))?
    ))
}
fn local_secret_reference(store: &Store, project: &str, environment: &str, id: &str) -> Result<()> {
    let r = store
        .resources(project, environment)?
        .into_iter()
        .find(|r| r.id == id)
        .ok_or(Error::new("RESOURCE_SCOPE_MISMATCH"))?;
    if r.locator != format!("local://{project}/{environment}/{id}")
        || r.kind != workstation_core::control::ResourceKind::SecretReference
    {
        return Err(Error::new("LOCAL_SECRET_REFERENCE_REQUIRED"));
    }
    Ok(())
}
pub fn prepare(
    store: &Store,
    project: &str,
    environment: &str,
    request: Request,
) -> Result<Operation> {
    store.require_v4()?;
    store.require_environment(project, environment)?;
    let op = match request {
        Request::WorkspaceCleanup {
            workspace_id,
            quiescence_acknowledged,
        } => Operation::WorkspaceCleanup {
            target: crate::workspace_lifecycle::prepare_cleanup(
                store,
                project,
                &workspace_id,
                quiescence_acknowledged,
                &crate::new_id(),
            )?,
        },
        Request::WorktreeCreate {
            source_workspace_id,
            destination,
            branch,
            integration_id,
        } => {
            workstation_core::integrations::absolute_path(&destination).map_err(Error::new)?;
            branch_name(&branch).map_err(Error::new)?;
            crate::workspace_lifecycle::require_missing_entry(Path::new(&destination))?;
            let parent = Path::new(&destination)
                .parent()
                .ok_or(Error::new("DESTINATION_PARENT_REQUIRED"))?;
            crate::paths::directory(parent)?;
            let destination_parent_identity = crate::control_workspace::directory_identity(parent)?;
            let d = crate::workspace_lifecycle::collect(store, project, &source_workspace_id)?;
            if d.stamp.coverage != workstation_core::Coverage::Complete
                || d.checkout_filters_present != Some(false)
            {
                return Err(Error::new("SOURCE_WORKSPACE_FILTER_OR_INSPECTION_BLOCKER"));
            }
            let p = store
                .projects()?
                .into_iter()
                .find(|p| p.id == project)
                .ok_or(Error::new("PROJECT_MISSING"))?;
            let digest = if let Some(id) = &integration_id {
                let i = store.integration(id)?;
                if i.project_id != project
                    || i.environment != environment
                    || i.adapter != Adapter::Worktrunk
                {
                    return Err(Error::new("WORKTRUNK_SCOPE_MISMATCH"));
                }
                Some(pin(&i)?)
            } else {
                None
            };
            Operation::WorktreeCreate {
                integration_id,
                integration_digest: digest,
                destination,
                branch,
                base_commit: d.stamp.head,
                repository_identity: d.repository_identity,
                source_workspace_id,
                git_sha256: crate::external::hash_file(&p.git_executable)?,
                destination_parent_identity,
            }
        }
        Request::SecretRotate { resource_id } => {
            local_secret_reference(store, project, environment, &resource_id)?;
            Operation::SecretRotate {
                previous_version: store.secret_head(&resource_id)?.map(|x| x.0),
                resource_id,
            }
        }
        Request::SecretRevoke { resource_id } => {
            local_secret_reference(store, project, environment, &resource_id)?;
            let (v, s, _) = store
                .secret_head(&resource_id)?
                .ok_or(Error::new("LEGACY_SECRET_REQUIRES_VERSION_ADOPTION_FIRST"))?;
            if s != "active" {
                return Err(Error::new("SECRET_NOT_ACTIVE"));
            }
            Operation::SecretRevoke {
                resource_id,
                expected_version: v,
            }
        }
        Request::JournalArchive { before, limit } => {
            if before > epoch() - 3600 {
                return Err(Error::new("ARCHIVE_REQUIRES_OLD_TERMINAL_RECORDS"));
            }
            Operation::JournalArchive {
                before,
                limit,
                selection_digest: store.journal_selection_digest(project, before, limit)?,
            }
        }
    };
    op.validate().map_err(Error::new)?;
    Ok(op)
}
pub fn execute(store: &mut Store, run: &str, plan: &EffectPlan, op: &Operation) -> Result<Value> {
    store.require_v4()?;
    op.validate().map_err(Error::new)?;
    match op {
        Operation::WorkspaceCleanup { target } => {
            if target.project_id != plan.project_id {
                return Err(Error::new("CLEANUP_PROJECT_MISMATCH"));
            }
            crate::workspace_lifecycle::remove(store, run, target)
        }
        Operation::WorktreeCreate {
            integration_id,
            integration_digest,
            destination,
            branch,
            base_commit,
            repository_identity,
            source_workspace_id,
            git_sha256,
            destination_parent_identity,
        } => {
            let parent = Path::new(destination)
                .parent()
                .ok_or(Error::new("DESTINATION_PARENT_REQUIRED"))?;
            if crate::control_workspace::directory_identity(parent)? != *destination_parent_identity
            {
                return Err(Error::new("DESTINATION_PARENT_CHANGED"));
            }
            let d =
                crate::workspace_lifecycle::collect(store, &plan.project_id, source_workspace_id)?;
            if &d.repository_identity != repository_identity || &d.stamp.head != base_commit {
                return Err(Error::new("WORKTREE_BASE_CHANGED"));
            }
            let p = store
                .projects()?
                .into_iter()
                .find(|x| x.id == plan.project_id)
                .ok_or(Error::new("PROJECT_MISSING"))?;
            if crate::external::hash_file(&p.git_executable)? != *git_sha256 {
                return Err(Error::new("GIT_EXECUTABLE_CHANGED"));
            }
            let i: Option<Integration> = integration_id
                .as_ref()
                .map(|id| store.integration(id))
                .transpose()?;
            if let Some(i) = &i {
                if i.project_id != plan.project_id
                    || i.environment != plan.environment
                    || Some(pin(i)?) != *integration_digest
                {
                    return Err(Error::new("WORKTRUNK_PROFILE_CHANGED"));
                }
            }
            store.effect_step(
                run,
                "worktree_create_exact",
                "intent",
                &json!({"destination":destination,"branch":branch,"base":base_commit}),
            )?;
            let result = crate::workspace_lifecycle::create(
                store,
                &plan.project_id,
                i.as_ref(),
                destination,
                branch,
                base_commit,
                git_sha256,
                &d,
                destination_parent_identity,
            )?;
            store.effect_step(run, "worktree_create_exact", "completed", &result)?;
            Ok(result)
        }
        Operation::SecretRotate {
            resource_id,
            previous_version,
        } => crate::secret_lifecycle::rotate(
            store,
            run,
            &plan.project_id,
            &plan.environment,
            resource_id,
            previous_version.as_deref(),
        ),
        Operation::SecretRevoke {
            resource_id,
            expected_version,
        } => {
            local_secret_reference(store, &plan.project_id, &plan.environment, resource_id)?;
            store.revoke_secret_version(run, resource_id, expected_version)
        }
        Operation::JournalArchive {
            before,
            limit,
            selection_digest,
        } => store.archive_terminal_journals(
            run,
            &plan.project_id,
            *before,
            *limit,
            selection_digest,
        ),
    }
}
