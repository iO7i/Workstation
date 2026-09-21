use crate::{
    control_store::{epoch, sha},
    new_id,
    storage::Store,
    Error, Result,
};
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};
use workstation_core::{control::*, effects::*, integrations::*, lifecycle::LifecycleEvent};
fn db(_: rusqlite::Error) -> Error {
    Error::new("INTEGRATION_DATABASE_REJECTED")
}
fn encode<T: Serialize>(v: &T) -> Result<String> {
    serde_json::to_string(v).map_err(|_| Error::new("ENCODING_FAILED"))
}
fn decode<T: DeserializeOwned>(s: &str) -> Result<T> {
    serde_json::from_str(s).map_err(|_| Error::new("STORED_INTEGRATION_INVALID"))
}
impl Store {
    pub fn require_v3(&self) -> Result<()> {
        if ![3, 4, 5].contains(&self.schema_version()?) {
            return Err(Error::new("INTEGRATION_UPGRADE_REQUIRED"));
        }
        Ok(())
    }
    pub fn upgrade(&mut self) -> Result<Value> {
        self.upgrade_v5()
    }
    pub(crate) fn upgrade_v3_only(&mut self) -> Result<Value> {
        let from = self.schema_version()?;
        if from == 3 {
            return Ok(json!({"from":3,"to":3,"changed":false}));
        }
        if ![1, 2].contains(&from) {
            return Err(Error::new("MIGRATION_SOURCE_UNSUPPORTED"));
        }
        let first = if from == 1 {
            Some(self.upgrade_control_v2()?)
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
        if v != 2 {
            return Err(Error::new("CONCURRENT_MIGRATION"));
        }
        tx.execute_batch(include_str!("../../../schema/003_integrations.sql"))
            .map_err(db)?;
        tx.commit().map_err(db)?;
        Ok(
            json!({"from":from,"to":3,"changed":true,"backup":backup,"prior_upgrade":first,"certification":"deferred"}),
        )
    }
    pub fn require_environment(&self, project: &str, env: &str) -> Result<()> {
        self.require_v3()?;
        id(project).map_err(Error::new)?;
        id(env).map_err(Error::new)?;
        let ok: bool = self
            .conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM environments WHERE project_id=?1 AND name=?2)",
                params![project, env],
                |r| r.get(0),
            )
            .map_err(db)?;
        if !ok {
            return Err(Error::new("EXPLICIT_ENVIRONMENT_REQUIRED_NO_FALLBACK"));
        }
        Ok(())
    }
    pub fn register_integration(&mut self, i: &Integration, approved: &str) -> Result<Value> {
        i.validate().map_err(Error::new)?;
        self.require_environment(&i.project_id, &i.environment)?;
        let payload = encode(i)?;
        let digest = sha(payload.as_bytes());
        if digest != approved {
            return Err(Error::new("INTEGRATION_APPROVAL_MISMATCH"));
        }
        crate::paths::local_existing(std::path::Path::new(&i.executable))?;
        if crate::external::hash_file(std::path::Path::new(&i.executable))? != i.executable_sha256 {
            return Err(Error::new("EXECUTABLE_CHANGED"));
        }
        if payload.len() > 32768 {
            return Err(Error::new("PROFILE_LIMIT"));
        }
        self.conn
            .execute(
                "INSERT INTO integration_profiles VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![
                    i.id,
                    i.project_id,
                    i.environment,
                    i.adapter.id(),
                    digest,
                    payload,
                    epoch()
                ],
            )
            .map_err(db)?;
        Ok(
            json!({"integration_id":i.id,"registration":"explicit_version_claim_not_certification","digest":digest,"capabilities":capabilities(i.adapter)}),
        )
    }
    pub fn integration(&self, id: &str) -> Result<Integration> {
        self.require_v3()?;
        let (p, d): (String, String) = self
            .conn
            .query_row(
                "SELECT payload,digest FROM integration_profiles WHERE id=?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(db)?;
        if sha(p.as_bytes()) != d {
            return Err(Error::new("PROFILE_DIGEST_MISMATCH"));
        }
        let i: Integration = decode(&p)?;
        i.validate().map_err(Error::new)?;
        Ok(i)
    }
    pub fn integrations(&self, project: &str) -> Result<Vec<Integration>> {
        self.require_v3()?;
        let mut q=self.conn.prepare("SELECT payload,digest FROM integration_profiles WHERE project_id=?1 ORDER BY id LIMIT 129").map_err(db)?;
        let rows = q
            .query_map([project], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })
            .map_err(db)?;
        let mut out = vec![];
        for r in rows {
            let (payload, digest) = r.map_err(db)?;
            if sha(payload.as_bytes()) != digest {
                return Err(Error::new("PROFILE_DIGEST_MISMATCH"));
            }
            let i: Integration = decode(&payload)?;
            i.validate().map_err(Error::new)?;
            out.push(i);
        }
        if out.len() > 128 {
            return Err(Error::new("INTEGRATION_QUERY_LIMIT"));
        }
        Ok(out)
    }
    pub fn register_task(&mut self, t: &ApprovedTask, approved: &str) -> Result<Value> {
        t.validate().map_err(Error::new)?;
        self.require_environment(&t.project_id, &t.environment)?;
        let payload = encode(t)?;
        let digest = sha(payload.as_bytes());
        if approved != digest {
            return Err(Error::new("TASK_APPROVAL_MISMATCH"));
        }
        crate::tasks::validate_files(t)?;
        self.conn
            .execute(
                "INSERT INTO approved_tasks VALUES(?1,?2,?3,?4,?5,?6)",
                params![t.id, t.project_id, t.environment, digest, payload, epoch()],
            )
            .map_err(db)?;
        Ok(json!({"task_id":t.id,"digest":digest,"executed":false}))
    }
    pub fn task(&self, id: &str) -> Result<ApprovedTask> {
        self.require_v3()?;
        let (p, d): (String, String) = self
            .conn
            .query_row(
                "SELECT payload,digest FROM approved_tasks WHERE id=?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(db)?;
        if sha(p.as_bytes()) != d {
            return Err(Error::new("TASK_DIGEST_MISMATCH"));
        }
        let t: ApprovedTask = decode(&p)?;
        t.validate().map_err(Error::new)?;
        Ok(t)
    }
    pub fn save_effect_plan(&mut self, p: &EffectPlan) -> Result<Value> {
        p.validate().map_err(Error::new)?;
        self.require_environment(&p.project_id, &p.environment)?;
        let payload = encode(p)?;
        let digest = sha(payload.as_bytes());
        self.conn
            .execute(
                "INSERT INTO effect_plans VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
                params![
                    p.id,
                    p.project_id,
                    p.environment,
                    p.policy,
                    digest,
                    p.created_at,
                    p.expires_at,
                    payload
                ],
            )
            .map_err(db)?;
        Ok(
            json!({"plan":p,"approve_sha256":digest,"effects_performed":false,"release_status":"implementation_not_certified"}),
        )
    }
    pub fn effect_plan(&self, id: &str) -> Result<(EffectPlan, String)> {
        self.require_v3()?;
        let (p, d): (String, String) = self
            .conn
            .query_row(
                "SELECT payload,digest FROM effect_plans WHERE id=?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(db)?;
        let p = self.hydrate_journal("plan", id, &p)?;
        if sha(p.as_bytes()) != d {
            return Err(Error::new("PLAN_CORRUPTION"));
        }
        Ok((decode(&p)?, d))
    }
    pub fn begin_effect(&mut self, p: &EffectPlan, digest: &str, approval: &str) -> Result<String> {
        p.approve(digest, approval, epoch()).map_err(Error::new)?;
        let run = new_id();
        self.conn
            .execute(
                "INSERT INTO effect_runs(id,plan_id,state,started_at) VALUES(?1,?2,'running',?3)",
                params![run, p.id, epoch()],
            )
            .map_err(db)?;
        Ok(run)
    }
    pub fn effect_step(
        &mut self,
        run: &str,
        name: &str,
        status: &str,
        payload: &Value,
    ) -> Result<()> {
        text(name, 128).map_err(Error::new)?;
        let p = encode(payload)?;
        if p.len() > 32768 {
            return Err(Error::new("STEP_SIZE_LIMIT"));
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db)?;
        let state: String = tx
            .query_row("SELECT state FROM effect_runs WHERE id=?1", [run], |r| {
                r.get(0)
            })
            .map_err(db)?;
        if state != "running" {
            return Err(Error::new("EXECUTION_NOT_RUNNING"));
        }
        tx.execute("INSERT INTO effect_steps SELECT ?1,COALESCE(MAX(seq),0)+1,?2,?3,?4,?5 FROM effect_steps WHERE run_id=?1",params![run,name,status,epoch(),p]).map_err(db)?;
        tx.commit().map_err(db)?;
        Ok(())
    }
    pub fn finish_effect(&mut self, r: &Receipt) -> Result<()> {
        let state = serde_json::to_value(r.state).map_err(|_| Error::new("ENCODING_FAILED"))?;
        let state = state.as_str().ok_or(Error::new("STATE_INVALID"))?;
        let n=self.conn.execute("UPDATE effect_runs SET state=?1,finished_at=?2,error_code=?3,receipt=?4 WHERE id=?5 AND plan_id=?6 AND state='running'",params![state,r.finished_at,r.error_code,encode(r)?,r.execution_id,r.plan_id]).map_err(db)?;
        if n != 1 {
            return Err(Error::new("EXECUTION_STATE_CONFLICT"));
        }
        Ok(())
    }
    pub fn effect_history(&self, project: &str) -> Result<Value> {
        self.require_v3()?;
        let mut q=self.conn.prepare("SELECT r.id,r.plan_id,r.state,r.started_at,r.finished_at,r.error_code,r.receipt FROM effect_runs r JOIN effect_plans p ON p.id=r.plan_id WHERE p.project_id=?1 ORDER BY r.started_at DESC LIMIT 100").map_err(db)?;
        let rows = q
            .query_map([project], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, Option<i64>>(4)?,
                    r.get::<_, Option<String>>(5)?,
                    r.get::<_, Option<String>>(6)?,
                ))
            })
            .map_err(db)?;
        let mut out = vec![];
        for r in rows {
            let (a, b, c, d, e, f, g) = r.map_err(db)?;
            out.push(json!({"execution_id":a,"plan_id":b,"state":c,"started_at":d,"finished_at":e,"error_code":f,"receipt":g.map(|x|self.hydrate_journal("receipt",&a,&x).and_then(|s|decode::<Value>(&s))).transpose()?,"unfinished_run_is":"possibly_interrupted_do_not_replay"}));
        }
        Ok(json!({"runs":out}))
    }
    pub fn cache_observation(
        &mut self,
        project: &str,
        category: &str,
        payload: &Value,
    ) -> Result<()> {
        self.require_v3()?;
        id(category).map_err(Error::new)?;
        let data = encode(payload)?;
        if data.len() > 262144 {
            return Err(Error::new("CACHE_LIMIT"));
        }
        self.conn.execute("INSERT INTO observation_caches VALUES(?1,?2,?3,?4) ON CONFLICT(project_id,category) DO UPDATE SET observed_at=excluded.observed_at,payload=excluded.payload",params![project,category,epoch(),data]).map_err(db)?;
        Ok(())
    }
    pub fn cached_observation(&self, project: &str, category: &str) -> Result<Value> {
        self.require_v3()?;
        let row:Option<(i64,String)>=self.conn.query_row("SELECT observed_at,payload FROM observation_caches WHERE project_id=?1 AND category=?2",params![project,category],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(db)?;
        match row {
            Some((at, p)) => Ok(
                json!({"observed_at":at,"fresh":at<=epoch()&&epoch()-at<=300,"data":decode::<Value>(&p)?}),
            ),
            None => Ok(json!({"coverage":"unknown","data":null})),
        }
    }
    pub fn ingest_lifecycle(&mut self, e: &LifecycleEvent) -> Result<Value> {
        self.require_v3()?;
        let at = epoch();
        if e.observed_at > at + 60 || e.observed_at < 0 {
            return Err(Error::new("LIFECYCLE_TIME_INVALID"));
        }
        if let Some(w) = &e.workspace_id {
            self.workspace_path(&e.project_id, w)?;
        }
        let recent:Option<i64>=self.conn.query_row("SELECT max(received_at) FROM lifecycle_events WHERE project_id=?1 AND json_extract(payload,'$.external_session_id')=?2 AND json_extract(payload,'$.agent')=?3",params![e.project_id,e.external_session_id,e.agent],|r|r.get(0)).map_err(db)?;
        if e.event == "activity" && recent.is_some_and(|t| t <= at && at - t < 10) {
            return Ok(
                json!({"recorded":false,"reason":"activity_sampling_budget","authority":"agent_reported"}),
            );
        }
        let payload = encode(e)?;
        let digest = sha(payload.as_bytes());
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db)?;
        let previous: Option<String> = tx
            .query_row(
                "SELECT digest FROM lifecycle_events WHERE project_id=?1 AND source_id=?2",
                params![e.project_id, e.source_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(db)?;
        if let Some(old) = previous {
            if old == digest {
                return Ok(json!({"duplicate":true,"changed":false}));
            }
            return Err(Error::new("LIFECYCLE_ID_REUSE_WITH_DIFFERENT_CONTENT"));
        }
        let session_id = format!(
            "lifecycle-{}",
            &sha(format!("{}:{}:{}", e.project_id, e.agent, e.external_session_id).as_bytes())
                [..32]
        );
        let existing: Option<String> = tx
            .query_row(
                "SELECT payload FROM sessions WHERE project_id=?1 AND agent=?2 AND external_id=?3",
                params![e.project_id, e.agent, e.external_session_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(db)?;
        let mut s = if let Some(x) = existing {
            decode::<Session>(&x)?
        } else {
            Session {
                id: session_id,
                project_id: e.project_id.clone(),
                agent: e.agent.clone(),
                external_id: e.external_session_id.clone(),
                adapter_version: e.source_revision.clone(),
                role: "worker".into(),
                workspace_id: e.workspace_id.clone(),
                last_observed: e.observed_at,
                ended_at: None,
                coverage: workstation_core::Coverage::Partial,
            }
        };
        if e.workspace_id.is_some() && e.workspace_id != s.workspace_id {
            return Err(Error::new(
                "LIFECYCLE_WORKSPACE_CHANGE_REQUIRES_EXPLICIT_REBIND",
            ));
        }
        if e.observed_at >= s.last_observed {
            if s.ended_at.is_some() && e.event != "ended" {
                return Err(Error::new("ENDED_SESSION_NOT_IMPLICITLY_REOPENED"));
            }
            s.last_observed = e.observed_at;
            if e.event == "ended" {
                s.ended_at = Some(e.observed_at);
            }
        }
        tx.execute(
            "INSERT INTO lifecycle_events VALUES(?1,?2,?3,?4,?5)",
            params![e.source_id, e.project_id, digest, payload, at],
        )
        .map_err(db)?;
        tx.execute("INSERT INTO sessions VALUES(?1,?2,?3,?4,?5,?6,?7,?8) ON CONFLICT(id) DO UPDATE SET last_observed=excluded.last_observed,ended_at=excluded.ended_at,payload=excluded.payload",params![s.id,s.project_id,s.agent,s.external_id,s.workspace_id,s.last_observed,s.ended_at,encode(&s)?]).map_err(db)?;
        tx.execute("INSERT INTO audit_events(event_id,project_id,kind,recorded_at,source_kind,payload) VALUES(?1,?2,'lifecycle_import',?3,'agent_reported',?4)",params![new_id(),e.project_id,at,encode(&json!({"session_id":s.id,"event":e.event,"source_id":e.source_id,"transcript_read":false}))?]).map_err(db)?;
        tx.commit().map_err(db)?;
        Ok(
            json!({"session_id":s.id,"recorded":true,"authority":"agent_reported","assignment_changed":false}),
        )
    }
    pub fn renew_lease(&mut self, assignment: &str, session: &str, seconds: u32) -> Result<Value> {
        self.require_v3()?;
        if !(30..=3600).contains(&seconds) {
            return Err(Error::new("LEASE_RANGE"));
        }
        let at = epoch();
        let n=self.conn.execute("UPDATE assignments SET lease_until=?1 WHERE id=?2 AND session_id=?3 AND released_at IS NULL AND EXISTS(SELECT 1 FROM sessions WHERE id=?3 AND ended_at IS NULL)",params![at+i64::from(seconds),assignment,session]).map_err(db)?;
        if n != 1 {
            return Err(Error::new("LEASE_OWNER_OR_STATE_CHANGED"));
        }
        Ok(json!({"lease_until":at+i64::from(seconds),"exclusive_lock":false}))
    }
}

impl Store {
    pub fn reserve_continuation(
        &mut self,
        run: &str,
        cp: &Checkpoint,
        i: &Integration,
        resume: Option<&str>,
        timeout: u32,
    ) -> Result<String> {
        self.require_v3()?;
        let at = epoch();
        let mut session = Session {
            id: format!("run-{run}"),
            project_id: cp.work.project_id.clone(),
            agent: i.adapter.id().into(),
            external_id: format!("pending-{run}"),
            adapter_version: i.version_text.clone(),
            role: "worker".into(),
            workspace_id: Some(cp.workspace.workspace_id.clone()),
            last_observed: at,
            ended_at: None,
            coverage: workstation_core::Coverage::Partial,
        };
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db)?;
        let (version_sql, state): (i64, String) = tx
            .query_row(
                "SELECT version,state FROM work_items WHERE id=?1 AND project_id=?2",
                params![cp.work.id, cp.work.project_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(db)?;
        let version = u64::try_from(version_sql).map_err(|_| Error::new("WORK_VERSION_INVALID"))?;
        if version != cp.work.version || ["done", "cancelled"].contains(&state.as_str()) {
            return Err(Error::new("CONTINUATION_WORK_CHANGED_OR_TERMINAL"));
        }
        let active:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM assignments WHERE work_id=?1 AND role='primary' AND released_at IS NULL)",[&cp.work.id],|r|r.get(0)).map_err(db)?;
        if active {
            return Err(Error::new(
                "PRIMARY_MUST_BE_EXPLICITLY_RELEASED_BEFORE_CONTINUATION",
            ));
        }
        let collision:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM assignments a JOIN sessions s ON s.id=a.session_id WHERE a.project_id=?1 AND s.workspace_id=?2 AND a.role='primary' AND a.released_at IS NULL AND s.ended_at IS NULL)",params![cp.work.project_id,cp.workspace.workspace_id],|r|r.get(0)).map_err(db)?;
        if collision {
            return Err(Error::new("OTHER_PRIMARY_WORKER_IN_WORKSPACE"));
        }
        if let Some(external) = resume {
            let old:Option<String>=tx.query_row("SELECT payload FROM sessions WHERE project_id=?1 AND agent=?2 AND external_id=?3",params![cp.work.project_id,i.adapter.id(),external],|r|r.get(0)).optional().map_err(db)?;
            let old: Session =
                decode(&old.ok_or(Error::new("RESUME_SESSION_MUST_BE_REGISTERED"))?)?;
            if old.workspace_id != session.workspace_id || old.ended_at.is_none() {
                return Err(Error::new("RESUME_SESSION_SCOPE_OR_ACTIVITY"));
            }
            session.id = old.id;
            session.external_id = external.into();
            tx.execute(
                "UPDATE sessions SET last_observed=?1,ended_at=NULL,payload=?2 WHERE id=?3",
                params![at, encode(&session)?, session.id],
            )
            .map_err(db)?;
        } else {
            tx.execute(
                "INSERT INTO sessions VALUES(?1,?2,?3,?4,?5,?6,NULL,?7)",
                params![
                    session.id,
                    session.project_id,
                    session.agent,
                    session.external_id,
                    session.workspace_id,
                    at,
                    encode(&session)?
                ],
            )
            .map_err(db)?;
        }
        tx.execute(
            "INSERT INTO assignments VALUES(?1,?2,?3,?4,'primary',?5,?6,NULL)",
            params![
                format!("assignment-{run}"),
                cp.work.id,
                cp.work.project_id,
                session.id,
                at + i64::from(timeout) + 60,
                at
            ],
        )
        .map_err(db)?;
        tx.execute(
            "INSERT INTO continuation_runs VALUES(?1,?2,?3,?4,'starting',?5)",
            params![run, cp.work.id, cp.id, session.id, at],
        )
        .map_err(db)?;
        tx.execute("INSERT INTO audit_events(event_id,project_id,kind,recorded_at,source_kind,payload) VALUES(?1,?2,'continuation_reserved',?3,'observed',?4)",params![new_id(),cp.work.project_id,at,encode(&json!({"effect_run_id":run,"session_id":session.id,"work_id":cp.work.id,"scope":"bounded_owned_execution_not_exclusive_os_lock"}))?]).map_err(db)?;
        tx.commit().map_err(db)?;
        Ok(session.id)
    }
    pub fn bind_continuation_session(&mut self, run: &str, external: &str) -> Result<()> {
        text(external, 256).map_err(Error::new)?;
        let at = epoch();
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db)?;
        let(id,p):(String,String)=tx.query_row("SELECT s.id,s.payload FROM continuation_runs r JOIN sessions s ON s.id=r.target_session_id WHERE r.effect_run_id=?1 AND r.state='starting'",[run],|r|Ok((r.get(0)?,r.get(1)?))).map_err(db)?;
        let mut session: Session = decode(&p)?;
        session.external_id = external.into();
        session.last_observed = at;
        tx.execute(
            "UPDATE sessions SET external_id=?1,last_observed=?2,payload=?3 WHERE id=?4",
            params![external, at, encode(&session)?, id],
        )
        .map_err(db)?;
        tx.execute(
            "UPDATE continuation_runs SET state='running' WHERE effect_run_id=?1",
            [run],
        )
        .map_err(db)?;
        tx.commit().map_err(db)?;
        Ok(())
    }
    pub fn finish_continuation(&mut self, run: &str, clean: bool) -> Result<()> {
        let at = epoch();
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db)?;
        let row:Option<(String,String)>=tx.query_row("SELECT s.id,s.payload FROM continuation_runs r JOIN sessions s ON s.id=r.target_session_id WHERE r.effect_run_id=?1",[run],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(db)?;
        let Some((id, p)) = row else {
            return Ok(());
        };
        let mut session: Session = decode(&p)?;
        if clean {
            session.last_observed = at;
            session.ended_at = Some(at);
            tx.execute(
                "UPDATE sessions SET last_observed=?1,ended_at=?1,payload=?2 WHERE id=?3",
                params![at, encode(&session)?, id],
            )
            .map_err(db)?;
        }
        tx.execute(
            "UPDATE continuation_runs SET state=?1 WHERE effect_run_id=?2",
            params![if clean { "completed" } else { "uncertain" }, run],
        )
        .map_err(db)?;
        tx.execute("INSERT INTO audit_events(event_id,project_id,kind,recorded_at,source_kind,payload) VALUES(?1,?2,'continuation_transport_ended',?3,'observed',?4)",params![new_id(),session.project_id,at,encode(&json!({"effect_run_id":run,"clean_transport_end":clean,"work_marked_done":false,"primary_assignment_released":false,"ownership_retained":true}))?]).map_err(db)?;
        tx.commit().map_err(db)?;
        Ok(())
    }
    pub fn completed_receipt(&self, run: &str, project: &str) -> Result<Receipt> {
        let payload:String=self.conn.query_row("SELECT r.receipt FROM effect_runs r JOIN effect_plans p ON p.id=r.plan_id WHERE r.id=?1 AND p.project_id=?2 AND r.state='succeeded'",params![run,project],|r|r.get(0)).map_err(db)?;
        decode(&self.hydrate_journal("receipt", run, &payload)?)
    }
    pub fn effect_steps_read(&self, run: &str) -> Result<Value> {
        let mut q=self.conn.prepare("SELECT seq,name,status,observed_at,payload FROM effect_steps WHERE run_id=?1 ORDER BY seq LIMIT 101").map_err(db)?;
        let rows = q
            .query_map([run], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, String>(4)?,
                ))
            })
            .map_err(db)?;
        let mut out = vec![];
        for row in rows {
            let (seq, name, status, at, p) = row.map_err(db)?;
            out.push(json!({"seq":seq,"name":name,"status":status,"observed_at":at,"data":decode::<Value>(&self.hydrate_journal("step",&format!("{run}:{seq}"),&p)?)?}));
        }
        if out.len() > 100 {
            return Err(Error::new("STEP_LIMIT"));
        }
        Ok(json!({"steps":out}))
    }
}
