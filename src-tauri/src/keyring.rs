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
#[allow(dead_code)]
const KEYCHAIN_ACCOUNT: &str = "anthropic_api_key";

/// Get the Keychain account name for a provider
fn keychain_account_for_provider(provider_id: &str) -> String {
    format!("{}_api_key", provider_id)
}

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

// --- Per-provider API key functions (generic, no format validation) ---

/// Store an API key for a specific provider (no format validation — caller validates)
pub fn store_provider_api_key(provider_id: &str, api_key: &str) -> Result<(), KeyringError> {
    let account = keychain_account_for_provider(provider_id);

    #[cfg(target_os = "macos")]
    {
        set_generic_password(KEYCHAIN_SERVICE, &account, api_key.as_bytes())
            .map_err(|err| KeyringError::StorageAccess(format!("Keychain write failed: {}", err)))?;
        return Ok(());
    }

    #[cfg(not(target_os = "macos"))]
    {
        let home = std::env::var("HOME")
            .map_err(|_| KeyringError::StorageAccess("Could not find home directory".into()))?;
        let app_dir = PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join("com.hrcommandcenter.app");
        fs::create_dir_all(&app_dir)?;
        let path = app_dir.join(format!(".{}", account));
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

/// Retrieve the API key for a specific provider
pub fn get_provider_api_key(provider_id: &str) -> Result<String, KeyringError> {
    let account = keychain_account_for_provider(provider_id);

    #[cfg(target_os = "macos")]
    {
        match generic_password(PasswordOptions::new_generic_password(KEYCHAIN_SERVICE, &account)) {
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
                return Err(KeyringError::NotFound);
            }
            Err(err) => {
                return Err(KeyringError::StorageAccess(format!(
                    "Keychain read failed: {}",
                    err
                )));
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        let home = std::env::var("HOME")
            .map_err(|_| KeyringError::StorageAccess("Could not find home directory".into()))?;
        let path = PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join("com.hrcommandcenter.app")
            .join(format!(".{}", account));
        let key = fs::read_to_string(&path)?;
        Ok(key.trim().to_string())
    }
}

/// Delete the API key for a specific provider
pub fn delete_provider_api_key(provider_id: &str) -> Result<(), KeyringError> {
    let account = keychain_account_for_provider(provider_id);

    #[cfg(target_os = "macos")]
    {
        match delete_generic_password(KEYCHAIN_SERVICE, &account) {
            Ok(_) => {}
            Err(err) if err.code() == errSecItemNotFound => {}
            Err(err) => {
                return Err(KeyringError::StorageAccess(format!(
                    "Keychain delete failed: {}",
                    err
                )));
            }
        }
        return Ok(());
    }

    #[cfg(not(target_os = "macos"))]
    {
        let home = std::env::var("HOME")
            .map_err(|_| KeyringError::StorageAccess("Could not find home directory".into()))?;
        let path = PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join("com.hrcommandcenter.app")
            .join(format!(".{}", account));
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }
}

/// Check if an API key exists for a specific provider
pub fn has_provider_api_key(provider_id: &str) -> bool {
    get_provider_api_key(provider_id).is_ok()
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

/// Store the Anthropic API key (validates sk-ant- prefix, delegates to per-provider storage)
pub fn store_api_key(api_key: &str) -> Result<(), KeyringError> {
    // Validate format: Anthropic keys start with "sk-ant-"
    if !api_key.starts_with("sk-ant-") {
        return Err(KeyringError::InvalidFormat);
    }

    store_provider_api_key("anthropic", api_key)?;

    // Cleanup legacy file storage if present
    let _ = delete_legacy_api_key();

    Ok(())
}

/// Retrieve the Anthropic API key (with legacy file migration)
pub fn get_api_key() -> Result<String, KeyringError> {
    // First try the generic provider path
    match get_provider_api_key("anthropic") {
        Ok(key) => return Ok(key),
        Err(KeyringError::NotFound) => {
            // Legacy migration: try old file-based storage
            if let Ok(legacy_key) = get_legacy_api_key() {
                // store_api_key validates and stores in keychain, cleans up legacy file
                store_api_key(&legacy_key)?;
                return Ok(legacy_key);
            }
            Err(KeyringError::NotFound)
        }
        Err(other) => Err(other),
    }
}

/// Delete the Anthropic API key (delegates to per-provider, plus legacy cleanup)
pub fn delete_api_key() -> Result<(), KeyringError> {
    delete_provider_api_key("anthropic")?;
    let _ = delete_legacy_api_key();
    Ok(())
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

    #[test]
    fn test_keychain_account_for_provider() {
        assert_eq!(keychain_account_for_provider("anthropic"), "anthropic_api_key");
        assert_eq!(keychain_account_for_provider("openai"), "openai_api_key");
        assert_eq!(keychain_account_for_provider("gemini"), "gemini_api_key");
    }
}
