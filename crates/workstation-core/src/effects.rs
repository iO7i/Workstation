//! Single preview/approval/execution contract, shared by all seven modules.
use super::integrations::{digest, ApprovedTask};
use crate::control::{self, Check};
use serde::{Deserialize, Serialize};
pub const EFFECT_POLICY: &str = "workstation.effects.v5.durable1";
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepairKind {
    CodexAttachmentRegistry,
    DockerRuntime,
    DockerSecretsRuntime,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuarantineTarget {
    pub kind: RepairKind,
    pub root: String,
    pub target: String,
    pub identity: String,
    pub content_digest: String,
    pub source_version: String,
    pub source_error_path: String,
    pub source_error_sha256: String,
    pub error_observed_at: i64,
    pub original_exists: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueryKind {
    CodexQuota,
    CodexThreads,
    CopilotQuota,
    CopilotModels,
    VendorSessions,
    ProtocolHealth,
    WorktrunkList,
    DockerLocalEngine,
    Version,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BillingProvider {
    OpenAiCosts,
    AnthropicCosts,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Effect {
    Operation {
        request: crate::operations::Operation,
    },
    IntegrationQuery {
        integration_id: String,
        integration_digest: String,
        query: QueryKind,
    },
    CredentialedTask {
        task: ApprovedTask,
    },
    Continue {
        integration_id: String,
        integration_digest: String,
        checkpoint_id: String,
        workspace_digest: String,
        work_id: String,
        work_version: u64,
        resume_external_id: Option<String>,
        permission_mode: String,
        timeout_seconds: u32,
        #[serde(default)]
        deadlines: Option<crate::durable::DeadlinePolicy>,
        packet_digest: String,
        authority_digest: String,
    },
    BillingRead {
        provider: BillingProvider,
        credential_resource_id: String,
        account_alias: String,
        start_time: i64,
        end_time: i64,
    },
    EndpointCheck {
        resource_id: String,
        resource_digest: String,
    },
    Repair {
        target: QuarantineTarget,
    },
    RepairUndo {
        execution_id: String,
        receipt_digest: String,
    },
    DockerStart {
        integration_id: String,
        integration_digest: String,
        desktop_executable: String,
        desktop_sha256: String,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectPlan {
    pub id: String,
    pub project_id: String,
    pub environment: String,
    pub policy: String,
    pub created_at: i64,
    pub expires_at: i64,
    pub effect: Effect,
    #[serde(default)]
    pub secret_generation_digest: String,
    pub inherited_paths_digest: String,
    pub warnings: Vec<String>,
}
impl EffectPlan {
    pub fn validate(&self) -> Check<()> {
        control::id(&self.id)?;
        control::id(&self.project_id)?;
        control::id(&self.environment)?;
        if self.policy != EFFECT_POLICY {
            return Err("EFFECT_POLICY_MISMATCH");
        }
        control::timestamp(self.created_at)?;
        control::timestamp(self.expires_at)?;
        digest(&self.inherited_paths_digest)?;
        digest(&self.secret_generation_digest)?;
        if self.expires_at <= self.created_at || self.expires_at - self.created_at > 900 {
            return Err("EFFECT_PLAN_EXPIRY");
        }
        match &self.effect {
            Effect::Operation { request } => request.validate()?,
            Effect::IntegrationQuery {
                integration_id,
                integration_digest,
                ..
            }
            | Effect::DockerStart {
                integration_id,
                integration_digest,
                ..
            } => {
                control::id(integration_id)?;
                digest(integration_digest)?;
            }
            Effect::CredentialedTask { task } => {
                task.validate()?;
                if task.project_id != self.project_id || task.environment != self.environment {
                    return Err("TASK_SCOPE_MISMATCH");
                }
            }
            Effect::Continue {
                integration_id,
                integration_digest,
                checkpoint_id,
                workspace_digest,
                work_id,
                packet_digest,
                authority_digest,
                permission_mode,
                timeout_seconds,
                deadlines,
                resume_external_id,
                ..
            } => {
                for v in [integration_id, checkpoint_id, work_id] {
                    control::id(v)?;
                }
                for d in [
                    integration_digest,
                    workspace_digest,
                    packet_digest,
                    authority_digest,
                ] {
                    digest(d)?;
                }
                if !["read_only", "allow_once"].contains(&permission_mode.as_str())
                    || !(1..=3600).contains(timeout_seconds)
                {
                    return Err("CONTINUATION_LIMIT");
                }
                if let Some(policy) = deadlines {
                    policy.validate()?;
                }
                if let Some(s) = resume_external_id {
                    control::text(s, 256)?;
                    if s.starts_with('-') || s.trim().is_empty() {
                        return Err("RESUME_ID_INVALID");
                    }
                }
            }
            Effect::BillingRead {
                credential_resource_id,
                account_alias,
                start_time,
                end_time,
                ..
            } => {
                control::id(credential_resource_id)?;
                control::id(account_alias)?;
                control::timestamp(*start_time)?;
                control::timestamp(*end_time)?;
                if start_time >= end_time || end_time - start_time > 32 * 86400 {
                    return Err("BILLING_RANGE_LIMIT");
                }
            }
            Effect::EndpointCheck {
                resource_id,
                resource_digest,
            } => {
                control::id(resource_id)?;
                digest(resource_digest)?;
            }
            Effect::RepairUndo {
                execution_id,
                receipt_digest,
            } => {
                control::id(execution_id)?;
                digest(receipt_digest)?;
            }
            Effect::Repair { target } => {
                super::integrations::absolute_path(&target.root)?;
                super::integrations::absolute_path(&target.target)?;
                super::integrations::absolute_path(&target.source_error_path)?;
                digest(&target.content_digest)?;
                digest(&target.source_error_sha256)?;
                control::text(&target.identity, 256)?;
                control::text(&target.source_version, 128)?;
                if self.created_at < target.error_observed_at
                    || self.created_at - target.error_observed_at > 300
                {
                    return Err("STALE_REPAIR_EVIDENCE");
                }
            }
        }
        if let Effect::DockerStart {
            desktop_executable,
            desktop_sha256,
            ..
        } = &self.effect
        {
            super::integrations::absolute_path(desktop_executable)?;
            digest(desktop_sha256)?;
        }
        Ok(())
    }
    pub fn approve(&self, digest_actual: &str, approved: &str, now: i64) -> Check<()> {
        self.validate()?;
        digest(digest_actual)?;
        if approved != digest_actual {
            return Err("EFFECT_APPROVAL_MISMATCH");
        }
        if now < self.created_at || now >= self.expires_at {
            return Err("EFFECT_PLAN_EXPIRED");
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionState {
    Prepared,
    Running,
    Succeeded,
    Failed,
    Indeterminate,
    Cancelled,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub execution_id: String,
    pub plan_id: String,
    pub state: ExecutionState,
    pub started_at: i64,
    pub finished_at: Option<i64>,
    pub error_code: Option<String>,
    pub output: serde_json::Value,
    pub root_cause_fixed: bool,
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generic_shell_not_an_effect() {
        assert!(serde_json::from_str::<Effect>(r#"{"kind":"shell","command":"del *"}"#).is_err());
    }
    #[test]
    fn authority_extra_key_rejected() {
        assert!(serde_json::from_str::<QuarantineTarget>(r#"{"force":true}"#).is_err());
    }
}
