//! Explicit archive/rehydration for immutable journal content. Original row IDs remain.
//! Checksums detect corruption, not malicious same-user rewriting. No signature is claimed.
use crate::{
    control_store::{epoch, sha},
    storage::Store,
    Error, Result,
};
use rusqlite::{params, Connection, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};
const PACK_LIMIT: usize = 8 * 1024 * 1024;
const ARCHIVE_LIMIT: u64 = 64 * 1024 * 1024;
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    kind: String,
    key: String,
    digest: String,
    value: String,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pack {
    schema_version: u32,
    project_id: String,
    created_at: i64,
    entries: Vec<Entry>,
}
fn db(_: rusqlite::Error) -> Error {
    Error::new("ARCHIVE_DATABASE_REJECTED")
}
fn directory(home: &Path) -> Result<PathBuf> {
    let d = home.join("archives");
    if !crate::paths::entry_exists(&d)? {
        fs::create_dir(&d).map_err(|_| Error::new("ARCHIVE_DIRECTORY_CREATE"))?;
    }
    crate::paths::directory(&d)?;
    Ok(d)
}
fn read_bytes(path: &Path) -> Result<Vec<u8>> {
    crate::paths::local_existing(path)?;
    let m = fs::symlink_metadata(path).map_err(|_| Error::new("ARCHIVE_METADATA_UNAVAILABLE"))?;
    if !m.is_file() || m.len() > PACK_LIMIT as u64 {
        return Err(Error::new("ARCHIVE_SIZE_OR_TYPE"));
    }
    let mut bytes = vec![];
    fs::File::open(path)
        .map_err(|_| Error::new("ARCHIVE_MISSING"))?
        .take(PACK_LIMIT as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::new("ARCHIVE_READ"))?;
    if bytes.len() > PACK_LIMIT {
        return Err(Error::new("ARCHIVE_LIMIT"));
    }
    Ok(bytes)
}
fn fixed_name(s: &str) -> Result<()> {
    if s.len() > 80
        || !s.ends_with(".json")
        || !s
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-.".contains(&b))
    {
        return Err(Error::new("ARCHIVE_FILENAME_UNSAFE"));
    }
    Ok(())
}
// Reserve header/delimiter space before retaining each encoded value. The database
// bounds individual rows; this additionally bounds aggregate in-memory materialization.
fn push_bounded(records: &mut Vec<Entry>, used: &mut usize, entry: Entry) -> Result<()> {
    if records.len() >= 10000 {
        return Err(Error::new("ARCHIVE_RECORD_BUDGET"));
    }
    let bytes = serde_json::to_vec(&entry)
        .map_err(|_| Error::new("ARCHIVE_ENCODING"))?
        .len();
    let next = used
        .checked_add(bytes + 1)
        .ok_or(Error::new("ARCHIVE_TOO_LARGE_REDUCE_LIMIT"))?;
    if next > PACK_LIMIT - 4096 {
        return Err(Error::new("ARCHIVE_TOO_LARGE_REDUCE_LIMIT"));
    }
    *used = next;
    records.push(entry);
    Ok(())
}
fn selection_digest(records: &[Entry]) -> Result<String> {
    Ok(sha(
        &serde_json::to_vec(records).map_err(|_| Error::new("ARCHIVE_ENCODING"))?
    ))
}
impl Store {
    pub fn hydrate_journal(&self, kind: &str, key: &str, current: &str) -> Result<String> {
        let v: Value =
            serde_json::from_str(current).map_err(|_| Error::new("JOURNAL_JSON_INVALID"))?;
        let Some(marker) = v.get("workstation_archive") else {
            return Ok(current.into());
        };
        self.require_v4()?;
        if !["plan", "receipt", "step"].contains(&kind) {
            return Err(Error::new("ARCHIVE_KIND_INVALID"));
        }
        let id = marker
            .get("id")
            .and_then(Value::as_str)
            .ok_or(Error::new("ARCHIVE_MARKER_INVALID"))?;
        let digest = marker
            .get("sha256")
            .and_then(Value::as_str)
            .ok_or(Error::new("ARCHIVE_MARKER_INVALID"))?;
        let row:(String,String,String,String,String,i64)=self.conn.query_row("SELECT a.id,a.file_name,a.digest,e.content_digest,a.project_id,a.byte_count FROM journal_archive_entries e JOIN journal_archives a ON a.id=e.archive_id WHERE e.kind=?1 AND e.record_key=?2",params![kind,key],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).map_err(db)?;
        let archive_bytes =
            u64::try_from(row.5).map_err(|_| Error::new("ARCHIVE_BYTE_COUNT_INVALID"))?;
        if row.0 != id || row.3 != digest {
            return Err(Error::new("ARCHIVE_REFERENCE_MISMATCH"));
        }
        fixed_name(&row.1)?;
        let bytes = read_bytes(&self.home.join("archives").join(&row.1))?;
        if bytes.len() as u64 != archive_bytes || sha(&bytes) != row.2 {
            return Err(Error::new("ARCHIVE_CHECKSUM_MISMATCH"));
        }
        let pack: Pack =
            serde_json::from_slice(&bytes).map_err(|_| Error::new("ARCHIVE_FORMAT_INVALID"))?;
        if pack.schema_version != 1 || pack.project_id != row.4 || pack.entries.len() > 10000 {
            return Err(Error::new("ARCHIVE_FORMAT_LIMIT"));
        }
        let items: Vec<_> = pack
            .entries
            .iter()
            .filter(|e| e.kind == kind && e.key == key)
            .collect();
        if items.len() != 1 {
            return Err(Error::new("ARCHIVE_ENTRY_AMBIGUOUS"));
        }
        let e = items[0];
        if e.digest != digest || sha(e.value.as_bytes()) != digest {
            return Err(Error::new("ARCHIVE_ENTRY_CORRUPT"));
        }
        Ok(e.value.clone())
    }
    pub fn archive_inventory(&self, project: &str) -> Result<Value> {
        self.require_v4()?;
        let mut q=self.conn.prepare("SELECT id,file_name,digest,byte_count,record_count,created_at FROM journal_archives WHERE project_id=?1 ORDER BY created_at DESC LIMIT 129").map_err(db)?;
        let raw = q
            .query_map([project], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, i64>(4)?,
                    r.get::<_, i64>(5)?,
                ))
            })
            .map_err(db)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db)?;
        if raw.len() > 128 {
            return Err(Error::new("ARCHIVE_QUERY_LIMIT"));
        }
        let mut rows = Vec::with_capacity(raw.len());
        for (id, file, digest, bytes, records, created_at) in raw {
            let bytes =
                u64::try_from(bytes).map_err(|_| Error::new("ARCHIVE_BYTE_COUNT_INVALID"))?;
            let records =
                u64::try_from(records).map_err(|_| Error::new("ARCHIVE_RECORD_COUNT_INVALID"))?;
            rows.push(json!({"id":id,"file":file,"sha256":digest,"bytes":bytes,"records":records,"created_at":created_at}));
        }
        Ok(
            json!({"archives":rows,"automatic_deletion":false,"max_archive_bytes":ARCHIVE_LIMIT,"database_bytes_freed":"reusable_pages_not_guaranteed_OS_reclamation"}),
        )
    }
    fn terminal_records(&self, project: &str, before: i64, limit: u32) -> Result<Vec<Entry>> {
        self.require_v4()?;
        if !(1..=100).contains(&limit) || before < 0 || before > epoch() - 3600 {
            return Err(Error::new("ARCHIVE_CUTOFF_OR_LIMIT"));
        }
        let mut records = vec![];
        let mut selected_runs = vec![];
        let mut retained_bytes = 0usize;
        {
            let mut q=self.conn.prepare("SELECT p.id,p.payload,r.id,r.receipt FROM effect_plans p JOIN effect_runs r ON r.plan_id=p.id WHERE p.project_id=?1 AND r.state IN ('succeeded','failed','cancelled') AND r.finished_at<?2 AND NOT EXISTS(SELECT 1 FROM journal_archive_entries e WHERE e.kind='plan' AND e.record_key=p.id) ORDER BY r.finished_at,r.id LIMIT ?3").map_err(db)?;
            let rows = q
                .query_map(params![project, before, limit], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, Option<String>>(3)?,
                    ))
                })
                .map_err(db)?;
            for row in rows {
                let (plan, payload, execution, receipt) = row.map_err(db)?;
                push_bounded(
                    &mut records,
                    &mut retained_bytes,
                    Entry {
                        kind: "plan".into(),
                        key: plan,
                        digest: sha(payload.as_bytes()),
                        value: payload,
                    },
                )?;
                if let Some(receipt) = receipt {
                    push_bounded(
                        &mut records,
                        &mut retained_bytes,
                        Entry {
                            kind: "receipt".into(),
                            key: execution.clone(),
                            digest: sha(receipt.as_bytes()),
                            value: receipt,
                        },
                    )?;
                }
                selected_runs.push(execution);
            }
        }
        for execution in &selected_runs {
            let mut q = self
                .conn
                .prepare(
                    "SELECT seq,payload FROM effect_steps WHERE run_id=?1 ORDER BY seq LIMIT 1001",
                )
                .map_err(db)?;
            let rows = q
                .query_map([execution], |r| {
                    Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
                })
                .map_err(db)?;
            let mut n = 0;
            for row in rows {
                let (seq, value) = row.map_err(db)?;
                n += 1;
                if n > 1000 {
                    return Err(Error::new("ARCHIVE_RECORD_BUDGET"));
                }
                push_bounded(
                    &mut records,
                    &mut retained_bytes,
                    Entry {
                        kind: "step".into(),
                        key: format!("{execution}:{seq}"),
                        digest: sha(value.as_bytes()),
                        value,
                    },
                )?;
            }
        }
        Ok(records)
    }
    /// Binds approval to the exact bounded selection, not merely a cutoff and row count.
    pub fn journal_selection_digest(
        &self,
        project: &str,
        before: i64,
        limit: u32,
    ) -> Result<String> {
        selection_digest(&self.terminal_records(project, before, limit)?)
    }
    pub fn archive_terminal_journals(
        &mut self,
        run: &str,
        project: &str,
        before: i64,
        limit: u32,
        expected_selection: &str,
    ) -> Result<Value> {
        let records = self.terminal_records(project, before, limit)?;
        if selection_digest(&records)? != expected_selection {
            return Err(Error::new("ARCHIVE_SELECTION_CHANGED_REPLAN_REQUIRED"));
        }
        if records.is_empty() {
            return Ok(json!({"archived":0,"changed":false}));
        }
        let pack = Pack {
            schema_version: 1,
            project_id: project.into(),
            created_at: epoch(),
            entries: records,
        };
        let bytes = serde_json::to_vec(&pack).map_err(|_| Error::new("ARCHIVE_ENCODING"))?;
        if bytes.len() > PACK_LIMIT {
            return Err(Error::new("ARCHIVE_TOO_LARGE_REDUCE_LIMIT"));
        }
        let dir = directory(&self.home)?;
        let mut total = 0u64;
        let mut count = 0;
        for item in fs::read_dir(&dir).map_err(|_| Error::new("ARCHIVE_DIRECTORY_READ"))? {
            let e = item.map_err(|_| Error::new("ARCHIVE_DIRECTORY_READ"))?;
            crate::paths::local_existing(&e.path())?;
            let m = e
                .metadata()
                .map_err(|_| Error::new("ARCHIVE_DIRECTORY_METADATA"))?;
            total = total.saturating_add(m.len());
            count += 1;
            if count >= 128 {
                return Err(Error::new("ARCHIVE_FILE_BUDGET"));
            }
        }
        if total.saturating_add(bytes.len() as u64) > ARCHIVE_LIMIT {
            return Err(Error::new("ARCHIVE_STORAGE_BUDGET_EXPORT_REQUIRED"));
        }
        let archive_id = crate::new_id();
        let file_name = format!("{archive_id}.json");
        let path = dir.join(&file_name);
        let digest = sha(&bytes);
        self.effect_step(
            run,
            "journal_archive_write",
            "intent",
            &json!({"archive_id":archive_id,"sha256":digest,"records":pack.entries.len()}),
        )?;
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)
            .map_err(|_| Error::new("ARCHIVE_FILE_CREATE"))?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|_| Error::new("ARCHIVE_WRITE_INCOMPLETE_PRESERVED"))?;
        drop(file);
        if sha(&read_bytes(&path)?) != digest {
            return Err(Error::new("ARCHIVE_READBACK_MISMATCH"));
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db)?;
        tx.execute(
            "INSERT INTO journal_archives VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![
                archive_id,
                project,
                file_name,
                digest,
                bytes.len() as i64,
                pack.entries.len() as i64,
                epoch()
            ],
        )
        .map_err(db)?;
        for e in &pack.entries {
            tx.execute(
                "INSERT INTO journal_archive_entries VALUES(?1,?2,?3,?4)",
                params![e.kind, e.key, archive_id, e.digest],
            )
            .map_err(db)?;
            let marker =
                json!({"workstation_archive":{"id":archive_id,"sha256":e.digest}}).to_string();
            let n=match e.kind.as_str(){
    "plan"=>tx.execute("UPDATE effect_plans SET payload=?1 WHERE id=?2 AND payload=?3",params![marker,e.key,e.value]),
    "receipt"=>tx.execute("UPDATE effect_runs SET receipt=?1 WHERE id=?2 AND receipt=?3 AND state IN ('succeeded','failed','cancelled')",params![marker,e.key,e.value]),
    "step"=>{let(id,seq)=e.key.rsplit_once(':').ok_or(Error::new("ARCHIVE_KEY_INVALID"))?;let seq=seq.parse::<i64>().map_err(|_|Error::new("ARCHIVE_KEY_INVALID"))?;tx.execute("UPDATE effect_steps SET payload=?1 WHERE run_id=?2 AND seq=?3 AND payload=?4",params![marker,id,seq,e.value])},
    _=>return Err(Error::new("ARCHIVE_KIND_INVALID")),
   }.map_err(db)?;
            if n != 1 {
                return Err(Error::new("ARCHIVE_SOURCE_CHANGED"));
            }
        }
        tx.commit().map_err(db)?;
        let out = json!({"archive_id":archive_id,"sha256":digest,"records":pack.entries.len(),"bytes":bytes.len(),"logical_history_unchanged":true,"succeeded_failed_cancelled_only":true,"indeterminate_and_running_retained":true,"physical_disk_shrink_claimed":false});
        self.effect_step(run, "journal_archive_commit", "completed", &out)?;
        Ok(out)
    }
}
/// Copy exactly those immutable packs referenced by a particular backup database.
/// This is paired with the SQLite snapshot, not a racy list from the live database.
pub fn copy_archive_dependencies(
    conn: &Connection,
    source_home: &Path,
    destination: &Path,
) -> Result<Vec<Value>> {
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(db)?;
    if version < 4 {
        return Ok(vec![]);
    }
    let mut q = conn
        .prepare("SELECT file_name,digest,byte_count FROM journal_archives ORDER BY id LIMIT 129")
        .map_err(db)?;
    let rows = q
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
            ))
        })
        .map_err(db)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(db)?;
    if rows.len() > 128 {
        return Err(Error::new("BACKUP_ARCHIVE_COUNT_LIMIT"));
    }
    if rows.is_empty() {
        return Ok(vec![]);
    }
    if crate::paths::entry_exists(destination)? {
        return Err(Error::new("BACKUP_ARCHIVE_DESTINATION_EXISTS"));
    }
    fs::create_dir(destination).map_err(|_| Error::new("BACKUP_ARCHIVE_DIRECTORY"))?;
    crate::paths::directory(destination)?;
    let mut summary = vec![];
    let mut sum = 0u64;
    for (name, digest, size_sql) in rows {
        fixed_name(&name)?;
        let size =
            u64::try_from(size_sql).map_err(|_| Error::new("BACKUP_ARCHIVE_BYTES_INVALID"))?;
        sum = sum
            .checked_add(size)
            .ok_or(Error::new("BACKUP_ARCHIVE_BYTES_LIMIT"))?;
        if sum > ARCHIVE_LIMIT {
            return Err(Error::new("BACKUP_ARCHIVE_BYTES_LIMIT"));
        }
        let data = read_bytes(&source_home.join("archives").join(&name))?;
        if data.len() as u64 != size || sha(&data) != digest {
            return Err(Error::new("BACKUP_ARCHIVE_SOURCE_MISMATCH"));
        }
        let mut out = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(destination.join(&name))
            .map_err(|_| Error::new("BACKUP_ARCHIVE_CREATE"))?;
        out.write_all(&data)
            .and_then(|_| out.sync_all())
            .map_err(|_| Error::new("BACKUP_ARCHIVE_WRITE"))?;
        summary.push(json!({"file":name,"sha256":digest,"bytes":size}));
    }
    Ok(summary)
}
/// Restore packs from the backup's explicitly adjacent companion directory. Never fetch.
pub fn restore_archive_dependencies(conn: &Connection, backup: &Path, home: &Path) -> Result<()> {
    let v: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(db)?;
    if v < 4 {
        return Ok(());
    }
    let companion = backup.with_extension("archives");
    let mut q = conn
        .prepare("SELECT file_name,digest,byte_count FROM journal_archives ORDER BY id LIMIT 129")
        .map_err(db)?;
    let rows = q
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
            ))
        })
        .map_err(db)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(db)?;
    if rows.len() > 128 {
        return Err(Error::new("RESTORE_ARCHIVE_COUNT_LIMIT"));
    }
    if rows.is_empty() {
        return Ok(());
    }
    crate::paths::directory(&companion)?;
    let d = directory(home)?;
    let mut total = 0u64;
    for (name, digest, size_sql) in rows {
        fixed_name(&name)?;
        let size =
            u64::try_from(size_sql).map_err(|_| Error::new("RESTORE_ARCHIVE_BYTES_INVALID"))?;
        total = total
            .checked_add(size)
            .ok_or(Error::new("RESTORE_ARCHIVE_BYTES_LIMIT"))?;
        if total > ARCHIVE_LIMIT {
            return Err(Error::new("RESTORE_ARCHIVE_BYTES_LIMIT"));
        }
        let bytes = read_bytes(&companion.join(&name))?;
        if sha(&bytes) != digest || bytes.len() as u64 != size {
            return Err(Error::new("RESTORE_ARCHIVE_CORRUPT"));
        }
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(d.join(&name))
            .map_err(|_| Error::new("RESTORE_ARCHIVE_DESTINATION"))?;
        f.write_all(&bytes)
            .and_then(|_| f.sync_all())
            .map_err(|_| Error::new("RESTORE_ARCHIVE_INCOMPLETE"))?;
    }
    Ok(())
}

#[cfg(test)]
mod audit_tests {
    use super::*;
    fn entry(value: &str) -> Entry {
        Entry {
            kind: "step".into(),
            key: "run:1".into(),
            digest: sha(value.as_bytes()),
            value: value.into(),
        }
    }
    #[test]
    fn archive_rejects_aggregate_before_retaining() {
        let mut entries = vec![];
        let mut bytes = PACK_LIMIT - 4096;
        assert!(push_bounded(&mut entries, &mut bytes, entry("x")).is_err());
        assert!(entries.is_empty());
    }
    #[test]
    fn selection_changes_with_payload() {
        assert_ne!(
            selection_digest(&[entry("a")]).unwrap(),
            selection_digest(&[entry("b")]).unwrap()
        );
    }
    #[test]
    fn record_count_is_exact() {
        let mut entries = (0..10000).map(|_| entry("")).collect();
        let mut bytes = 0;
        assert!(push_bounded(&mut entries, &mut bytes, entry("")).is_err());
    }
}
