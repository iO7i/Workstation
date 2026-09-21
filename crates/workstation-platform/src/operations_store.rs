//! v4 persistence: explicit protection, process provenance, scope-bound versions and cached graph.
use crate::{control_store::epoch, storage::Store, Error, Result};
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde_json::{json, Value};
use workstation_core::{control::*, ownership::*, workspace_policy::*};

fn db(_: rusqlite::Error) -> Error {
    Error::new("OPERATIONS_DATABASE_REJECTED")
}
impl Store {
    pub fn require_v4(&self) -> Result<()> {
        if ![4, 5].contains(&self.schema_version()?) {
            return Err(Error::new("OPERATIONS_UPGRADE_REQUIRED"));
        }
        Ok(())
    }
    pub fn upgrade_v4(&mut self) -> Result<Value> {
        let from = self.schema_version()?;
        if from == 4 {
            return Ok(json!({"from":4,"to":4,"changed":false}));
        }
        if ![1, 2, 3].contains(&from) {
            return Err(Error::new("MIGRATION_SOURCE_UNSUPPORTED"));
        }
        let prior = if from < 3 {
            Some(self.upgrade_v3_only()?)
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
        if v != 3 {
            return Err(Error::new("CONCURRENT_MIGRATION"));
        }
        tx.execute_batch(include_str!("../../../schema/004_lifecycle_completion.sql"))
            .map_err(db)?;
        tx.commit().map_err(db)?;
        Ok(
            json!({"from":from,"to":4,"backup":backup,"prior_upgrade":prior,"changed":true,"certification":"not_run"}),
        )
    }
    /// Conservative epoch, not a claimed kernel boot GUID. Uptime regression or large
    /// wall-clock drift invalidates old bindings. A missed host observation cannot prove continuity.
    pub fn host_epoch(&mut self, uptime_ms: u64) -> Result<String> {
        self.require_v4()?;
        let at = epoch();
        let tick = i64::try_from(uptime_ms).map_err(|_| Error::new("UPTIME_OVERFLOW"))?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db)?;
        let old: Option<(String, i64, i64)> = tx
            .query_row(
                "SELECT epoch_id,last_uptime_ms,observed_at FROM host_epochs WHERE singleton=1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()
            .map_err(db)?;
        let keep = old.as_ref().is_some_and(|(_, t, w)| {
            tick >= *t && at >= *w && ((tick - *t) / 1000 - (at - *w)).abs() < 5 && at - *w <= 86400
        });
        let id = if keep {
            old.as_ref().unwrap().0.clone()
        } else {
            crate::new_id()
        };
        tx.execute("INSERT INTO host_epochs VALUES(1,?1,?2,?3) ON CONFLICT(singleton) DO UPDATE SET epoch_id=excluded.epoch_id,last_uptime_ms=excluded.last_uptime_ms,observed_at=excluded.observed_at",params![id,tick,at]).map_err(db)?;
        tx.commit().map_err(db)?;
        Ok(id)
    }
    pub fn bind_owned_process(
        &mut self,
        run: &str,
        pid: u32,
        creation: u64,
        executable_digest: &str,
    ) -> Result<()> {
        self.require_v4()?;
        if creation == 0 || pid == 0 {
            return Err(Error::new("PROCESS_IDENTITY_REQUIRED"));
        }
        workstation_core::integrations::digest(executable_digest).map_err(Error::new)?;
        let boot = self.host_epoch(crate::process_graph::uptime_ms()?)?;
        let session:String=self.conn.query_row("SELECT target_session_id FROM continuation_runs WHERE effect_run_id=?1 AND state='running'",[run],|r|r.get(0)).map_err(db)?;
        self.conn
            .execute(
                "INSERT OR IGNORE INTO managed_process_bindings VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
                params![
                    session,
                    self.config.installation_id,
                    boot,
                    pid,
                    creation.to_string(),
                    executable_digest,
                    run,
                    epoch()
                ],
            )
            .map_err(db)?;
        Ok(())
    }
    pub fn process_bindings(&self, project: &str) -> Result<Vec<Binding>> {
        self.require_v4()?;
        let mut q=self.conn.prepare("SELECT b.host_id,b.boot_id,b.pid,b.creation_time,b.session_id,b.bound_at FROM managed_process_bindings b JOIN sessions s ON s.id=b.session_id WHERE s.project_id=?1 ORDER BY b.bound_at DESC LIMIT 4097").map_err(db)?;
        let rows = q
            .query_map([project], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, u32>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, i64>(5)?,
                ))
            })
            .map_err(db)?;
        let mut out = vec![];
        for row in rows {
            let (h, b, p, c, s, at) = row.map_err(db)?;
            out.push(Binding {
                process: ProcessKey {
                    host: h,
                    boot: b,
                    pid: p,
                    created: c
                        .parse()
                        .map_err(|_| Error::new("PROCESS_CREATION_INVALID"))?,
                },
                session_id: s,
                source: "observed_owned_child_and_protocol_session".into(),
                shared: false,
                seen_at: at,
                expires_at: at.saturating_add(86400),
            });
        }
        let mut h=self.conn.prepare("SELECT b.host_id,b.boot_id,b.pid,b.creation_time,b.session_id,b.bound_at,b.expires_at FROM hook_process_bindings b JOIN sessions s ON s.id=b.session_id WHERE s.project_id=?1 AND b.expires_at>=?2 ORDER BY b.bound_at DESC LIMIT 4097").map_err(db)?;
        let rows = h
            .query_map(params![project, epoch()], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, u32>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, i64>(5)?,
                    r.get::<_, i64>(6)?,
                ))
            })
            .map_err(db)?;
        for row in rows {
            let (host, boot, pid, birth, session, at, until) = row.map_err(db)?;
            out.push(Binding {
                process: ProcessKey {
                    host,
                    boot,
                    pid,
                    created: birth
                        .parse()
                        .map_err(|_| Error::new("PROCESS_CREATION_INVALID"))?,
                },
                session_id: session,
                source: "observed_configured_hook_ancestry_not_exclusive".into(),
                shared: true,
                seen_at: at,
                expires_at: until,
            });
        }
        if out.len() > 8192 {
            return Err(Error::new("BINDING_BUDGET"));
        }
        Ok(out)
    }
    pub fn protection(
        &mut self,
        project: &str,
        workspace: &str,
        reason: &str,
        release: Option<&str>,
    ) -> Result<Value> {
        self.require_v4()?;
        self.workspace_path(project, workspace)?;
        text(reason, 512).map_err(Error::new)?;
        if let Some(id) = release {
            let n=self.conn.execute("UPDATE workspace_protections SET released_at=?1 WHERE id=?2 AND project_id=?3 AND workspace_id=?4 AND released_at IS NULL",params![epoch(),id,project,workspace]).map_err(db)?;
            if n != 1 {
                return Err(Error::new("PROTECTION_NOT_CURRENT"));
            }
            Ok(json!({"released":id,"other_protection_still_applies":true}))
        } else {
            let id = crate::new_id();
            self.conn
                .execute(
                    "INSERT INTO workspace_protections VALUES(?1,?2,?3,?4,?5,NULL)",
                    params![id, project, workspace, reason, epoch()],
                )
                .map_err(db)?;
            Ok(json!({"protection_id":id,"active":true}))
        }
    }
    pub fn cleanup_context(&self, detail: WorkspaceDetail, ack: bool) -> Result<CleanupContext> {
        self.require_v4()?;
        let p = &detail.stamp.project_id;
        let w = &detail.stamp.workspace_id;
        let mut q=self.conn.prepare("SELECT reason FROM workspace_protections WHERE project_id=?1 AND workspace_id=?2 AND released_at IS NULL LIMIT 101").map_err(db)?;
        let reasons = q
            .query_map(params![p, w], |r| r.get::<_, String>(0))
            .map_err(db)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db)?;
        if reasons.len() > 100 {
            return Err(Error::new("PROTECTION_QUERY_LIMIT"));
        }
        let work_states = self
            .work_items(p)?
            .into_iter()
            .filter(|x| x.workspace_id.as_deref() == Some(w))
            .map(|x| (x.id, x.state))
            .collect();
        let mut unresolved_sessions = Vec::new();
        let mut count = 0;
        for entry in self.roster(p)? {
            count += 1;
            if entry.session.workspace_id.as_deref() == Some(w)
                && (entry.session.ended_at.is_none() || entry.work_id.is_some())
            {
                unresolved_sessions.push(entry.session.id);
            }
        }
        let scoped_sessions = self
            .roster(p)?
            .into_iter()
            .filter(|r| r.session.workspace_id.as_deref() == Some(w))
            .map(|r| r.session.id)
            .collect::<std::collections::BTreeSet<_>>();
        let bindings = self.process_bindings(p)?;
        let requires_live = bindings
            .iter()
            .any(|b| scoped_sessions.contains(&b.session_id));
        let ownership = self.cached_observation(p, "ownership")?;
        let ownership_complete = ownership.get("fresh").and_then(Value::as_bool) == Some(true)
            && ownership.pointer("/data/coverage").and_then(Value::as_str) == Some("complete");
        if let Some(rows) = ownership.pointer("/data/rows").and_then(Value::as_array) {
            for row in rows {
                if let Some(sessions) = row.get("sessions").and_then(Value::as_array) {
                    for session in sessions {
                        if let Some(id) = session.as_str() {
                            if scoped_sessions.contains(id) {
                                unresolved_sessions.push(format!("live-process-session:{id}"));
                            }
                        }
                    }
                }
            }
        }
        unresolved_sessions.sort();
        unresolved_sessions.dedup();
        Ok(CleanupContext {
            detail,
            work_states,
            unresolved_sessions,
            protection_reasons: reasons,
            observations_complete: count < 100 && (!requires_live || ownership_complete),
            quiescence_acknowledged: ack,
            preserve_head_ref: true,
        })
    }
    pub fn workspace_actors(&self, project: &str) -> Result<Vec<WorkspaceActor>> {
        let at = epoch();
        let cache = self.cached_observation(project, "workspace-details")?;
        let details: Vec<WorkspaceDetail> =
            if cache.get("fresh").and_then(Value::as_bool) == Some(true) {
                cache
                    .pointer("/data/details")
                    .cloned()
                    .map(serde_json::from_value)
                    .transpose()
                    .map_err(|_| Error::new("WORKSPACE_DETAIL_CACHE_INVALID"))?
                    .unwrap_or_default()
            } else {
                vec![]
            };
        let mut actors = vec![];
        for e in self.roster(project)? {
            let d = details
                .iter()
                .find(|d| Some(&d.stamp.workspace_id) == e.session.workspace_id.as_ref());
            actors.push(WorkspaceActor {
                session_id: e.session.id,
                work_id: e.work_id,
                project_id: project.into(),
                workspace_id: e.session.workspace_id,
                repository_identity: d.map(|x| x.repository_identity.clone()),
                branch: d.and_then(|x| x.stamp.branch.clone()),
                paths: d.map(|x| x.changed_paths.clone()).unwrap_or_default(),
                paths_complete: d.is_some_and(|x| {
                    x.path_coverage == workstation_core::Coverage::Complete
                        && x.stamp.observed_at <= at
                        && at - x.stamp.observed_at <= 60
                }),
                role: e.role,
                ended: e.session.ended_at.is_some(),
                lease_until: e.lease_until,
                last_observed: e.session.last_observed,
            });
        }
        Ok(actors)
    }
    pub fn secret_head(&self, id: &str) -> Result<Option<(String, String, u64)>> {
        self.require_v4()?;
        let row: Option<(String, String, i64)> = self
            .conn
            .query_row(
                "SELECT version_id,state,generation FROM secret_heads WHERE resource_id=?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()
            .map_err(db)?;
        row.map(|(version, state, generation)| {
            let generation =
                u64::try_from(generation).map_err(|_| Error::new("SECRET_GENERATION_INVALID"))?;
            Ok((version, state, generation))
        })
        .transpose()
    }
    pub fn secret_history(&self, project: &str, environment: &str, id: &str) -> Result<Value> {
        self.require_environment(project, environment)?;
        let r = self
            .resources(project, environment)?
            .into_iter()
            .find(|r| r.id == id)
            .ok_or(Error::new("SECRET_SCOPE_MISMATCH"))?;
        if r.kind != ResourceKind::SecretReference {
            return Err(Error::new("SECRET_REFERENCE_REQUIRED"));
        }
        let head = self.secret_head(id)?;
        let mut q=self.conn.prepare("SELECT version_id,predecessor,created_at FROM secret_versions WHERE resource_id=?1 ORDER BY created_at,version_id LIMIT 129").map_err(db)?;
        let rows=q.query_map([id],|r|Ok(json!({"version":r.get::<_,String>(0)?,"predecessor":r.get::<_,Option<String>>(1)?,"created_at":r.get::<_,i64>(2)?}))).map_err(db)?.collect::<std::result::Result<Vec<_>,_>>().map_err(db)?;
        if rows.len() > 128 {
            return Err(Error::new("SECRET_HISTORY_LIMIT"));
        }
        Ok(
            json!({"resource_id":id,"head":head,"versions":rows,"legacy_ciphertext_possible":head.is_none(),"plaintext_exposed":false,"remote_token_revocation":false,"dpapi_portability":"not_portable_assume_reentry_or_provider_reauthentication","metadata_restored_does_not_mean_secret_resolvable":true}),
        )
    }
    pub fn record_secret_rotation(
        &mut self,
        run: &str,
        id: &str,
        version: &str,
        file: &str,
        digest: &str,
        previous: Option<&str>,
    ) -> Result<()> {
        self.require_v4()?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db)?;
        let old: Option<(String, i64)> = tx
            .query_row(
                "SELECT version_id,generation FROM secret_heads WHERE resource_id=?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(db)?;
        if old.as_ref().is_some_and(|(_, generation)| *generation < 0) {
            return Err(Error::new("SECRET_GENERATION_INVALID"));
        }
        let next_generation = match old.as_ref() {
            Some((_, generation)) => generation
                .checked_add(1)
                .ok_or(Error::new("SECRET_GENERATION_OVERFLOW"))?,
            None => 1,
        };
        if old.as_ref().map(|x| x.0.as_str()) != previous {
            return Err(Error::new("SECRET_HEAD_CHANGED"));
        }
        tx.execute(
            "INSERT INTO secret_versions VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![id, version, file, digest, previous, epoch(), run],
        )
        .map_err(db)?;
        if let Some((ref old_version, _)) = old {
            tx.execute(
                "INSERT INTO secret_transitions VALUES(?1,?2,?3,'superseded',?4,?5)",
                params![crate::new_id(), id, old_version, epoch(), run],
            )
            .map_err(db)?;
        }
        tx.execute("INSERT INTO secret_heads VALUES(?1,?2,'active',?3,?4) ON CONFLICT(resource_id) DO UPDATE SET version_id=excluded.version_id,state='active',generation=excluded.generation,changed_at=excluded.changed_at",params![id,version,next_generation,epoch()]).map_err(db)?;
        tx.execute(
            "INSERT INTO secret_transitions VALUES(?1,?2,?3,'activated',?4,?5)",
            params![crate::new_id(), id, version, epoch(), run],
        )
        .map_err(db)?;
        tx.commit().map_err(db)?;
        Ok(())
    }
    pub fn revoke_secret_version(&mut self, run: &str, id: &str, expected: &str) -> Result<Value> {
        self.require_v4()?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db)?;
        let n=tx.execute("UPDATE secret_heads SET state='revoked',generation=generation+1,changed_at=?1 WHERE resource_id=?2 AND version_id=?3 AND state='active'",params![epoch(),id,expected]).map_err(db)?;
        if n != 1 {
            return Err(Error::new("SECRET_HEAD_CHANGED_OR_REVOKED"));
        }
        tx.execute(
            "INSERT INTO secret_transitions VALUES(?1,?2,?3,'revoked',?4,?5)",
            params![crate::new_id(), id, expected, epoch(), run],
        )
        .map_err(db)?;
        tx.commit().map_err(db)?;
        Ok(
            json!({"resource_id":id,"local_access":"revoked","remote_provider_credential_revoked":false,"ciphertext_retained":true,"secure_erase_claimed":false}),
        )
    }
    pub fn assert_resume_scope(
        &self,
        project: &str,
        adapter: &str,
        external_id: &str,
        cwd: &str,
    ) -> Result<()> {
        let mut q=self.conn.prepare("SELECT w.path FROM sessions s JOIN workspace_registrations w ON w.project_id=s.project_id AND w.id=s.workspace_id WHERE s.project_id=?1 AND s.agent=?2 AND s.external_id=?3 LIMIT 3").map_err(db)?;
        let paths = q
            .query_map(params![project, adapter, external_id], |r| {
                r.get::<_, String>(0)
            })
            .map_err(db)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db)?;
        if paths.is_empty() {
            return Err(Error::new(
                "RESUME_REQUIRES_RECORDED_SAME_PROJECT_WORKSPACE_SESSION",
            ));
        }
        let id = crate::control_workspace::directory_identity(std::path::Path::new(cwd))?;
        for p in paths {
            if crate::control_workspace::directory_identity(std::path::Path::new(&p))? != id {
                return Err(Error::new("RESUME_WORKSPACE_CONFLICT"));
            }
        }
        Ok(())
    }
    pub fn record_retired(
        &mut self,
        run: &str,
        target: &workstation_core::operations::CleanupTarget,
        receipt: &Value,
    ) -> Result<()> {
        self.conn
            .execute(
                "INSERT INTO workspace_retirements VALUES(?1,?2,?3,?4,?5,?6)",
                params![
                    target.project_id,
                    target.workspace_id,
                    run,
                    target.preservation_ref,
                    epoch(),
                    serde_json::to_string(receipt)
                        .map_err(|_| Error::new("RETIREMENT_ENCODING"))?
                ],
            )
            .map_err(db)?;
        Ok(())
    }
    pub fn graph_context(&self, project: &str) -> Result<Value> {
        if self.schema_version()? < 4 {
            return Ok(json!({"coverage":"unsupported","requires_schema":4}));
        }
        let actors = self.workspace_actors(project)?;
        let collisions =
            workstation_core::workspace_policy::conflicts(&actors, epoch()).map_err(Error::new)?;
        Ok(
            json!({"actors":actors,"collisions":collisions,"host_ownership":self.cached_observation(project,"ownership")?,"source":"cached_only","no_background_observation":true}),
        )
    }
}
