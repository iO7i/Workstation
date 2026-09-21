//! Same-user DPAPI lifecycle. Local revocation is not remote-provider token revocation.
//! Ciphertext is immutable; metadata switches with compare-and-swap, never plaintext logging.
use crate::{control_store::sha, storage::Store, vault::SecretBytes, Error, Result};
#[cfg(windows)]
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
#[cfg(windows)]
use std::{
    io::{Read, Write},
    path::Path,
};
#[cfg(windows)]
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BindingV2 {
    schema_version: u32,
    project: String,
    environment: String,
    resource_id: String,
    version_id: String,
    secret: Vec<u8>,
}
fn local_resource(store: &Store, project: &str, environment: &str, id: &str) -> Result<()> {
    store.require_environment(project, environment)?;
    let r = store
        .resources(project, environment)?
        .into_iter()
        .find(|r| r.id == id)
        .ok_or(Error::new("LOCAL_SECRET_SCOPE_MISMATCH"))?;
    if r.locator != format!("local://{project}/{environment}/{id}")
        || r.kind != workstation_core::control::ResourceKind::SecretReference
    {
        return Err(Error::new("LOCAL_DPAPI_REFERENCE_REQUIRED"));
    }
    Ok(())
}
pub fn rotate(
    store: &mut Store,
    run: &str,
    project: &str,
    environment: &str,
    id: &str,
    expected: Option<&str>,
) -> Result<Value> {
    local_resource(store, project, environment, id)?;
    if store.secret_head(id)?.as_ref().map(|x| x.0.as_str()) != expected {
        return Err(Error::new("SECRET_CHANGED_SINCE_PLAN"));
    }
    #[cfg(not(windows))]
    {
        let _ = (run, sha);
        Err(Error::new("DPAPI_WINDOWS_REQUIRED"))
    }
    #[cfg(windows)]
    {
        // Secret is entered only now, in a physical hidden console, not in plans or argv.
        let secret = crate::vault::read_hidden()?;
        if secret.0.is_empty() || secret.0.len() > crate::vault::SECRET_BYTES_LIMIT {
            return Err(Error::new("SECRET_SIZE_LIMIT"));
        }
        let version = crate::new_id();
        let directory = store.home.join("vault");
        if !crate::paths::entry_exists(&directory)? {
            std::fs::create_dir(&directory).map_err(|_| Error::new("VAULT_CREATE_FAILED"))?;
            crate::windows::restrict_home_acl(&directory)?;
        }
        crate::paths::directory(&directory)?;
        if std::fs::read_dir(&directory)
            .map_err(|_| Error::new("VAULT_ENUMERATION"))?
            .take(1025)
            .count()
            >= 1024
        {
            return Err(Error::new("VAULT_RETENTION_REVIEW_REQUIRED"));
        }
        let mut binding = BindingV2 {
            schema_version: 2,
            project: project.into(),
            environment: environment.into(),
            resource_id: id.into(),
            version_id: version.clone(),
            secret: secret.0.clone(),
        };
        let encoded = serde_json::to_vec(&binding);
        for b in &mut binding.secret {
            unsafe { std::ptr::write_volatile(b, 0) }
        }
        let plaintext = SecretBytes(encoded.map_err(|_| Error::new("SECRET_BINDING_ENCODING"))?);
        let cipher = crate::vault::native::protect(&plaintext.0)?;
        if cipher.len() > crate::vault::CIPHER_BYTES_LIMIT {
            return Err(Error::new("VAULT_CIPHER_LIMIT"));
        }
        let name = format!(
            "{}.dpapi",
            sha(format!("local-v2:{project}:{environment}:{id}:{version}").as_bytes())
        );
        let path = directory.join(&name);
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|_| Error::new("VERSION_FILE_EXISTS_OR_CREATE_FAILED"))?;
        f.write_all(&cipher)
            .and_then(|_| f.sync_all())
            .map_err(|_| Error::new("VAULT_WRITE_INCOMPLETE_PRESERVED"))?;
        store.effect_step(
            run,
            "ciphertext_version_written",
            "completed",
            &json!({"resource_id":id,"version_id":version,"ciphertext_sha256":sha(&cipher)}),
        )?;
        // Crash/DB failure leaves encrypted bytes, not a lost active value or overwritten file.
        store.record_secret_rotation(run, id, &version, &name, &sha(&cipher), expected)?;
        let confirm = crate::control_store::Record::ResourceConfirm {
            resource_id: id.into(),
        };
        let digest =
            sha(&serde_json::to_vec(&confirm).map_err(|_| Error::new("CONFIRMATION_ENCODING"))?);
        store.record(confirm, &digest)?;
        Ok(
            json!({"resource_id":id,"version_id":version,"state":"active","previous":expected,"old_ciphertext_preserved":true,"provider_token_rotated":false,"note":"Entered value changed locally. Rotate the real provider token separately through its authorized administration interface."}),
        )
    }
}
pub(crate) fn resolve(
    store: &Store,
    project: &str,
    environment: &str,
    id: &str,
) -> Result<SecretBytes> {
    local_resource(store, project, environment, id)?;
    if store.schema_version()? < 4 {
        return crate::vault::resolve_bound(&store.home, project, environment, id);
    }
    let Some((version, state, _)) = store.secret_head(id)? else {
        return crate::vault::resolve_bound(&store.home, project, environment, id);
    };
    if state != "active" {
        return Err(Error::new("LOCAL_SECRET_REVOKED_NO_LEGACY_FALLBACK"));
    }
    #[cfg(not(windows))]
    {
        let _ = version;
        Err(Error::new("DPAPI_WINDOWS_REQUIRED"))
    }
    #[cfg(windows)]
    {
        let(file,digest):(String,String)=store.conn.query_row("SELECT file_name,cipher_digest FROM secret_versions WHERE resource_id=?1 AND version_id=?2",rusqlite::params![id,version],|r|Ok((r.get(0)?,r.get(1)?))).map_err(|_|Error::new("VAULT_VERSION_METADATA"))?;
        if Path::new(&file).file_name().and_then(|s| s.to_str()) != Some(file.as_str())
            || !file.ends_with(".dpapi")
        {
            return Err(Error::new("VAULT_VERSION_PATH_INVALID"));
        }
        let path = store.home.join("vault").join(file);
        crate::paths::local_existing(&path)?;
        let mut cipher = vec![];
        std::fs::File::open(&path)
            .map_err(|_| Error::new("VAULT_VERSION_MISSING_REENTRY_REQUIRED"))?
            .take(crate::vault::CIPHER_BYTES_LIMIT as u64 + 1)
            .read_to_end(&mut cipher)
            .map_err(|_| Error::new("VAULT_VERSION_READ"))?;
        if cipher.len() > crate::vault::CIPHER_BYTES_LIMIT || sha(&cipher) != digest {
            return Err(Error::new("VAULT_VERSION_INTEGRITY"));
        }
        let plaintext = crate::vault::native::unprotect(&cipher)?;
        let mut b: BindingV2 = serde_json::from_slice(&plaintext.0)
            .map_err(|_| Error::new("VAULT_VERSION_BINDING"))?;
        let bytes = SecretBytes(std::mem::take(&mut b.secret));
        if bytes.0.is_empty() || bytes.0.len() > crate::vault::SECRET_BYTES_LIMIT {
            return Err(Error::new("VAULT_SECRET_SIZE_LIMIT"));
        }
        if b.schema_version != 2
            || b.project != project
            || b.environment != environment
            || b.resource_id != id
            || b.version_id != version
        {
            return Err(Error::new("VAULT_VERSION_SCOPE_MISMATCH"));
        }
        Ok(bytes)
    }
}
impl Store {
    pub fn secret_generation_digest(&self, project: &str, environment: &str) -> Result<String> {
        self.require_v4()?;
        let mut stmt=self.conn.prepare("SELECT h.resource_id,h.version_id,h.state,h.generation FROM secret_heads h JOIN resources r ON r.id=h.resource_id WHERE r.project_id=?1 AND r.environment=?2 ORDER BY h.resource_id LIMIT 1025").map_err(|_|Error::new("SECRET_GENERATION_QUERY"))?;
        let rows = stmt
            .query_map(rusqlite::params![project, environment], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                ))
            })
            .map_err(|_| Error::new("SECRET_GENERATION_QUERY"))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|_| Error::new("SECRET_GENERATION_QUERY"))?;
        if rows.len() > 1024 {
            return Err(Error::new("SECRET_GENERATION_LIMIT"));
        }
        if rows.iter().any(|row| row.3 < 0) {
            return Err(Error::new("SECRET_GENERATION_INVALID"));
        }
        Ok(sha(
            &serde_json::to_vec(&rows).map_err(|_| Error::new("SECRET_GENERATION_ENCODING"))?
        ))
    }
    pub fn recovery_inventory(&self, project: &str, environment: &str) -> Result<Value> {
        self.require_v4()?;
        let resources = self.resources(project, environment)?;
        let mut result = vec![];
        for r in resources
            .into_iter()
            .filter(|r| r.kind == workstation_core::control::ResourceKind::SecretReference)
        {
            let head = self.secret_head(&r.id)?;
            let status = if !r.locator.starts_with("local://") {
                "provider_reauthentication_may_be_required"
            } else if head.as_ref().is_some_and(|x| x.1 == "revoked") {
                "revoked"
            } else {
                "decryptability_not_tested_reentry_may_be_required"
            };
            result.push(json!({"resource_id":r.id,"metadata_present":true,"status":status,"head":head,"value_read":false}));
        }
        Ok(
            json!({"resources":result,"portable_credential_backup":false,"decryption_performed":false}),
        )
    }
}
