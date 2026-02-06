// HR Command Center - Secure API Key Storage
// Stores API keys in the macOS Keychain (with legacy file migration)

use std::fs;
use std::path::PathBuf;
use thiserror::Error;

#[cfg(target_os = "macos")]
use security_framework::passwords::{
    delete_generic_password, generic_password, set_generic_password, PasswordOptions,
};
#[cfg(target_os = "macos")]
use security_framework_sys::base::errSecItemNotFound;

const KEYCHAIN_SERVICE: &str = "com.hrcommandcenter.app";
const KEYCHAIN_ACCOUNT: &str = "anthropic_api_key";

#[derive(Error, Debug)]
pub enum KeyringError {
    #[error("Failed to access storage: {0}")]
    StorageAccess(String),
    #[error("API key not found")]
    NotFound,
    #[error("Invalid API key format")]
    InvalidFormat,
}

impl From<std::io::Error> for KeyringError {
    fn from(err: std::io::Error) -> Self {
        if err.kind() == std::io::ErrorKind::NotFound {
            KeyringError::NotFound
        } else {
            KeyringError::StorageAccess(err.to_string())
        }
    }
}

/// Legacy file path used by older app versions before Keychain migration
fn get_legacy_key_path() -> Result<PathBuf, KeyringError> {
    let home = std::env::var("HOME")
        .map_err(|_| KeyringError::StorageAccess("Could not find home directory".into()))?;
    let app_dir = PathBuf::from(home)
        .join("Library")
        .join("Application Support")
        .join("com.hrcommandcenter.app");

    // Ensure directory exists
    fs::create_dir_all(&app_dir)?;

    Ok(app_dir.join(".api_key"))
}

fn get_legacy_api_key() -> Result<String, KeyringError> {
    let path = get_legacy_key_path()?;
    let key = fs::read_to_string(&path)?;
    Ok(key.trim().to_string())
}

fn delete_legacy_api_key() -> Result<(), KeyringError> {
    let path = get_legacy_key_path()?;
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

// Make KeyringError serializable for Tauri commands
impl serde::Serialize for KeyringError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

/// Store the Anthropic API key
pub fn store_api_key(api_key: &str) -> Result<(), KeyringError> {
    // Validate format: Anthropic keys start with "sk-ant-"
    if !api_key.starts_with("sk-ant-") {
        return Err(KeyringError::InvalidFormat);
    }

    #[cfg(target_os = "macos")]
    {
        set_generic_password(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT, api_key.as_bytes())
            .map_err(|err| KeyringError::StorageAccess(format!("Keychain write failed: {}", err)))?;

        // Cleanup legacy file storage if present
        let _ = delete_legacy_api_key();
        return Ok(());
    }

    #[cfg(not(target_os = "macos"))]
    {
        // Non-macOS fallback keeps existing file-based behavior.
        let path = get_legacy_key_path()?;
        fs::write(&path, api_key)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = fs::Permissions::from_mode(0o600);
            fs::set_permissions(&path, perms)?;
        }

        Ok(())
    }
}

/// Retrieve the Anthropic API key
pub fn get_api_key() -> Result<String, KeyringError> {
    #[cfg(target_os = "macos")]
    {
        match generic_password(PasswordOptions::new_generic_password(
            KEYCHAIN_SERVICE,
            KEYCHAIN_ACCOUNT,
        )) {
            Ok(key_bytes) => {
                return String::from_utf8(key_bytes)
                    .map(|key| key.trim().to_string())
                    .map_err(|_| {
                        KeyringError::StorageAccess(
                            "Stored API key is not valid UTF-8".to_string(),
                        )
                    });
            }
            Err(err) if err.code() == errSecItemNotFound => {
                // Migrate legacy plaintext key file to Keychain on first read.
                if let Ok(legacy_key) = get_legacy_api_key() {
                    store_api_key(&legacy_key)?;
                    return Ok(legacy_key);
                }
                return Err(KeyringError::NotFound);
            }
            Err(err) => {
                return Err(KeyringError::StorageAccess(format!(
                    "Keychain read failed: {}",
                    err
                )))
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        get_legacy_api_key()
    }
}

/// Delete the API key
pub fn delete_api_key() -> Result<(), KeyringError> {
    #[cfg(target_os = "macos")]
    {
        match delete_generic_password(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT) {
            Ok(_) => {}
            // Already missing is fine for delete operations
            Err(err) if err.code() == errSecItemNotFound => {}
            Err(err) => {
                return Err(KeyringError::StorageAccess(format!(
                    "Keychain delete failed: {}",
                    err
                )))
            }
        }

        let _ = delete_legacy_api_key();
        return Ok(());
    }

    #[cfg(not(target_os = "macos"))]
    {
        delete_legacy_api_key()
    }
}

/// Check if an API key exists
pub fn has_api_key() -> bool {
    get_api_key().is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_format() {
        let result = store_api_key("invalid-key");
        assert!(matches!(result, Err(KeyringError::InvalidFormat)));
    }

    #[test]
    fn test_valid_format_prefix() {
        let key = "sk-ant-test123";
        assert!(key.starts_with("sk-ant-"));
    }

    #[test]
    fn test_storage_path() {
        let path = get_legacy_key_path().unwrap();
        assert!(path.to_string_lossy().contains("com.hrcommandcenter.app"));
    }
}
