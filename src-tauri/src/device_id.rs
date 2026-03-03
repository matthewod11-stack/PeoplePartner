// People Partner - Device ID Module
// Generates and persists a stable UUID v4 per installation for trial quota tracking.

use crate::db::DbPool;
use crate::settings;

const DEVICE_ID_KEY: &str = "device_id";

/// Get or create a stable device ID for this installation.
/// Generates a UUID v4 on first call, stores in the settings table,
/// and returns the same value on all subsequent calls.
pub async fn get_or_create_device_id(pool: &DbPool) -> Result<String, settings::SettingsError> {
    if let Some(id) = settings::get_setting(pool, DEVICE_ID_KEY).await? {
        return Ok(id);
    }

    let id = uuid::Uuid::new_v4().to_string();
    settings::set_setting(pool, DEVICE_ID_KEY, &id).await?;
    Ok(id)
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_uuid_v4_format() {
        let id = uuid::Uuid::new_v4().to_string();
        assert_eq!(id.len(), 36);
        // UUID v4 format: 8-4-4-4-12, version nibble is '4'
        let parts: Vec<&str> = id.split('-').collect();
        assert_eq!(parts.len(), 5);
        assert_eq!(parts[0].len(), 8);
        assert_eq!(parts[1].len(), 4);
        assert_eq!(parts[2].len(), 4);
        assert!(parts[2].starts_with('4'));
        assert_eq!(parts[3].len(), 4);
        assert_eq!(parts[4].len(), 12);
    }
}
