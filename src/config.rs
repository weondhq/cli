use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use zeroize::Zeroize;

#[derive(Debug, Clone, Serialize, Deserialize, Zeroize)]
#[zeroize(drop)]
pub struct StoredToken {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: i64,
    #[serde(default)]
    pub refresh_token: String,
}

pub fn token_file_path() -> Result<PathBuf> {
    let base = dirs::config_dir().context("could not determine user configuration directory")?;
    Ok(base.join("weoc").join("token.json"))
}

pub fn save_token(token: &StoredToken) -> Result<()> {
    let path = token_file_path()?;
    save_token_at(&path, token)
}

pub fn save_token_at(path: &Path, token: &StoredToken) -> Result<()> {
    let dir = path
        .parent()
        .context("token path has no parent directory")?;
    fs::create_dir_all(dir)
        .with_context(|| format!("failed to create config directory {}", dir.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))
            .with_context(|| format!("failed to set 0700 permissions on {}", dir.display()))?;
    }

    let payload = serde_json::to_vec_pretty(token).context("failed to serialize token")?;

    let mut opts = OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }

    let mut file = opts
        .open(path)
        .with_context(|| format!("failed to open {} for writing", path.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .with_context(|| format!("failed to enforce 0600 permissions on {}", path.display()))?;
    }

    file.write_all(&payload)
        .with_context(|| format!("failed to write token to {}", path.display()))?;
    file.sync_all()
        .with_context(|| format!("failed to flush {}", path.display()))?;
    Ok(())
}

pub fn load_token() -> Result<StoredToken> {
    let path = token_file_path()?;
    load_token_from(&path)
}

pub fn load_token_from(path: &Path) -> Result<StoredToken> {
    let raw =
        fs::read(path).with_context(|| format!("credentials not found at {}", path.display()))?;
    let token: StoredToken =
        serde_json::from_slice(&raw).context("stored credentials file is malformed")?;
    if token.access_token.trim().is_empty() {
        anyhow::bail!("stored access token is empty");
    }
    Ok(token)
}

pub fn remove_token() -> Result<bool> {
    let path = token_file_path()?;
    match fs::remove_file(&path) {
        Ok(()) => Ok(true),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(err).with_context(|| format!("failed to remove {}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn saves_and_loads_token_with_strict_permissions() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("weoc").join("token.json");
        let tok = StoredToken {
            access_token: "test_secret_token_1234567890".into(),
            token_type: "Bearer".into(),
            expires_in: 2_592_000,
            refresh_token: String::new(),
        };
        save_token_at(&path, &tok).unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let file_mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            let dir_mode = fs::metadata(path.parent().unwrap())
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(file_mode, 0o600);
            assert_eq!(dir_mode, 0o700);
        }

        let loaded = load_token_from(&path).unwrap();
        assert_eq!(loaded.access_token, tok.access_token);
        assert_eq!(loaded.token_type, "Bearer");
    }
}
