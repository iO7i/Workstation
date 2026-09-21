//! Cached capability availability and explicit adoption drafts; no integration activation.
use crate::{
    control_store::{epoch, sha, Record},
    storage::Store,
    Error, Result,
};
use serde_json::{json, Value};
use workstation_core::{
    control::{self, ResourceKind},
    discovery::Capability,
    integrations::Adapter,
};
impl Store {
    pub fn available_capabilities(&self, project: &str) -> Result<Vec<Value>> {
        if self.schema_version()? < 3 {
            return Ok(vec![]);
        }
        let mut out = vec![];
        let at = epoch();
        for i in self.integrations(project)? {
            let resource = match i.adapter {
                Adapter::Worktrunk => Some("worktrunk"),
                Adapter::Entire => Some("entire"),
                Adapter::Doppler => Some("doppler-cli"),
                Adapter::OnePassword => Some("onepassword-references"),
                Adapter::Codex => Some("codex-app-server"),
                _ => None,
            };
            let Some(resource) = resource else {
                continue;
            };
            let cached = self
                .cached_observation(project, &format!("profile-{}", &sha(i.id.as_bytes())[..16]))?;
            let data = cached.get("data").unwrap_or(&Value::Null);
            let observed = data.get("observed_at").and_then(Value::as_i64);
            let fresh = observed.is_some_and(|t| t <= at && at - t <= 300);
            let available = fresh
                && data
                    .get("executable_digest_matches")
                    .and_then(Value::as_bool)
                    == Some(true)
                && data.get("executable_sha256").and_then(Value::as_str)
                    == Some(i.executable_sha256.as_str());
            out.push(json!({"resource_id":resource,"integration_id":i.id,"environment":i.environment,"state":if available{"available"}else{"unknown"},"observed_at":observed,"runtime_compatibility_certified":false,"scope":"registered_executable_available_not_feature_effectiveness"}));
        }
        Ok(out)
    }
    pub fn capability_adoption_draft(
        &self,
        project: &str,
        environment: &str,
        resource_id: &str,
        revision: &str,
        rationale: &str,
    ) -> Result<Value> {
        self.require_environment(project, environment)?;
        control::text(rationale, 1024).map_err(Error::new)?;
        if rationale.trim().is_empty() {
            return Err(Error::new("ADOPTION_RATIONALE_REQUIRED"));
        }
        let mut catalog: Vec<Capability> =
            serde_json::from_str(include_str!("../../../catalog/resources.json"))
                .map_err(|_| Error::new("CATALOG_INVALID"))?;
        let mut q=self.conn.prepare("SELECT payload FROM capability_candidates WHERE id=?1 AND review_status='reviewed'").map_err(|_|Error::new("CATALOG_QUERY"))?;
        let rows = q
            .query_map([resource_id], |r| r.get::<_, String>(0))
            .map_err(|_| Error::new("CATALOG_QUERY"))?;
        for row in rows {
            let s = row.map_err(|_| Error::new("CATALOG_QUERY"))?;
            catalog.push(serde_json::from_str(&s).map_err(|_| Error::new("CATALOG_INVALID"))?);
        }
        let matches = catalog
            .into_iter()
            .filter(|c| c.id == resource_id)
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err(Error::new("CATALOG_RESOURCE_MISSING_OR_AMBIGUOUS"));
        }
        let c = &matches[0];
        c.validate().map_err(Error::new)?;
        let at = epoch();
        if c.review_status != "reviewed"
            || c.review_revision != revision
            || c.reviewed_at > at
            || at - c.reviewed_at > i64::from(c.fresh_for_seconds)
        {
            return Err(Error::new("CATALOG_REVISION_NEEDS_REVIEW"));
        }
        let resource = Record::ResourceAdd {
            id: crate::new_id(),
            project_id: project.into(),
            environment: environment.into(),
            kind: ResourceKind::Reference,
            name: c.title.clone(),
            locator: c.source.clone(),
            provider: None,
            fresh_for_secs: c.fresh_for_seconds,
        };
        let decision = Record::DecisionPropose {
            id: crate::new_id(),
            project_id: project.into(),
            topic: format!("Capability {resource_id}"),
            scope: environment.into(),
            statement: format!(
                "Evaluate/adopt reference {} at revision {}.",
                c.title, revision
            ),
            rationale: rationale.into(),
            alternatives: vec!["Keep the current workflow without this resource.".into()],
            consequences: c.limitations.clone(),
            predecessor: None,
            effective_at: at,
        };
        Ok(
            json!({"atlas_record_draft":resource,"chronicle_proposal_draft":decision,"current_catalog_revision":revision,"created_records":false,"accepted_decision":false,"installation_authorized":false,"next":"Review/save each draft and use the ordinary record preview/approval. Accepting a decision is a separate action."}),
        )
    }
    pub fn atlas_status(&self, project: &str, environment: &str) -> Result<Value> {
        let resources = self.resources(project, environment)?;
        let at = epoch();
        let mut out = vec![];
        for resource in resources {
            let confirmed:Option<i64>=self.conn.query_row("SELECT max(observed_at) FROM resource_verifications WHERE resource_id=?1 AND claim='user_confirmed' AND observed_at<=?2",rusqlite::params![resource.id,at],|r|r.get(0)).map_err(|_|Error::new("VERIFICATION_QUERY"))?;
            let endpoint = self.cached_observation(
                project,
                &format!("endpoint-{}", &sha(resource.id.as_bytes())[..16]),
            )?;
            let seen = endpoint.get("observed_at").and_then(Value::as_i64);
            let fresh =
                seen.is_some_and(|t| t <= at && at - t <= i64::from(resource.fresh_for_secs));
            out.push(json!({"resource":resource,"authority_confirmation_at":confirmed,"authority_confirmation_fresh":confirmed.is_some_and(|t|at-t<=i64::from(resource.fresh_for_secs)),"network_observation":endpoint,"network_observation_fresh":fresh,"claims":{"declared":true,"user_confirmed":confirmed.is_some(),"provider_identity_verified":false},"no_fallback_environment":true,"provider_identity":"unsupported_no_verifier_registered","confirmation_is_not_network_or_provider_evidence":true}));
        }
        Ok(
            json!({"project_id":project,"environment":environment,"resources":out,"source":"cached_only","secrets_resolved":false}),
        )
    }
}
