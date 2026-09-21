//! Descriptive imports and bounded history. Inputs cannot self-authorize environment use.
use crate::{
    control_store::{epoch, sha},
    storage::Store,
    Error, Result,
};
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use workstation_core::{control::*, integrations::ApprovedTask};
fn db(_: rusqlite::Error) -> Error {
    Error::new("PROJECT_METADATA_REJECTED")
}
fn encode<T: Serialize>(v: &T) -> Result<String> {
    serde_json::to_string(v).map_err(|_| Error::new("ENCODING_FAILED"))
}
fn decode<T: serde::de::DeserializeOwned>(v: &str) -> Result<T> {
    serde_json::from_str(v).map_err(|_| Error::new("STORED_PAYLOAD_INVALID"))
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestResource {
    pub id: String,
    pub environment: String,
    pub kind: ResourceKind,
    pub name: String,
    pub locator: String,
    pub provider: Option<String>,
    pub fresh_for_secs: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectManifest {
    pub schema_version: u32,
    pub project_id: String,
    pub environments: Vec<String>,
    pub resources: Vec<ManifestResource>,
}
impl ProjectManifest {
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != 1 || self.environments.len() > 16 || self.resources.len() > 64 {
            return Err(Error::new("MANIFEST_BUDGET"));
        }
        id(&self.project_id).map_err(Error::new)?;
        let mut envs = std::collections::BTreeSet::new();
        for e in &self.environments {
            id(e).map_err(Error::new)?;
            if !envs.insert(e.clone()) {
                return Err(Error::new("DUPLICATE_MANIFEST_ENVIRONMENT"));
            }
        }
        let mut ids = std::collections::BTreeSet::new();
        for r in &self.resources {
            if !ids.insert(r.id.clone()) || !envs.contains(&r.environment) {
                return Err(Error::new("MANIFEST_RESOURCE_SCOPE"));
            }
            validate_resource(&Resource {
                id: r.id.clone(),
                project_id: self.project_id.clone(),
                environment: r.environment.clone(),
                kind: r.kind,
                name: r.name.clone(),
                locator: r.locator.clone(),
                provider: r.provider.clone(),
                evidence: Evidence {
                    kind: EvidenceKind::AgentReported,
                    source: "descriptive_manifest".into(),
                    at: 0,
                    coverage: workstation_core::Coverage::Partial,
                },
                fresh_for_secs: r.fresh_for_secs,
            })
            .map_err(Error::new)?;
        }
        Ok(())
    }
}
impl Store {
    pub fn cost_reports(&self, project: &str) -> Result<Value> {
        if self.schema_version()? < 3 {
            return Ok(Value::Null);
        }
        let mut q=self.conn.prepare("SELECT category,observed_at,payload FROM observation_caches WHERE project_id=?1 AND (category LIKE 'openai-costs%' OR category LIKE 'anthropic-costs%') ORDER BY category LIMIT 33").map_err(db)?;
        let rows = q
            .query_map([project], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .map_err(db)?;
        let mut reports = vec![];
        for row in rows {
            let (key, at, p) = row.map_err(db)?;
            reports.push(json!({"stream":key,"observed_at":at,"stale":at>epoch()||epoch()-at>300,"report":decode::<Value>(&p)?}));
        }
        if reports.len() > 32 {
            return Err(Error::new("COST_ACCOUNT_QUERY_LIMIT"));
        }
        Ok(json!({"organization_cost_reports":reports,"combined_subscription_allowance":null}))
    }

    pub fn authority_digest(&self, project: &str, env: &str) -> Result<String> {
        self.require_environment(project, env)?;
        let at = epoch();
        let decisions = self.decisions(project, at, at)?;
        let resources = self.resources(project, env)?;
        let focus: Option<String> = self
            .conn
            .query_row(
                "SELECT focus_json FROM project_profiles WHERE project_id=?1",
                [project],
                |r| r.get(0),
            )
            .map_err(db)?;
        let mut q=self.conn.prepare("SELECT v.payload FROM resource_verifications v JOIN resources r ON r.id=v.resource_id WHERE r.project_id=?1 AND r.environment=?2 ORDER BY v.id").map_err(db)?;
        let rows = q
            .query_map(params![project, env], |r| r.get::<_, String>(0))
            .map_err(db)?;
        let verified = rows
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db)?;
        if verified.len() > 1000 {
            return Err(Error::new("AUTHORITY_CONTEXT_BUDGET"));
        }
        Ok(sha(encode(&json!({"decisions":decisions,"resources":resources,"confirmations":verified,"focus":focus}))?.as_bytes()))
    }
    pub fn task_inventory(&self, project: &str) -> Result<Value> {
        self.require_v3()?;
        id(project).map_err(Error::new)?;
        let mut q = self
            .conn
            .prepare("SELECT payload FROM approved_tasks WHERE project_id=?1 ORDER BY id LIMIT 129")
            .map_err(db)?;
        let rows = q
            .query_map([project], |r| r.get::<_, String>(0))
            .map_err(db)?;
        let mut out = vec![];
        for r in rows {
            let t: ApprovedTask = decode(&r.map_err(db)?)?;
            out.push(t);
        }
        if out.len() > 128 {
            return Err(Error::new("TASK_QUERY_LIMIT"));
        }
        Ok(json!({"tasks":out,"executed":false}))
    }
    pub fn import_manifest(&mut self, m: &ProjectManifest, approval: &str) -> Result<Value> {
        self.require_v3()?;
        m.validate()?;
        let digest = sha(encode(m)?.as_bytes());
        if digest != approval {
            return Err(Error::new("MANIFEST_APPROVAL_MISMATCH"));
        }
        let at = epoch();
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db)?;
        let exists: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM projects WHERE id=?1)",
                [&m.project_id],
                |r| r.get(0),
            )
            .map_err(db)?;
        if !exists {
            return Err(Error::new("PROJECT_NOT_REGISTERED"));
        }
        for env in &m.environments {
            tx.execute("INSERT INTO environments(project_id,name) VALUES(?1,?2) ON CONFLICT(project_id,name) DO NOTHING",params![m.project_id,env]).map_err(db)?;
        }
        let mut added = 0;
        for x in &m.resources {
            let r = Resource {
                id: x.id.clone(),
                project_id: m.project_id.clone(),
                environment: x.environment.clone(),
                kind: x.kind,
                name: x.name.clone(),
                locator: x.locator.clone(),
                provider: x.provider.clone(),
                evidence: Evidence {
                    kind: EvidenceKind::AgentReported,
                    source: format!("manifest:{}", &digest[..16]),
                    at,
                    coverage: workstation_core::Coverage::Partial,
                },
                fresh_for_secs: x.fresh_for_secs,
            };
            let prior: Option<String> = tx
                .query_row(
                    "SELECT payload FROM resources WHERE id=?1",
                    [&r.id],
                    |row| row.get(0),
                )
                .optional()
                .map_err(db)?;
            if let Some(old) = prior {
                let old: Resource = decode(&old)?;
                if old.project_id != r.project_id
                    || old.environment != r.environment
                    || old.locator != r.locator
                    || old.kind != r.kind
                    || old.name != r.name
                    || old.provider != r.provider
                    || old.fresh_for_secs != r.fresh_for_secs
                {
                    return Err(Error::new("MANIFEST_WOULD_REPLACE_ACCEPTED_RESOURCE"));
                }
                continue;
            }
            let kind = serde_json::to_value(r.kind).map_err(|_| Error::new("ENCODING_FAILED"))?;
            tx.execute(
                "INSERT INTO resources VALUES(?1,?2,?3,?4,?5,?6)",
                params![
                    r.id,
                    r.project_id,
                    r.environment,
                    kind.as_str(),
                    encode(&r)?,
                    at
                ],
            )
            .map_err(db)?;
            added += 1;
        }
        let output = json!({"manifest_digest":digest,"added_resources":added,"resource_authority":"unverified_descriptive_reference","secrets_resolved":false,"decisions_accepted":false});
        tx.execute("INSERT INTO audit_events(event_id,project_id,kind,recorded_at,source_kind,payload) VALUES(?1,?2,'manifest_imported',?3,'user_approved',?4)",params![crate::new_id(),m.project_id,at,encode(&output)?]).map_err(db)?;
        tx.commit().map_err(db)?;
        Ok(output)
    }
    pub fn decision_history(
        &self,
        project: &str,
        topic: Option<&str>,
        known_at: i64,
    ) -> Result<Value> {
        self.require_control()?;
        id(project).map_err(Error::new)?;
        timestamp(known_at).map_err(Error::new)?;
        if let Some(t) = topic {
            text(t, 256).map_err(Error::new)?;
        }
        let mut q=self.conn.prepare("SELECT d.payload,a.accepted_at,s.disposition,s.recorded_at FROM decisions d LEFT JOIN decision_acceptances a ON a.decision_id=d.id AND a.accepted_at<=?2 LEFT JOIN decision_dispositions s ON s.decision_id=d.id AND s.recorded_at<=?2 WHERE d.project_id=?1 AND d.recorded_at<=?2 AND (?3 IS NULL OR d.topic=?3) ORDER BY d.recorded_at,d.id LIMIT 501").map_err(db)?;
        let rows = q
            .query_map(params![project, known_at, topic], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Option<i64>>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, Option<i64>>(3)?,
                ))
            })
            .map_err(db)?;
        let mut out = vec![];
        for row in rows {
            let (p, accepted, disposed, disposed_at) = row.map_err(db)?;
            let d: Decision = decode(&p)?;
            out.push(json!({"decision":d,"accepted_at":accepted,"disposition":disposed,"disposition_recorded_at":disposed_at}));
        }
        if out.len() > 500 {
            return Err(Error::new("HISTORY_QUERY_LIMIT_REFINE_TOPIC"));
        }
        Ok(json!({"known_at":known_at,"revisions":out,"effective_selection_is_separate":true}))
    }
    pub fn compare_decisions(
        &self,
        project: &str,
        before: i64,
        after: i64,
        known: i64,
    ) -> Result<Value> {
        let a = self.decisions(project, before, known)?;
        let b = self.decisions(project, after, known)?;
        let ids_a: std::collections::BTreeSet<_> = a.iter().map(|d| d.id.clone()).collect();
        let ids_b: std::collections::BTreeSet<_> = b.iter().map(|d| d.id.clone()).collect();
        Ok(
            json!({"valid_before":before,"valid_after":after,"known_at":known,"no_longer_applicable":a.iter().filter(|d|!ids_b.contains(&d.id)).collect::<Vec<_>>(),"became_applicable":b.iter().filter(|d|!ids_a.contains(&d.id)).collect::<Vec<_>>(),"past_overwritten":false}),
        )
    }
    pub fn integration_summary(&self, project: &str) -> Result<Value> {
        if self.schema_version()? < 3 {
            return Ok(json!({"integrations":[],"status":"upgrade_required"}));
        }
        let rows=self.integrations(project)?.into_iter().map(|i|json!({"id":i.id,"environment":i.environment,"adapter":i.adapter,"declared_version":i.version_text,"capabilities":workstation_core::integrations::capabilities(i.adapter)})).collect::<Vec<_>>();
        Ok(
            json!({"integrations":rows,"certification":"per_adapter_runtime_evidence_separate_from_declaration","automatic_execution":false,"durable_runs":if self.schema_version()?==5{self.durable_runs(project)?.iter().take(10).map(|r|json!({"id":r.id,"state":r.state(),"updated_at":r.updated_at,"provider":r.provider,"verification":r.verification})).collect::<Vec<_>>()}else{vec![]}}),
        )
    }
    /// Local documented response ingestion, separate from user-uploaded quota imports.
    pub fn save_received_quota(
        &mut self,
        project: &str,
        run: &str,
        mut rows: Vec<workstation_core::economics::QuotaSample>,
    ) -> Result<()> {
        self.require_v3()?;
        if rows.is_empty() || rows.len() > 256 {
            return Err(Error::new("QUOTA_BATCH_LIMIT"));
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db)?;
        for (index, s) in rows.iter_mut().enumerate() {
            s.id = format!("quota-{run}-{index}");
            s.validate().map_err(Error::new)?;
            s.evidence = Evidence {
                kind: EvidenceKind::Observed,
                source: "received_owned_protocol_response_not_account_identity_verified".into(),
                at: epoch(),
                coverage: workstation_core::Coverage::Partial,
            };
            let meter = serde_json::to_value(s.meter).map_err(|_| Error::new("ENCODING_FAILED"))?;
            tx.execute(
                "INSERT INTO quota_samples VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
                params![
                    s.id,
                    project,
                    s.provider,
                    s.account_alias,
                    s.bucket,
                    meter.as_str(),
                    s.unit,
                    s.window_id,
                    s.used,
                    s.limit,
                    s.reset_at,
                    s.observed_at,
                    encode(s)?
                ],
            )
            .map_err(db)?;
        }
        tx.execute("DELETE FROM quota_samples WHERE project_id=?1 AND id NOT IN(SELECT id FROM quota_samples WHERE project_id=?1 ORDER BY observed_at DESC,id DESC LIMIT 10000)",[project]).map_err(db)?;
        tx.commit().map_err(db)?;
        Ok(())
    }
}
