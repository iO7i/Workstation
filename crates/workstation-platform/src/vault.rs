//! Local user-scoped DPAPI only. Not isolation against malicious software under the same user.
//! No reveal command, model tool, generic process runner or plaintext logging.
#[cfg(windows)]
use crate::paths;
use crate::{storage::Store, Error, Result};
#[cfg(windows)]
use serde::{Deserialize, Serialize};
use std::path::Path;
use workstation_core::control::{self, Resource};
#[cfg(windows)]
use workstation_core::control::{Evidence, EvidenceKind, ResourceKind};
#[cfg(windows)]
use workstation_core::Coverage;
pub(crate) const SECRET_BYTES_LIMIT: usize = 16 * 1024;
// A Vec<u8> serialized as decimal JSON can occupy four bytes per input byte,
// before binding fields and DPAPI overhead. Read and write bounds must agree.
pub(crate) const CIPHER_BYTES_LIMIT: usize = 128 * 1024;
pub struct SecretBytes(pub(crate) Vec<u8>);
impl Drop for SecretBytes {
    fn drop(&mut self) {
        for b in &mut self.0 {
            unsafe { std::ptr::write_volatile(b, 0) }
        }
        std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
    }
}
#[cfg(windows)]
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CipherBinding {
    schema_version: u32,
    project: String,
    environment: String,
    id: String,
    secret: Vec<u8>,
}
#[cfg(windows)]
pub(crate) mod native {
    use super::*;
    use windows_sys::Win32::{Foundation::LocalFree, Security::Cryptography::*};
    const FLAGS: u32 = CRYPTPROTECT_UI_FORBIDDEN; // deliberately NOT CRYPTPROTECT_LOCAL_MACHINE
    pub fn protect(bytes: &[u8]) -> Result<Vec<u8>> {
        let input = CRYPT_INTEGER_BLOB {
            cbData: bytes.len() as u32,
            pbData: bytes.as_ptr() as *mut u8,
        };
        let mut output: CRYPT_INTEGER_BLOB = unsafe { std::mem::zeroed() };
        if unsafe {
            CryptProtectData(
                &input,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null(),
                FLAGS,
                &mut output,
            )
        } == 0
        {
            return Err(Error::new("DPAPI_ENCRYPT_FAILED"));
        }
        let result =
            unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize) }.to_vec();
        unsafe {
            LocalFree(output.pbData as *mut _);
        }
        Ok(result)
    }
    pub fn unprotect(bytes: &[u8]) -> Result<SecretBytes> {
        let input = CRYPT_INTEGER_BLOB {
            cbData: bytes.len() as u32,
            pbData: bytes.as_ptr() as *mut u8,
        };
        let mut output: CRYPT_INTEGER_BLOB = unsafe { std::mem::zeroed() };
        if unsafe {
            CryptUnprotectData(
                &input,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null(),
                FLAGS,
                &mut output,
            )
        } == 0
        {
            return Err(Error::new("DPAPI_DECRYPT_FAILED"));
        }
        let result = SecretBytes(
            unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize) }.to_vec(),
        );
        unsafe {
            for i in 0..output.cbData as usize {
                std::ptr::write_volatile(output.pbData.add(i), 0);
            }
            LocalFree(output.pbData as *mut _);
        }
        Ok(result)
    }
    pub fn hidden_console() -> Result<SecretBytes> {
        use windows_sys::Win32::Foundation::HANDLE;
        use windows_sys::Win32::System::Console::*;
        struct Restore(HANDLE, u32);
        impl Drop for Restore {
            fn drop(&mut self) {
                unsafe {
                    SetConsoleMode(self.0, self.1);
                }
            }
        }
        let handle = unsafe { GetStdHandle(STD_INPUT_HANDLE) };
        let mut mode = 0;
        if unsafe { GetConsoleMode(handle, &mut mode) } == 0 {
            return Err(Error::new("INTERACTIVE_CONSOLE_REQUIRED"));
        }
        if unsafe {
            SetConsoleMode(
                handle,
                (mode | ENABLE_LINE_INPUT | ENABLE_PROCESSED_INPUT) & !ENABLE_ECHO_INPUT,
            )
        } == 0
        {
            return Err(Error::new("CONSOLE_PRIVACY_MODE_FAILED"));
        }
        let _restore = Restore(handle, mode);
        let mut wide = vec![0u16; 8192];
        let mut read = 0;
        let ok = unsafe {
            ReadConsoleW(
                handle,
                wide.as_mut_ptr().cast(),
                wide.len() as u32,
                &mut read,
                std::ptr::null(),
            )
        };
        let text = if ok != 0 && read > 0 && read < (wide.len() as u32) {
            String::from_utf16(&wide[..read as usize]).ok()
        } else {
            None
        };
        for x in &mut wide {
            unsafe {
                std::ptr::write_volatile(x, 0);
            }
        }
        let string = text.ok_or_else(|| Error::new("SECRET_INPUT_FAILED_OR_LIMIT"))?;
        let mut bytes = SecretBytes(string.into_bytes());
        while matches!(bytes.0.last(), Some(b'\n' | b'\r')) {
            bytes.0.pop();
        }
        if bytes.0.is_empty() || bytes.0.len() > SECRET_BYTES_LIMIT {
            return Err(Error::new("SECRET_INPUT_LIMIT"));
        }
        Ok(bytes)
    }
}
#[cfg(windows)]
pub fn read_hidden() -> Result<SecretBytes> {
    native::hidden_console()
}
#[cfg(not(windows))]
pub fn read_hidden() -> Result<SecretBytes> {
    Err(Error::new("DPAPI_WINDOWS_REQUIRED"))
}
/// Explicit local operation. New immutable ciphertext IDs only; no overwrite or rotation claim.
pub fn put(
    store: &mut Store,
    project: &str,
    environment: &str,
    id: &str,
    secret: SecretBytes,
) -> Result<Resource> {
    store.require_control()?;
    control::id(project).map_err(Error::new)?;
    control::id(environment).map_err(Error::new)?;
    control::id(id).map_err(Error::new)?;
    if secret.0.is_empty() || secret.0.len() > SECRET_BYTES_LIMIT {
        return Err(Error::new("SECRET_SIZE_LIMIT"));
    }
    // Exact environment must exist; no fallback.
    let exists: i64 = store
        .conn
        .query_row(
            "SELECT count(*) FROM environments WHERE project_id=?1 AND name=?2",
            rusqlite::params![project, environment],
            |r| r.get(0),
        )
        .map_err(|_| Error::new("DATABASE_OPERATION_FAILED"))?;
    if exists != 1 {
        return Err(Error::new("EXPLICIT_ENVIRONMENT_REQUIRED"));
    }
    #[cfg(not(windows))]
    {
        let _ = (project, environment, id, secret);
        Err(Error::new("DPAPI_WINDOWS_REQUIRED"))
    }
    #[cfg(windows)]
    {
        use std::{fs, io::Write};
        let dir = store.home.join("vault");
        if !paths::entry_exists(&dir)? {
            fs::create_dir(&dir).map_err(|_| Error::new("VAULT_CREATE_FAILED"))?;
            crate::windows::restrict_home_acl(&dir)?;
        }
        paths::directory(&dir)?;
        if fs::read_dir(&dir)
            .map_err(|_| Error::new("VAULT_READ_FAILED"))?
            .take(257)
            .count()
            >= 256
        {
            return Err(Error::new("VAULT_COUNT_LIMIT"));
        }
        let binding = CipherBinding {
            schema_version: 1,
            project: project.into(),
            environment: environment.into(),
            id: id.into(),
            secret: secret.0.clone(),
        };
        let mut binding = binding;
        let plaintext_result = serde_json::to_vec(&binding);
        for b in &mut binding.secret {
            unsafe {
                std::ptr::write_volatile(b, 0);
            }
        }
        let plaintext =
            SecretBytes(plaintext_result.map_err(|_| Error::new("SECRET_ENCODING_FAILED"))?);
        let ciphertext = native::protect(&plaintext.0)?;
        if ciphertext.len() > CIPHER_BYTES_LIMIT {
            return Err(Error::new("VAULT_CIPHER_LIMIT"));
        }
        let locator = format!("local://{project}/{environment}/{id}");
        let name = crate::control_store::sha(locator.as_bytes());
        let path = dir.join(format!("{name}.dpapi"));
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|_| Error::new("VAULT_EXISTS_OR_CREATE_FAILED"))?;
        f.write_all(&ciphertext)
            .and_then(|_| f.sync_all())
            .map_err(|_| Error::new("VAULT_WRITE_FAILED"))?;
        let resource = Resource {
            id: id.into(),
            project_id: project.into(),
            environment: environment.into(),
            kind: ResourceKind::SecretReference,
            name: id.into(),
            locator: locator.clone(),
            provider: Some("local_dpapi".into()),
            evidence: Evidence {
                kind: EvidenceKind::UserApproved,
                source: "local_interactive_vault_entry".into(),
                at: crate::control_store::epoch(),
                coverage: Coverage::Complete,
            },
            fresh_for_secs: 86400 * 30,
        };
        let command = crate::control_store::Record::ResourceAdd {
            id: id.into(),
            project_id: project.into(),
            environment: environment.into(),
            kind: ResourceKind::SecretReference,
            name: id.into(),
            locator: locator.clone(),
            provider: Some("local_dpapi".into()),
            fresh_for_secs: 86400 * 30,
        };
        let digest = crate::control_store::sha(
            &serde_json::to_vec(&command).map_err(|_| Error::new("SERIALIZE_FAILED"))?,
        );
        // If metadata commit fails, preserve ciphertext; do not delete it or overwrite other state.
        store.record(command, &digest)?;
        let confirm = crate::control_store::Record::ResourceConfirm {
            resource_id: id.into(),
        };
        let confirm_digest = crate::control_store::sha(
            &serde_json::to_vec(&confirm).map_err(|_| Error::new("SERIALIZE_FAILED"))?,
        );
        store.record(confirm, &confirm_digest)?;
        Ok(resource)
    }
}
/// Used only by future certified tasks and Windows round-trip tests, not exposed over CLI/MCP.
pub fn resolve_bound(
    home: &Path,
    project: &str,
    environment: &str,
    id: &str,
) -> Result<SecretBytes> {
    control::id(project).map_err(Error::new)?;
    control::id(environment).map_err(Error::new)?;
    control::id(id).map_err(Error::new)?;
    #[cfg(not(windows))]
    {
        let _ = home;
        Err(Error::new("DPAPI_WINDOWS_REQUIRED"))
    }
    #[cfg(windows)]
    {
        use std::io::Read;
        let locator = format!("local://{project}/{environment}/{id}");
        let name = crate::control_store::sha(locator.as_bytes());
        let path = home.join("vault").join(format!("{name}.dpapi"));
        paths::local_existing(&path)?;
        let mut bytes = Vec::new();
        std::fs::File::open(path)
            .map_err(|_| Error::new("VAULT_UNAVAILABLE"))?
            .take(CIPHER_BYTES_LIMIT as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| Error::new("VAULT_UNREADABLE"))?;
        if bytes.len() > CIPHER_BYTES_LIMIT {
            return Err(Error::new("VAULT_CIPHER_LIMIT"));
        }
        let decoded = native::unprotect(&bytes)?;
        let mut binding: CipherBinding =
            serde_json::from_slice(&decoded.0).map_err(|_| Error::new("VAULT_BINDING_INVALID"))?;
        let value = SecretBytes(std::mem::take(&mut binding.secret));
        if value.0.is_empty() || value.0.len() > SECRET_BYTES_LIMIT {
            return Err(Error::new("VAULT_SECRET_SIZE_LIMIT"));
        }
        if binding.schema_version != 1
            || binding.project != project
            || binding.environment != environment
            || binding.id != id
        {
            return Err(Error::new("VAULT_ENVIRONMENT_BINDING_MISMATCH"));
        }
        Ok(value)
    }
}
#[cfg(all(test, windows))]
mod tests {
    use super::*;
    #[test]
    fn dpapi_roundtrip() {
        let input = SecretBytes(b"roundtrip-test-only".to_vec());
        let encrypted = native::protect(&input.0).unwrap();
        assert_ne!(encrypted, input.0);
        let output = native::unprotect(&encrypted).unwrap();
        assert_eq!(output.0, input.0);
    }
    #[test]
    fn dpapi_tamper_rejected() {
        let mut c = native::protect(b"test").unwrap();
        let n = c.len() / 2;
        c[n] ^= 0x44;
        assert!(native::unprotect(&c).is_err());
    }
}

#[cfg(all(test, windows))]
mod audit_cipher_size_tests {
    use super::*;
    #[test]
    fn max_secret_is_within_cipher_read_contract() {
        let b = CipherBinding {
            schema_version: 1,
            project: "p".into(),
            environment: "production".into(),
            id: "s".into(),
            secret: vec![255; SECRET_BYTES_LIMIT],
        };
        let bytes = serde_json::to_vec(&b).unwrap();
        assert!(bytes.len() > 64 * 1024);
        let cipher = native::protect(&bytes).unwrap();
        assert!(cipher.len() <= CIPHER_BYTES_LIMIT);
        assert_eq!(native::unprotect(&cipher).unwrap().0, bytes);
    }
}
