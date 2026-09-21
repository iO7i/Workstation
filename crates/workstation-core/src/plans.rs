//! Plan validation is implemented; no external repair recipe is certified/enabled yet.
use crate::{
    control::{self, Check},
    Coverage,
};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetIdentity {
    pub object_id: String,
    pub identity_digest: String,
    pub coverage: Coverage,
    pub active_dependencies: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TypedAction {
    DockerRuntimeQuarantine,
    CodexAttachmentRegistryReset,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepairPlan {
    pub id: String,
    pub project_id: String,
    pub policy_version: String,
    pub action: TypedAction,
    pub created_at: i64,
    pub expires_at: i64,
    pub target: TargetIdentity,
    pub evidence_refs: Vec<String>,
    pub backup_required: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Approval {
    pub plan_id: String,
    pub digest: String,
    pub approved_at: i64,
}
impl RepairPlan {
    pub fn validate(&self) -> Check<()> {
        control::id(&self.id)?;
        control::id(&self.project_id)?;
        control::timestamp(self.created_at)?;
        control::timestamp(self.expires_at)?;
        control::id(&self.target.object_id)?;
        control::text(&self.policy_version, 128)?;
        control::text(&self.target.identity_digest, 256)?;
        if self.target.identity_digest.is_empty() {
            return Err("TARGET_IDENTITY_REQUIRED");
        }
        control::lines(&self.evidence_refs)?;
        if self.expires_at <= self.created_at || self.expires_at - self.created_at > 900 {
            return Err("PLAN_EXPIRY_INVALID");
        }
        if !self.backup_required || self.evidence_refs.is_empty() {
            return Err("PRESERVATION_AND_EVIDENCE_REQUIRED");
        }
        Ok(())
    }
    pub fn revalidate(
        &self,
        approval: &Approval,
        digest: &str,
        live: &TargetIdentity,
        now: i64,
        policy: &str,
    ) -> Check<()> {
        self.validate()?;
        if approval.plan_id != self.id || approval.digest != digest {
            return Err("APPROVAL_DIGEST_MISMATCH");
        }
        if now < self.created_at
            || now >= self.expires_at
            || approval.approved_at < self.created_at
            || approval.approved_at > now
        {
            return Err("PLAN_EXPIRED_OR_TIME_INVALID");
        }
        if self.policy_version != policy {
            return Err("PLAN_POLICY_CHANGED");
        }
        if live != &self.target {
            return Err("PLAN_TARGET_CHANGED");
        }
        if live.coverage != Coverage::Complete || live.active_dependencies {
            return Err("PROTECTED_TARGET");
        }
        Ok(())
    }
    pub fn execution_status(&self) -> &'static str {
        "blocked_pending_native_recipe_certification"
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn no_generic_shell_action() {
        assert!(serde_json::from_str::<TypedAction>("\"shell\"").is_err());
    }
    #[test]
    fn unknown_input_fields_rejected() {
        assert!(serde_json::from_value::<TargetIdentity>(serde_json::json!({"object_id":"x","identity_digest":"x","coverage":"complete","active_dependencies":false,"force":true})).is_err());
    }
}
