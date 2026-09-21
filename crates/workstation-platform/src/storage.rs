use crate::{new_id, now, paths, Error, Result};
use rusqlite::{
    backup::{Backup, StepResult},
    params, Connection, OpenFlags, OptionalExtension,
};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use workstation_core::*;

const APPLICATION_ID: i64 = 1_465_078_832;
const DATABASE_LIMIT: u64 = 32 * 1024 * 1024;
const MAX_SAVED_SCANS: i64 = 20;
const SQL: &str = include_str!("../../../schema/001_r0.sql");
pub struct Store {
    pub(crate) conn: Connection,
    pub home: PathBuf,
    pub config: Config,
}

fn db_err(_: rusqlite::Error) -> Error {
    Error::new("DATABASE_OPERATION_FAILED")
}
pub fn sqlite_runtime_allowed(version: i32) -> bool {
    version >= 3_051_003 && version != 3_052_000
}
fn verify_runtime() -> Result<()> {
    if !sqlite_runtime_allowed(rusqlite::version_number()) {
        return Err(Error::new("UNPATCHED_SQLITE_REJECTED"));
    }
    Ok(())
}
fn configure(conn: &Connection) -> Result<()> {
    conn.busy_timeout(Duration::from_millis(250))
        .map_err(db_err)?;
    conn.execute_batch("PRAGMA foreign_keys=ON; PRAGMA trusted_schema=OFF; PRAGMA synchronous=FULL; PRAGMA max_page_count=8192;").map_err(db_err)?;
    Ok(())
}
fn verify_foreign_keys(conn: &Connection) -> Result<()> {
    let mut q = conn.prepare("PRAGMA foreign_key_check").map_err(db_err)?;
    if q.query([])
        .map_err(db_err)?
        .next()
        .map_err(db_err)?
        .is_some()
    {
        return Err(Error::new("DATABASE_FOREIGN_KEY_INTEGRITY_FAILED"));
    }
    Ok(())
}
fn read_config(home: &Path) -> Result<Config> {
    if paths::entry_exists(&home.join("restore.pending"))? {
        return Err(Error::new("RESTORE_INCOMPLETE_HOME_PROTECTED"));
    }
    let p = home.join("config.json");
    paths::regular_file(&p)?;
    let mut b = Vec::new();
    fs::File::open(&p)
        .map_err(|_| Error::new("CONFIG_UNREADABLE"))?
        .take(64 * 1024 + 1)
        .read_to_end(&mut b)
        .map_err(|_| Error::new("CONFIG_UNREADABLE"))?;
    if b.len() > 64 * 1024 {
        return Err(Error::new("CONFIG_TOO_LARGE"));
    }
    let config: Config = serde_json::from_slice(&b).map_err(|_| Error::new("CONFIG_INVALID"))?;
    if config.schema_version != SCHEMA_VERSION || config.home != home {
        return Err(Error::new("HOME_CONFIG_MISMATCH"));
    }
    Ok(config)
}

impl Store {
    pub fn init(home: &Path) -> Result<Self> {
        verify_runtime()?;
        paths::prospective_home(home)?;
        fs::create_dir(home).map_err(|_| Error::new("HOME_CREATE_FAILED"))?;
        #[cfg(windows)]
        crate::windows::restrict_home_acl(home)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(home, fs::Permissions::from_mode(0o700))
                .map_err(|_| Error::new("HOME_PERMISSIONS_FAILED"))?;
        }
        for d in ["reports", "backups"] {
            fs::create_dir(home.join(d)).map_err(|_| Error::new("HOME_CREATE_FAILED"))?;
        }
        let config = Config {
            schema_version: SCHEMA_VERSION.into(),
            home: home.into(),
            installation_id: new_id(),
            created_at: now(),
        };
        // Only a newly created home is initialized. On failure it is preserved for inspection.
        let conn = Connection::open(home.join("workstation.db")).map_err(db_err)?;
        configure(&conn)?;
        conn.execute_batch("PRAGMA journal_mode=WAL;")
            .map_err(db_err)?;
        conn.execute_batch(SQL).map_err(db_err)?;
        let mut f = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(home.join("config.json"))
            .map_err(|_| Error::new("CONFIG_CREATE_FAILED"))?;
        let bytes = serde_json::to_vec_pretty(&config)
            .map_err(|_| Error::new("CONFIG_SERIALIZE_FAILED"))?;
        f.write_all(&bytes)
            .and_then(|_| f.sync_all())
            .map_err(|_| Error::new("CONFIG_WRITE_FAILED"))?;
        Ok(Self {
            conn,
            home: home.into(),
            config,
        })
    }
    pub fn open_readonly(home: &Path) -> Result<Self> {
        verify_runtime()?;
        paths::directory(home)?;
        let config = read_config(home)?;
        let db = home.join("workstation.db");
        paths::regular_file(&db)?;
        for name in ["workstation.db-wal", "workstation.db-shm"] {
            let p = home.join(name);
            if paths::entry_exists(&p)? {
                paths::local_existing(&p)?;
            }
        }
        if fs::metadata(&db)
            .map_err(|_| Error::new("DATABASE_UNREADABLE"))?
            .len()
            > DATABASE_LIMIT
        {
            return Err(Error::new("DATABASE_SIZE_LIMIT"));
        }
        let conn =
            Connection::open_with_flags(db, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(db_err)?;
        conn.busy_timeout(Duration::from_millis(250))
            .map_err(db_err)?;
        conn.execute_batch("PRAGMA query_only=ON; PRAGMA trusted_schema=OFF;")
            .map_err(db_err)?;
        let app: i64 = conn
            .query_row("PRAGMA application_id", [], |r| r.get(0))
            .map_err(db_err)?;
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .map_err(db_err)?;
        if app != APPLICATION_ID || ![2, 3, 4, 5].contains(&version) {
            return Err(Error::new("CONTROL_SCHEMA_UPGRADE_REQUIRED"));
        }
        Ok(Self {
            conn,
            home: home.into(),
            config,
        })
    }
    pub fn open(home: &Path) -> Result<Self> {
        verify_runtime()?;
        paths::directory(home)?;
        let config = read_config(home)?;
        let db = home.join("workstation.db");
        paths::regular_file(&db)?;
        for n in ["workstation.db-wal", "workstation.db-shm"] {
            let p = home.join(n);
            if paths::entry_exists(&p)? {
                paths::local_existing(&p)?;
            }
        }
        if fs::metadata(&db)
            .map_err(|_| Error::new("DATABASE_UNREADABLE"))?
            .len()
            > DATABASE_LIMIT
        {
            return Err(Error::new("DATABASE_SIZE_LIMIT"));
        }
        let conn = Connection::open_with_flags(
            db,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(db_err)?;
        configure(&conn)?;
        let v: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .map_err(db_err)?;
        let app: i64 = conn
            .query_row("PRAGMA application_id", [], |r| r.get(0))
            .map_err(db_err)?;
        if ![1, 2, 3, 4, 5].contains(&v) || app != APPLICATION_ID {
            return Err(Error::new("DATABASE_SCHEMA_UNSUPPORTED"));
        }
        Ok(Self {
            conn,
            home: home.into(),
            config,
        })
    }
    pub fn health(&self) -> Result<serde_json::Value> {
        let integrity: String = self
            .conn
            .query_row("PRAGMA quick_check(1)", [], |r| r.get(0))
            .map_err(db_err)?;
        if integrity != "ok" {
            return Err(Error::new("DATABASE_INTEGRITY_FAILED"));
        }
        verify_foreign_keys(&self.conn)?;
        let source: String = self
            .conn
            .query_row("SELECT sqlite_source_id()", [], |r| r.get(0))
            .map_err(db_err)?;
        let journal: String = self
            .conn
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .map_err(db_err)?;
        if journal != "wal" {
            return Err(Error::new("DATABASE_JOURNAL_UNEXPECTED"));
        }
        Ok(
            serde_json::json!({"sqlite_version": rusqlite::version(), "sqlite_source_id": source,
            "integrity": integrity, "journal_mode": journal, "saved_scan_limit": MAX_SAVED_SCANS}),
        )
    }
    pub fn roots(&self) -> Result<Vec<Root>> {
        let mut q = self
            .conn
            .prepare("SELECT id,path,kind FROM roots ORDER BY id")
            .map_err(db_err)?;
        let rows = q
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .map_err(db_err)?;
        let mut out = Vec::new();
        for row in rows {
            let (id, path, kind) = row.map_err(db_err)?;
            let kind = serde_json::from_value(serde_json::Value::String(kind))
                .map_err(|_| Error::new("ROOT_KIND_INVALID"))?;
            out.push(Root {
                id,
                path: PathBuf::from(path),
                kind,
            });
        }
        if out.len() > MAX_ROOTS {
            return Err(Error::new("ROOT_LIMIT"));
        }
        Ok(out)
    }
    pub fn register_root(&mut self, root: &Root) -> Result<()> {
        if !workstation_core::policy::valid_id(&root.id) {
            return Err(Error::new("INVALID_ID"));
        }
        paths::directory(&root.path)?;
        if paths::overlaps(&self.home, &root.path) {
            return Err(Error::new("OWN_HOME_NOT_A_SCAN_ROOT"));
        }
        // Serializing metadata registration prevents concurrent overlapping-root insertion.
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(db_err)?;
        let existing: Vec<String> = {
            let mut q = tx.prepare("SELECT path FROM roots").map_err(db_err)?;
            let rows = q.query_map([], |r| r.get(0)).map_err(db_err)?;
            rows.collect::<std::result::Result<Vec<_>, _>>()
                .map_err(db_err)?
        };
        if existing.len() >= MAX_ROOTS {
            return Err(Error::new("ROOT_LIMIT"));
        }
        if existing
            .iter()
            .any(|p| paths::overlaps(Path::new(p), &root.path))
        {
            return Err(Error::new("OVERLAPPING_ROOT"));
        }
        let kind = serde_json::to_value(root.kind).map_err(|_| Error::new("SERIALIZE_FAILED"))?;
        tx.execute(
            "INSERT INTO roots(id,path,kind) VALUES(?1,?2,?3)",
            params![root.id, root.path.to_string_lossy(), kind.as_str()],
        )
        .map_err(db_err)?;
        tx.commit().map_err(db_err)
    }
    pub fn projects(&self) -> Result<Vec<Project>> {
        let mut q = self
            .conn
            .prepare("SELECT id,path,git_executable FROM projects ORDER BY id")
            .map_err(db_err)?;
        let rows = q
            .query_map([], |r| {
                Ok(Project {
                    id: r.get(0)?,
                    path: PathBuf::from(r.get::<_, String>(1)?),
                    git_executable: PathBuf::from(r.get::<_, String>(2)?),
                })
            })
            .map_err(db_err)?;
        let out: Vec<Project> = rows
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db_err)?;
        if out.len() > MAX_PROJECTS {
            return Err(Error::new("PROJECT_LIMIT"));
        }
        Ok(out)
    }
    pub fn register_project(&mut self, project: &Project) -> Result<()> {
        if !workstation_core::policy::valid_id(&project.id) {
            return Err(Error::new("INVALID_ID"));
        }
        paths::directory(&project.path)?;
        paths::local_existing(&project.git_executable)?;
        if paths::overlaps(&self.home, &project.path) {
            return Err(Error::new("PROJECT_HOME_OVERLAP"));
        }
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(db_err)?;
        let count: i64 = tx
            .query_row("SELECT count(*) FROM projects", [], |r| r.get(0))
            .map_err(db_err)?;
        if count >= MAX_PROJECTS as i64 {
            return Err(Error::new("PROJECT_LIMIT"));
        }
        tx.execute(
            "INSERT INTO projects(id,path,git_executable) VALUES(?1,?2,?3)",
            params![
                project.id,
                project.path.to_string_lossy(),
                project.git_executable.to_string_lossy()
            ],
        )
        .map_err(db_err)?;
        tx.commit().map_err(db_err)
    }
    pub fn latest(&self) -> Result<Option<Snapshot>> {
        let json: Option<String> = self
            .conn
            .query_row(
                "SELECT snapshot_json FROM scans ORDER BY observed_at DESC,run_id DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(db_err)?;
        json.map(|j| serde_json::from_str(&j).map_err(|_| Error::new("SNAPSHOT_INVALID")))
            .transpose()
    }
    pub fn save(&mut self, snapshot: &Snapshot) -> Result<()> {
        let json =
            serde_json::to_string(snapshot).map_err(|_| Error::new("SNAPSHOT_SERIALIZE_FAILED"))?;
        if json.len() > MAX_WIRE_BYTES {
            return Err(Error::new("SNAPSHOT_SIZE_LIMIT"));
        }
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(db_err)?;
        tx.execute(
            "INSERT INTO scans(run_id,observed_at,snapshot_json) VALUES(?1,?2,?3)",
            params![snapshot.run_id, snapshot.observed_at, json],
        )
        .map_err(db_err)?;
        tx.execute("DELETE FROM scans WHERE run_id NOT IN (SELECT run_id FROM scans ORDER BY observed_at DESC,run_id DESC LIMIT ?1)", [MAX_SAVED_SCANS]).map_err(db_err)?;
        tx.commit().map_err(db_err)
    }
    /// Restore verified backup metadata into a NEW home only. Existing homes are never overwritten.
    pub fn restore_into(home: &Path, source: &Path, expected_sha256: &str) -> Result<Self> {
        verify_runtime()?;
        paths::local_existing(source)?;
        paths::prospective_home(home)?;
        if expected_sha256.len() != 64 || !expected_sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(Error::new("BACKUP_DIGEST_REQUIRED"));
        }
        let meta = fs::symlink_metadata(source).map_err(|_| Error::new("BACKUP_UNAVAILABLE"))?;
        if !meta.is_file() || meta.len() > DATABASE_LIMIT {
            return Err(Error::new("BACKUP_SIZE_OR_TYPE"));
        }
        for suffix in ["-wal", "-shm"] {
            let side = PathBuf::from(format!("{}{suffix}", source.to_string_lossy()));
            if paths::entry_exists(&side)? {
                return Err(Error::new("RESTORE_REQUIRES_ISOLATED_BACKUP"));
            }
        }
        let mut f = fs::File::open(source).map_err(|_| Error::new("BACKUP_UNREADABLE"))?;
        let mut hasher = Sha256::new();
        let mut bytes = [0u8; 16384];
        let mut consumed = 0u64;
        loop {
            let n = f
                .read(&mut bytes)
                .map_err(|_| Error::new("BACKUP_UNREADABLE"))?;
            if n == 0 {
                break;
            }
            consumed = consumed
                .checked_add(n as u64)
                .ok_or(Error::new("BACKUP_SIZE_OR_TYPE"))?;
            if consumed > DATABASE_LIMIT {
                return Err(Error::new("BACKUP_SIZE_OR_TYPE"));
            }
            hasher.update(&bytes[..n]);
        }
        if format!("{:x}", hasher.finalize()) != expected_sha256 {
            return Err(Error::new("BACKUP_DIGEST_MISMATCH"));
        }
        let from = Connection::open_with_flags(source, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(db_err)?;
        from.busy_timeout(Duration::from_millis(250))
            .map_err(db_err)?;
        from.execute_batch("PRAGMA trusted_schema=OFF; PRAGMA query_only=ON;")
            .map_err(db_err)?;
        let app: i64 = from
            .query_row("PRAGMA application_id", [], |r| r.get(0))
            .map_err(db_err)?;
        let v: i64 = from
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .map_err(db_err)?;
        let check: String = from
            .query_row("PRAGMA integrity_check(1)", [], |r| r.get(0))
            .map_err(db_err)?;
        if app != APPLICATION_ID || ![1, 2, 3, 4, 5].contains(&v) || check != "ok" {
            return Err(Error::new("BACKUP_IDENTITY_OR_INTEGRITY"));
        }
        verify_foreign_keys(&from)?;
        let mut target = Self::init(home)?;
        let marker = home.join("restore.pending");
        let mut pending = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&marker)
            .map_err(|_| Error::new("RESTORE_MARKER_CREATE_FAILED"))?;
        pending.write_all(b"Restore in progress. Do not open or use this home before review of a failed restore.\n").and_then(|_|pending.sync_all()).map_err(|_|Error::new("RESTORE_MARKER_WRITE_FAILED"))?;
        drop(pending);
        let start = Instant::now();
        {
            let copy = Backup::new(&from, &mut target.conn).map_err(db_err)?;
            loop {
                if start.elapsed() > Duration::from_secs(5) {
                    return Err(Error::new("RESTORE_INCOMPLETE_NEW_HOME_PRESERVED"));
                }
                match copy.step(64).map_err(db_err)? {
                    StepResult::Done => break,
                    StepResult::More | StepResult::Busy | StepResult::Locked => thread_sleep(),
                    _ => return Err(Error::new("RESTORE_STATUS_UNSUPPORTED")),
                }
            }
        }
        crate::journal_archive::restore_archive_dependencies(&from, source, home)?;
        target.health()?;
        drop(target);
        paths::local_existing(&marker)?;
        fs::remove_file(&marker).map_err(|_| Error::new("RESTORE_MARKER_CLEAR_FAILED"))?;
        Self::open(home)
    }
    pub fn backup(&self) -> Result<PathBuf> {
        let folder = self.home.join("backups");
        paths::directory(&folder)?;
        let existing = fs::read_dir(&folder)
            .map_err(|_| Error::new("BACKUP_DIRECTORY_UNAVAILABLE"))?
            .take(11)
            .count();
        if existing >= 10 {
            return Err(Error::new("BACKUP_BUDGET_REVIEW_REQUIRED"));
        }
        let destination = folder.join(format!("{}.sqlite", new_id()));
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&destination)
            .map_err(|_| Error::new("BACKUP_CREATE_FAILED"))?;
        drop(file);
        let mut target =
            Connection::open_with_flags(&destination, OpenFlags::SQLITE_OPEN_READ_WRITE)
                .map_err(db_err)?;
        let start = Instant::now();
        {
            let backup = Backup::new(&self.conn, &mut target).map_err(db_err)?;
            loop {
                if start.elapsed() > Duration::from_secs(5) {
                    return Err(Error::new("BACKUP_TIMEOUT_REVIEW_PARTIAL"));
                }
                match backup.step(64).map_err(db_err)? {
                    StepResult::Done => break,
                    StepResult::More | StepResult::Busy | StepResult::Locked => {
                        std::thread::sleep(Duration::from_millis(10))
                    }
                    _ => return Err(Error::new("BACKUP_STATUS_UNSUPPORTED")),
                }
            }
        }
        let check: String = target
            .query_row("PRAGMA integrity_check", [], |r| r.get(0))
            .map_err(db_err)?;
        if check != "ok" {
            return Err(Error::new("BACKUP_INTEGRITY_FAILED"));
        }
        verify_foreign_keys(&target)?;
        let archive_dependencies = crate::journal_archive::copy_archive_dependencies(
            &target,
            &self.home,
            &destination.with_extension("archives"),
        )?;
        drop(target);
        let mut file =
            fs::File::open(&destination).map_err(|_| Error::new("BACKUP_READ_FAILED"))?;
        let mut hash = Sha256::new();
        let mut bytes = [0u8; 16 * 1024];
        loop {
            let n = file
                .read(&mut bytes)
                .map_err(|_| Error::new("BACKUP_READ_FAILED"))?;
            if n == 0 {
                break;
            }
            hash.update(&bytes[..n]);
        }
        let receipt = serde_json::json!({"schema": 1, "created_at": now(), "sha256": format!("{:x}", hash.finalize()),
            "integrity": "ok", "sqlite_version": rusqlite::version(), "contains": "private metadata and referenced journal archive companions; vault ciphertext and vendor state are excluded", "archive_dependencies":archive_dependencies});
        let mut receipt_file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(destination.with_extension("receipt.json"))
            .map_err(|_| Error::new("BACKUP_RECEIPT_FAILED"))?;
        receipt_file
            .write_all(
                serde_json::to_string_pretty(&receipt)
                    .map_err(|_| Error::new("SERIALIZE_FAILED"))?
                    .as_bytes(),
            )
            .and_then(|_| receipt_file.sync_all())
            .map_err(|_| Error::new("BACKUP_RECEIPT_FAILED"))?;
        Ok(destination)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn runtime_gate_rejects_unpatched_and_withdrawn() {
        assert!(!sqlite_runtime_allowed(3_051_002));
        assert!(!sqlite_runtime_allowed(3_052_000));
        assert!(sqlite_runtime_allowed(3_051_003));
        assert!(sqlite_runtime_allowed(3_053_002));
    }
    #[test]
    fn init_open_backup() {
        let t = tempfile::tempdir().unwrap();
        let home = t.path().join("home");
        let s = Store::init(&home).unwrap();
        assert_eq!(s.health().unwrap()["integrity"], "ok");
        assert!(s.backup().unwrap().is_file());
        drop(s);
        assert!(Store::open(&home).is_ok());
    }
    #[test]
    fn init_does_not_replace_existing_directory() {
        let t = tempfile::tempdir().unwrap();
        fs::write(t.path().join("canary"), b"keep").unwrap();
        assert!(Store::init(t.path()).is_err());
        assert_eq!(fs::read(t.path().join("canary")).unwrap(), b"keep");
    }
    #[test]
    fn register_and_reject_overlapping_root() {
        let t = tempfile::tempdir().unwrap();
        let mut s = Store::init(&t.path().join("home")).unwrap();
        let p = t.path().join("data");
        fs::create_dir(&p).unwrap();
        fs::create_dir(p.join("nested")).unwrap();
        s.register_root(&Root {
            id: "data".into(),
            path: p.clone(),
            kind: RootKind::Other,
        })
        .unwrap();
        assert!(s
            .register_root(&Root {
                id: "nested".into(),
                path: p.join("nested"),
                kind: RootKind::Other
            })
            .is_err());
        assert_eq!(s.roots().unwrap().len(), 1);
    }
    #[test]
    fn opens_only_known_schema() {
        let t = tempfile::tempdir().unwrap();
        let home = t.path().join("home");
        let s = Store::init(&home).unwrap();
        s.conn.execute_batch("PRAGMA user_version=99").unwrap();
        drop(s);
        assert!(Store::open(&home).is_err());
    }
}

pub fn linked_sqlite_version() -> &'static str {
    rusqlite::version()
}

fn thread_sleep() {
    std::thread::sleep(Duration::from_millis(10));
}

#[cfg(test)]
mod audit_restore_tests {
    use super::*;
    #[test]
    fn sqlite_integrity_is_not_foreign_key_integrity() {
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch("PRAGMA foreign_keys=OFF;CREATE TABLE parent(id INTEGER PRIMARY KEY);CREATE TABLE child(id INTEGER REFERENCES parent);INSERT INTO child VALUES(42);").unwrap();
        let check: String = c
            .query_row("PRAGMA integrity_check", [], |r| r.get(0))
            .unwrap();
        assert_eq!(check, "ok");
        assert!(verify_foreign_keys(&c).is_err());
    }
    #[test]
    fn incomplete_restore_home_refuses_open() {
        let t = tempfile::tempdir().unwrap();
        let h = t.path().join("home");
        drop(Store::init(&h).unwrap());
        fs::write(h.join("restore.pending"), "pending").unwrap();
        assert!(Store::open(&h).is_err());
        assert!(Store::open_readonly(&h).is_err());
    }
}
