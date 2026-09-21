use crate::{Error, Result};
use std::{
    fs,
    path::{Component, Path, PathBuf},
};

/// Reject lexical ambiguity before performing filesystem lookups.
pub fn validate_lexical(path: &Path) -> Result<()> {
    let text = path
        .to_str()
        .ok_or_else(|| Error::new("PATH_ENCODING_UNSUPPORTED"))?;
    if text.len() > 16_384 || text.chars().any(|c| c.is_control()) || !path.is_absolute() {
        return Err(Error::new("ABSOLUTE_LOCAL_PATH_REQUIRED"));
    }
    if path.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(Error::new("PATH_TRAVERSAL_REJECTED"));
    }
    #[cfg(windows)]
    {
        // Accept explicit drive-rooted paths only; no UNC, devices, ADS, or verbatim paths.
        let b = text.as_bytes();
        if b.len() < 3
            || !b[0].is_ascii_alphabetic()
            || b[1] != b':'
            || !matches!(b[2], b'\\' | b'/')
            || text[2..].contains(':')
        {
            return Err(Error::new("LOCAL_DRIVE_PATH_REQUIRED"));
        }
        for c in path.components() {
            if let Component::Normal(n) = c {
                let n = n.to_string_lossy();
                if n.ends_with('.') || n.ends_with(' ') {
                    return Err(Error::new("AMBIGUOUS_WINDOWS_PATH"));
                }
            }
        }
    }
    Ok(())
}

pub fn is_redirect_or_placeholder(m: &fs::Metadata) -> bool {
    if m.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        // REPARSE_POINT | OFFLINE | RECALL_ON_OPEN | RECALL_ON_DATA_ACCESS.
        m.file_attributes() & (0x400 | 0x1000 | 0x40000 | 0x400000) != 0
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Validate every existing component. The same-user malicious race threat remains out of scope.
pub fn local_existing(path: &Path) -> Result<PathBuf> {
    validate_lexical(path)?;
    let mut cur = PathBuf::new();
    for c in path.components() {
        cur.push(c.as_os_str());
        // A bare Windows prefix (D:) is drive-relative until RootDir is appended.
        if matches!(c, Component::Prefix(_)) {
            continue;
        }
        let m = fs::symlink_metadata(&cur).map_err(|_| Error::new("PATH_UNAVAILABLE"))?;
        if is_redirect_or_placeholder(&m) {
            return Err(Error::new("PATH_REDIRECTION_BLOCKED"));
        }
    }
    #[cfg(windows)]
    crate::windows::validate_local_ntfs(path)?;
    // Preserve normal drive syntax instead of introducing \\?\ canonical names.
    Ok(path.to_path_buf())
}

pub fn directory(path: &Path) -> Result<PathBuf> {
    local_existing(path)?;
    if !fs::symlink_metadata(path)
        .map_err(|_| Error::new("PATH_UNAVAILABLE"))?
        .is_dir()
    {
        return Err(Error::new("DIRECTORY_REQUIRED"));
    }
    Ok(path.to_path_buf())
}

/// Only NotFound proves absence. A dangling link is an existing entry.
pub fn entry_exists(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err(Error::new("PATH_EXISTENCE_UNKNOWN")),
    }
}

pub fn require_absent(path: &Path) -> Result<()> {
    validate_lexical(path)?;
    if entry_exists(path)? {
        return Err(Error::new("TARGET_ALREADY_EXISTS"));
    }
    Ok(())
}

pub fn prospective_home(path: &Path) -> Result<()> {
    validate_lexical(path)?;
    if path.parent().is_none() || path.file_name().is_none() {
        return Err(Error::new("DRIVE_ROOT_NOT_ALLOWED"));
    }
    let parent = path.parent().ok_or_else(|| Error::new("PARENT_REQUIRED"))?;
    directory(parent)?;
    for p in parent.ancestors() {
        if entry_exists(&p.join(".git"))? {
            return Err(Error::new("HOME_INSIDE_REPOSITORY"));
        }
        if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
            let n = name.to_ascii_lowercase();
            if n.starts_with("onedrive")
                || ["dropbox", "google drive", "googledrive", "icloud drive"].contains(&n.as_str())
            {
                return Err(Error::new("CLOUD_SYNC_HOME_BLOCKED"));
            }
        }
    }
    for key in [
        "OneDrive",
        "OneDriveCommercial",
        "OneDriveConsumer",
        "DROPBOX_ROOT",
    ] {
        if let Some(cloud) = std::env::var_os(key) {
            if overlaps(path, &PathBuf::from(cloud)) {
                return Err(Error::new("CLOUD_SYNC_HOME_BLOCKED"));
            }
        }
    }
    if entry_exists(path)? {
        return Err(Error::new("HOME_ALREADY_EXISTS"));
    }
    Ok(())
}

pub fn overlaps(a: &Path, b: &Path) -> bool {
    let key = |p: &Path| {
        let s = p
            .to_string_lossy()
            .replace('\\', "/")
            .trim_end_matches('/')
            .to_string();
        if cfg!(windows) {
            s.to_lowercase()
        } else {
            s
        }
    };
    let a = key(a);
    let b = key(b);
    a == b || a.starts_with(&(b.clone() + "/")) || b.starts_with(&(a + "/"))
}

/// Only explicit CLI/environment locator selection in R0; never a default C: fallback.
pub fn select_home(
    explicit: Option<PathBuf>,
    environment: Option<std::ffi::OsString>,
) -> Result<PathBuf> {
    let home = explicit
        .or_else(|| environment.map(PathBuf::from))
        .ok_or_else(|| Error::new("HOME_NOT_CONFIGURED"))?;
    directory(&home).map_err(|_| Error::new("HOME_UNAVAILABLE"))?;
    Ok(home)
}

/// Reject special files before opening a bounded read. Bounds do not stop FIFO/device opens.
pub fn regular_file(path: &Path) -> Result<()> {
    local_existing(path)?;
    let metadata =
        fs::symlink_metadata(path).map_err(|_| Error::new("FILE_METADATA_UNAVAILABLE"))?;
    if !metadata.is_file() {
        return Err(Error::new("REGULAR_FILE_REQUIRED"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn relative_and_parent_paths_rejected() {
        assert!(validate_lexical(Path::new("relative")).is_err());
        let t = tempfile::tempdir().unwrap();
        assert!(validate_lexical(&t.path().join("../escape")).is_err());
    }
    #[test]
    fn only_missing_entry_is_absent() {
        let t = tempfile::tempdir().unwrap();
        assert!(!entry_exists(&t.path().join("missing")).unwrap());
        assert!(entry_exists(t.path()).unwrap());
    }
    #[cfg(unix)]
    #[test]
    fn dangling_link_is_not_absent() {
        let t = tempfile::tempdir().unwrap();
        let p = t.path().join("link");
        std::os::unix::fs::symlink(t.path().join("missing"), &p).unwrap();
        assert!(entry_exists(&p).unwrap());
        assert!(require_absent(&p).is_err());
    }
    #[test]
    fn unavailable_home_never_created() {
        let t = tempfile::tempdir().unwrap();
        let p = t.path().join("missing");
        assert!(select_home(Some(p.clone()), None).is_err());
        assert!(!p.exists());
    }
    #[test]
    fn explicit_home_wins() {
        let t = tempfile::tempdir().unwrap();
        assert_eq!(
            select_home(Some(t.path().to_path_buf()), Some("missing".into())).unwrap(),
            t.path()
        );
    }
    #[test]
    fn roots_must_not_overlap() {
        let t = tempfile::tempdir().unwrap();
        assert!(overlaps(t.path(), &t.path().join("a")));
        assert!(!overlaps(&t.path().join("a"), &t.path().join("ab")));
    }
    #[test]
    fn existing_home_not_overwritten() {
        let t = tempfile::tempdir().unwrap();
        assert!(prospective_home(t.path()).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn symlink_component_blocked() {
        let t = tempfile::tempdir().unwrap();
        let link = t.path().join("link");
        std::os::unix::fs::symlink(t.path(), &link).unwrap();
        assert!(local_existing(&link).is_err());
    }
}

#[cfg(all(test, unix))]
mod audit_regular_file_tests {
    use super::*;
    #[test]
    fn directory_is_not_regular_input() {
        let t = tempfile::tempdir().unwrap();
        assert!(regular_file(t.path()).is_err());
    }
}
