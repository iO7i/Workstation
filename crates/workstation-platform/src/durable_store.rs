//! SQLite transactions bind each projection update and event atomically.
use crate::{
    control_store::{epoch, sha},
    storage::Store,
    Error, Result,
};
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde_json::{json, Value};
use workstation_core::{
    durable::*,
    effects::{Effect, EffectPlan},
};
fn db(_: rusqlite::Error) -> Error {
    Error::new("DURABLE_DATABASE_CONFLICT")
}
fn enc<T: serde::Serialize>(v: &T) -> Result<String> {
    serde_json::to_string(v).map_err(|_| Error::new("DURABLE_ENCODING"))
}
fn dec<T: serde::de::DeserializeOwned>(v: &str) -> Result<T> {
    serde_json::from_str(v).map_err(|_| Error::new("DURABLE_STORED_SHAPE"))
}
fn read(conn: &rusqlite::Connection, id: &str) -> Result<RunRecord> {
    let (p, c): (String, bool) = conn
        .query_row(
            "SELECT payload,cancel_requested FROM external_runs WHERE id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(db)?;
    let mut record: RunRecord = dec(&p)?;
    record.cancellation_requested |= c;
    Ok(record)
}
fn update(tx: &rusqlite::Transaction<'_>, mut r: RunRecord, event: &RunEvent) -> Result<RunRecord> {
    let prior = r.revision;
    if r.sequence >= MAX_EVENTS {
        return Err(Error::new("DURABLE_EVENT_BUDGET"));
    }
    apply_event(&mut r, event, epoch()).map_err(Error::new)?;
    let changed=tx.execute("UPDATE external_runs SET revision=?1,seq=?2,updated_at=?3,payload=?4 WHERE id=?5 AND revision=?6",params![sqlint(r.revision)?,sqlint(r.sequence)?,r.updated_at,enc(&r)?,r.id,sqlint(prior)?]).map_err(db)?;
    if changed != 1 {
        return Err(Error::new("DURABLE_REVISION_CONFLICT"));
    }
    tx.execute(
        "INSERT INTO external_run_events VALUES(?1,?2,?3,?4)",
        params![r.id, sqlint(r.sequence)?, r.updated_at, enc(event)?],
    )
    .map_err(db)?;
    Ok(r)
}
impl Store {
    pub fn require_v5(&self) -> Result<()> {
        if self.schema_version()? != 5 {
            return Err(Error::new("DURABLE_UPGRADE_REQUIRED"));
        }
        Ok(())
    }
    pub fn upgrade_v5(&mut self) -> Result<Value> {
        let from = self.schema_version()?;
        if from == 5 {
            return Ok(json!({"from":5,"to":5,"changed":false}));
        }
        let prior = if from < 4 {
            Some(self.upgrade_v4()?)
        } else {
            None
        };
        let backup = self.backup()?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db)?;
        let v: i64 = tx
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .map_err(db)?;
        if v != 4 {
            return Err(Error::new("CONCURRENT_MIGRATION"));
        }
        tx.execute_batch(include_str!("../../../schema/005_durable_runtime.sql"))
            .map_err(db)?;
        tx.commit().map_err(db)?;
        Ok(
            json!({"from":from,"to":5,"changed":true,"backup":backup,"prior_upgrade":prior,"old_runs_replayed":false}),
        )
    }
    pub fn durable_prepare(&mut self, p: &EffectPlan) -> Result<()> {
        self.require_v5()?;
        let Effect::Continue {
            integration_id,
            integration_digest,
            work_id,
            work_version,
            checkpoint_id,
            workspace_digest,
            timeout_seconds,
            deadlines,
            ..
        } = &p.effect
        else {
            return Ok(());
        };
        let integration = self.integration(integration_id)?;
        let cp = self.checkpoint(checkpoint_id)?;
        let policy = deadlines
            .clone()
            .unwrap_or_else(|| DeadlinePolicy::from_legacy(*timeout_seconds));
        policy.validate().map_err(Error::new)?;
        let r = RunRecord {
            id: p.id.clone(),
            plan_id: p.id.clone(),
            execution_id: None,
            project_id: p.project_id.clone(),
            environment: p.environment.clone(),
            integration_id: integration_id.clone(),
            integration_digest: integration_digest.clone(),
            executable_sha256: integration.executable_sha256,
            declared_version: integration.version_text,
            adapter: integration.adapter.id().into(),
            work_id: work_id.clone(),
            work_version: *work_version,
            checkpoint_id: checkpoint_id.clone(),
            workspace_id: cp.workspace.workspace_id,
            workspace_digest: workspace_digest.clone(),
            deadlines: policy,
            supervisor: None,
            child: None,
            session_id: None,
            turn_id: None,
            transport: TransportState::NotStarted,
            provider: ProviderState::Unknown,
            workspace: WorkspaceState::NotObserved,
            verification: VerificationState::NotRequested,
            created_at: epoch(),
            updated_at: epoch(),
            last_heartbeat_at: None,
            last_progress_at: None,
            finished_at: None,
            cancellation_requested: false,
            owner_released: false,
            revision: 0,
            sequence: 0,
            error_code: None,
            deadline_expired: None,
        };
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db)?;
        let n: i64 = tx
            .query_row("SELECT count(*) FROM external_runs", [], |r| r.get(0))
            .map_err(db)?;
        if n >= MAX_RUNS as i64 {
            return Err(Error::new("DURABLE_RUN_BUDGET_ARCHIVE_REVIEW_REQUIRED"));
        }
        tx.execute(
            "INSERT INTO external_runs VALUES(?1,?1,?2,?3,0,0,0,?4,?5)",
            params![r.id, r.project_id, r.environment, r.updated_at, enc(&r)?],
        )
        .map_err(db)?;
        tx.commit().map_err(db)?;
        Ok(())
    }
    /// The effect reservation and supervisor claim COMMIT TOGETHER, before child spawn.
    pub fn durable_begin(
        &mut self,
        p: &EffectPlan,
        digest: &str,
        approval: &str,
    ) -> Result<String> {
        p.approve(digest, approval, epoch()).map_err(Error::new)?;
        self.require_v5()?;
        let me = super::durable_runtime::self_identity(&self.config.installation_id)?;
        let execution = crate::new_id();
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db)?;
        let r = read(&tx, &p.id)?;
        if r.cancellation_requested {
            return Err(Error::new("DURABLE_CANCELLED_BEFORE_START"));
        }
        if r.execution_id.is_some() {
            return Err(Error::new("DURABLE_ALREADY_STARTED_NO_REPLAY"));
        }
        tx.execute(
            "INSERT INTO effect_runs(id,plan_id,state,started_at) VALUES(?1,?2,'running',?3)",
            params![execution, p.id, epoch()],
        )
        .map_err(db)?;
        update(
            &tx,
            r,
            &RunEvent::Claimed {
                supervisor: me,
                execution_id: execution.clone(),
            },
        )?;
        tx.commit().map_err(db)?;
        Ok(execution)
    }
    pub fn durable_run(&self, id: &str) -> Result<RunRecord> {
        self.require_v5()?;
        workstation_core::control::id(id).map_err(Error::new)?;
        read(&self.conn, id)
    }
    pub fn durable_runs(&self, project: &str) -> Result<Vec<RunRecord>> {
        self.require_v5()?;
        workstation_core::control::id(project).map_err(Error::new)?;
        let mut q=self.conn.prepare("SELECT id FROM external_runs WHERE project_id=?1 ORDER BY updated_at DESC,id LIMIT 100").map_err(db)?;
        let ids = q
            .query_map([project], |r| r.get::<_, String>(0))
            .map_err(db)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db)?;
        ids.iter().map(|id| self.durable_run(id)).collect()
    }
    pub fn durable_events(&self, id: &str, after: u64, limit: u32) -> Result<Value> {
        self.durable_run(id)?;
        if limit == 0 || limit > 200 {
            return Err(Error::new("DURABLE_EVENT_PAGE_LIMIT"));
        }
        let mut q=self.conn.prepare("SELECT seq,observed_at,payload FROM external_run_events WHERE run_id=?1 AND seq>?2 ORDER BY seq LIMIT ?3").map_err(db)?;
        let rows = q
            .query_map(params![id, sqlint(after)?, limit], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .map_err(db)?;
        let mut events = vec![];
        for row in rows {
            let (seq, at, p) = row.map_err(db)?;
            events.push(json!({"sequence":seq,"observed_at":at,"event":dec::<RunEvent>(&p)?}));
        }
        Ok(
            json!({"run_id":id,"events":events,"next_after":events.last().and_then(|v|v.get("sequence")).cloned().unwrap_or(json!(after))}),
        )
    }
    pub(crate) fn durable_event(&mut self, id: &str, event: RunEvent) -> Result<RunRecord> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db)?;
        let r = read(&tx, id)?;
        // Resolve cancellation versus timeout while holding the SQLite write transaction.
        // If cancellation committed first, a later deadline observation cannot overwrite it.
        if matches!(event, RunEvent::DeadlineExceeded { .. }) && r.cancellation_requested {
            return Ok(r);
        }
        if matches!(&event,RunEvent::ProviderTerminal{status}|RunEvent::Reconciled{status} if *status==r.provider)
        {
            return Ok(r);
        }
        // Progress cannot consume the last reserved event slots or resurrect a terminal run.
        if matches!(event, RunEvent::Progress)
            && (r.sequence >= MAX_EVENTS - 64 || provider_terminal(r.provider))
        {
            return Ok(r);
        }
        let r = update(&tx, r, &event)?;
        tx.commit().map_err(db)?;
        Ok(r)
    }
    pub(crate) fn durable_heartbeat(
        &mut self,
        id: &str,
        owner: &ProcessIdentity,
    ) -> Result<RunRecord> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db)?;
        let mut r = read(&tx, id)?;
        if r.supervisor.as_ref() != Some(owner) || r.owner_released {
            return Err(Error::new("DURABLE_SUPERVISOR_CHANGED"));
        }
        let at = epoch();
        if at < r.updated_at {
            return Err(Error::new("DURABLE_CLOCK_REGRESSION"));
        }
        r.last_heartbeat_at = Some(at);
        // Heartbeat is not progress and does not create an ever-growing event stream.
        tx.execute(
            "UPDATE external_runs SET payload=?1 WHERE id=?2",
            params![enc(&r)?, id],
        )
        .map_err(db)?;
        tx.commit().map_err(db)?;
        Ok(r)
    }
    pub fn durable_cancel(&mut self, id: &str, expected: u64) -> Result<Value> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db)?;
        let r = read(&tx, id)?;
        if r.revision != expected {
            return Err(Error::new("DURABLE_REVISION_CONFLICT"));
        }
        if r.owner_released || r.cancellation_requested {
            return Ok(json!({"run":id,"changed":false,"no_process_signalled":true}));
        }
        let r = update(&tx, r, &RunEvent::CancelRequested)?;
        tx.execute(
            "UPDATE external_runs SET cancel_requested=1 WHERE id=?1",
            [id],
        )
        .map_err(db)?;
        tx.commit().map_err(db)?;
        Ok(json!({"run":r,"request_persisted":true,"signalled_arbitrary_pid":false}))
    }
    pub(crate) fn durable_baseline(&self, id: &str) -> Result<Option<ContentManifest>> {
        let p: Option<(String, String)> = self
            .conn
            .query_row(
                "SELECT digest,payload FROM external_run_baselines WHERE run_id=?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(db)?;
        p.map(|(d, p)| {
            if sha(p.as_bytes()) != d {
                return Err(Error::new("BASELINE_DIGEST_MISMATCH"));
            }
            dec(&p)
        })
        .transpose()
    }
    pub fn save_durable_baseline(&mut self, id: &str, m: &ContentManifest) -> Result<Value> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db)?;
        let r = read(&tx, id)?;
        if r.execution_id.is_some() {
            return Err(Error::new("BASELINE_MUST_PRECEDE_RUN"));
        }
        let payload = enc(m)?;
        let digest = sha(payload.as_bytes());
        tx.execute(
            "INSERT INTO external_run_baselines VALUES(?1,?2,?3)",
            params![id, digest, payload],
        )
        .map_err(db)?;
        tx.commit().map_err(db)?;
        Ok(
            json!({"run_id":id,"manifest_sha256":digest,"files":m.files.len(),"bytes_hashed":m.bytes,"file_contents_retained":false}),
        )
    }
}

fn sqlint(v: u64) -> Result<i64> {
    i64::try_from(v).map_err(|_| Error::new("DURABLE_INTEGER_RANGE"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use workstation_core::Project;

    #[test]
    fn schema_four_to_five_preserves_data_and_is_idempotent() {
        let scratch = tempfile::tempdir().unwrap();
        let project_path = scratch.path().join("project");
        std::fs::create_dir(&project_path).unwrap();
        let mut store = Store::init(&scratch.path().join("home")).unwrap();
        store
            .register_project(&Project {
                id: "preserved-project".into(),
                path: project_path,
                git_executable: std::env::current_exe().unwrap(),
            })
            .unwrap();
        store.upgrade_v4().unwrap();
        assert_eq!(store.schema_version().unwrap(), 4);

        let result = store.upgrade_v5().unwrap();
        assert_eq!(result["from"], 4);
        assert_eq!(result["to"], 5);
        assert_eq!(result["changed"], true);
        assert!(std::path::Path::new(result["backup"].as_str().unwrap()).is_file());
        assert_eq!(store.projects().unwrap()[0].id, "preserved-project");
        assert_eq!(store.health().unwrap()["integrity"], "ok");
        for table in [
            "external_runs",
            "external_run_events",
            "external_run_baselines",
            "external_run_verifications",
        ] {
            let present: bool = store
                .conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='table' AND name=?1)",
                    [table],
                    |row| row.get(0),
                )
                .unwrap();
            assert!(present, "missing schema-v5 table {table}");
        }

        let second = store.upgrade_v5().unwrap();
        assert_eq!(second, json!({"from":5,"to":5,"changed":false}));
    }
}
