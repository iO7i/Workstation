use crate::{
    control_store::{epoch, sha},
    storage::Store,
    Error, Result,
};
use rusqlite::{params, TransactionBehavior};
use serde_json::{json, Value};
use workstation_core::{integrations::Integration, telemetry};
impl Store {
    /// Preconfigured hooks are an explicit local write path, not an authenticated channel.
    pub fn ingest_usage_hook(
        &mut self,
        i: &Integration,
        format: &str,
        payload: &Value,
    ) -> Result<Value> {
        self.require_v4()?;
        if !telemetry::profile_formats(i.adapter).contains(&format) {
            return Err(Error::new("TELEMETRY_PROFILE_MISMATCH"));
        }
        let at = epoch();
        let normalized = telemetry::normalize(format, payload, &i.version_text, &i.id, at)
            .map_err(Error::new)?;
        // No raw payload persisted. Stable digest identifies a same-second duplicate projection.
        let digest =
            sha(&serde_json::to_vec(&normalized).map_err(|_| Error::new("TELEMETRY_ENCODING"))?);
        let cache_key = format!("telemetry-{}", &sha(i.id.as_bytes())[..16]);
        if self
            .cached_observation(&i.project_id, &cache_key)?
            .pointer("/data/digest")
            .and_then(Value::as_str)
            == Some(digest.as_str())
        {
            return Ok(json!({"duplicate":true,"effects":"none"}));
        }
        if !normalized.samples.is_empty() {
            let mut samples = normalized.samples.clone();
            for (n, s) in samples.iter_mut().enumerate() {
                s.id = format!("hook-{}-{n}", &digest[..32]);
            }
            // Hook approval permits reported metadata, never user/provider authority promotion.
            let tx = self
                .conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(|_| Error::new("TELEMETRY_TRANSACTION"))?;
            for s in &samples {
                s.validate().map_err(Error::new)?;
                let meter =
                    serde_json::to_value(s.meter).map_err(|_| Error::new("METER_ENCODING"))?;
                tx.execute("INSERT INTO quota_samples VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13) ON CONFLICT(id) DO NOTHING",params![s.id,i.project_id,s.provider,s.account_alias,s.bucket,meter.as_str(),s.unit,s.window_id,s.used,s.limit,s.reset_at,s.observed_at,serde_json::to_string(s).map_err(|_|Error::new("SAMPLE_ENCODING"))?]).map_err(|_|Error::new("TELEMETRY_SAMPLE_REJECTED"))?;
            }
            tx.execute("DELETE FROM quota_samples WHERE project_id=?1 AND id NOT IN(SELECT id FROM quota_samples WHERE project_id=?1 ORDER BY observed_at DESC,id DESC LIMIT 10000)",[&i.project_id]).map_err(|_|Error::new("TELEMETRY_RETENTION"))?;
            tx.commit().map_err(|_| Error::new("TELEMETRY_COMMIT"))?;
        }
        let result = json!({"digest":digest,"integration_id":i.id,"adapter":i.adapter,"format":format,"observed_at":at,"samples":normalized.samples,"metrics":normalized.metrics,"provenance":normalized.provenance,"quota_supported":!normalized.samples.is_empty(),"raw_content_retained":false});
        self.cache_observation(&i.project_id, &cache_key, &result)?;
        Ok(
            json!({"stored":true,"integration_id":i.id,"raw_content_retained":false,"provenance":"agent_reported"}),
        )
    }
    pub fn telemetry_summary(&self, project: &str) -> Result<Value> {
        if self.schema_version()? < 4 {
            return Ok(json!({"streams":[],"coverage":"upgrade_required"}));
        }
        let mut q=self.conn.prepare("SELECT category,observed_at,payload FROM observation_caches WHERE project_id=?1 AND category LIKE 'telemetry-%' ORDER BY category LIMIT 65").map_err(|_|Error::new("TELEMETRY_QUERY"))?;
        let rows = q
            .query_map([project], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .map_err(|_| Error::new("TELEMETRY_QUERY"))?;
        let mut out = vec![];
        for row in rows {
            let (category, at, p) = row.map_err(|_| Error::new("TELEMETRY_QUERY"))?;
            let data: Value =
                serde_json::from_str(&p).map_err(|_| Error::new("TELEMETRY_SHAPE"))?;
            out.push(json!({"stream":category,"observed_at":at,"fresh":at<=epoch()&&epoch()-at<=300,"data":data}));
        }
        if out.len() > 64 {
            return Err(Error::new("TELEMETRY_STREAM_LIMIT"));
        }
        Ok(
            json!({"streams":out,"unavailable_provider_quota":"unknown_not_scraped","combined_quota":null}),
        )
    }
}
