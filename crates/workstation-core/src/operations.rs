//! Bounded v4 operation contracts. All effects use the existing plan/journal boundary.
use crate::{
    control::{self, Check},
    integrations::{absolute_path, digest},
    workspace_policy::WorkspaceDetail,
};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CleanupTarget {
    pub project_id: String,
    pub workspace_id: String,
    pub path: String,
    pub directory_identity: String,
    pub git_sha256: String,
    pub head: String,
    pub branch: Option<String>,
    pub repository_identity: String,
    pub state_digest: String,
    pub preservation_ref: String,
    pub quiescence_acknowledged: bool,
}
impl CleanupTarget {
    pub fn validate(&self) -> Check<()> {
        control::id(&self.project_id)?;
        control::id(&self.workspace_id)?;
        absolute_path(&self.path)?;
        digest(&self.state_digest)?;
        digest(&self.git_sha256)?;
        if !self.quiescence_acknowledged {
            return Err("QUIESCENCE_ACK_REQUIRED");
        }
        let suffix = self
            .preservation_ref
            .strip_prefix("refs/workstation/retired/")
            .ok_or("PRESERVATION_REF_INVALID")?;
        if suffix.is_empty()
            || suffix.len() > 64
            || !suffix
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-')
        {
            return Err("PRESERVATION_REF_INVALID");
        }
        if !matches!(self.head.len(), 40 | 64) || !self.head.bytes().all(|c| c.is_ascii_hexdigit())
        {
            return Err("HEAD_INVALID");
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    WorkspaceCleanup {
        target: CleanupTarget,
    },
    WorktreeCreate {
        integration_id: Option<String>,
        integration_digest: Option<String>,
        destination: String,
        branch: String,
        base_commit: String,
        repository_identity: String,
        source_workspace_id: String,
        git_sha256: String,
        destination_parent_identity: String,
    },
    SecretRotate {
        resource_id: String,
        previous_version: Option<String>,
    },
    SecretRevoke {
        resource_id: String,
        expected_version: String,
    },
    JournalArchive {
        before: i64,
        limit: u32,
        #[serde(default)]
        selection_digest: String,
    },
}
impl Operation {
    pub fn validate(&self) -> Check<()> {
        match self {
            Self::WorkspaceCleanup { target } => target.validate(),
            Self::WorktreeCreate {
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
                absolute_path(destination)?;
                branch_name(branch)?;
                control::text(repository_identity, 128)?;
                control::id(source_workspace_id)?;
                digest(git_sha256)?;
                control::text(destination_parent_identity, 128)?;
                if !matches!(base_commit.len(), 40 | 64)
                    || !base_commit.bytes().all(|c| c.is_ascii_hexdigit())
                {
                    return Err("EXACT_BASE_COMMIT_REQUIRED");
                }
                match (integration_id, integration_digest) {
                    (Some(i), Some(d)) => {
                        control::id(i)?;
                        digest(d)?;
                    }
                    (None, None) => {}
                    _ => return Err("INTEGRATION_PIN_INCOMPLETE"),
                };
                Ok(())
            }
            Self::SecretRotate {
                resource_id,
                previous_version,
            } => {
                control::id(resource_id)?;
                if let Some(v) = previous_version {
                    control::id(v)?;
                }
                Ok(())
            }
            Self::SecretRevoke {
                resource_id,
                expected_version,
            } => {
                control::id(resource_id)?;
                control::id(expected_version)
            }
            Self::JournalArchive {
                before,
                limit,
                selection_digest,
            } => {
                digest(selection_digest)?;
                control::timestamp(*before)?;
                if !(1..=100).contains(limit) {
                    return Err("ARCHIVE_LIMIT");
                }
                Ok(())
            }
        }
    }
}
pub fn branch_name(s: &str) -> Check<()> {
    control::text(s, 128)?;
    if s.is_empty()
        || s.starts_with('-')
        || s.starts_with('/')
        || s.ends_with('/')
        || s.ends_with('.')
        || s.ends_with(".lock")
        || s.contains("..")
        || s.contains("@{")
        || s.contains("//")
        || !s
            .bytes()
            .all(|x| x.is_ascii_alphanumeric() || b"-_/".contains(&x))
    {
        return Err("BRANCH_NAME_INVALID");
    }
    Ok(())
}
pub fn state_projection(d: &WorkspaceDetail) -> serde_json::Value {
    serde_json::json!({"identity":d.stamp.directory_identity,"path":d.stamp.path,"head":d.stamp.head,"branch":d.stamp.branch,"status_digest":d.stamp.status_digest,"dirty":d.stamp.dirty,"untracked":d.stamp.untracked,"ignored":d.stamp.ignored,"local_only_commits":d.stamp.local_only_commits,"blockers":d.stamp.blockers,"repository_identity":d.repository_identity,"main":d.main_worktree,"locked":d.locked,"coverage":d.path_coverage,"checkout_filters_present":d.checkout_filters_present})
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_options_in_branch() {
        assert!(branch_name("--force").is_err());
        assert!(branch_name("feat/auth").is_ok());
    }
    #[test]
    fn rejects_ref_traversal() {
        for s in ["a..b", "a/.lock", "/x", "x/"] {
            assert!(branch_name(s).is_err());
        }
    }
}
