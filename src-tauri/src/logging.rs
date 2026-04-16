use std::fs;
use std::io::Write;
use std::path::Path;
use tauri::plugin::TauriPlugin;
use tauri::Runtime;
use tauri_plugin_log::{Builder, RotationStrategy, Target, TargetKind};

// 5 MB — matches the per-file cap from issue #30.
const MAX_FILE_SIZE: u128 = 5_000_000;

const LOG_FILE_NAME: &str = "people-partner";

pub fn plugin<R: Runtime>() -> TauriPlugin<R> {
    let mut builder = Builder::default()
        .clear_targets()
        .target(Target::new(TargetKind::LogDir {
            file_name: Some(LOG_FILE_NAME.to_string()),
        }))
        .max_file_size(MAX_FILE_SIZE)
        .rotation_strategy(RotationStrategy::KeepAll)
        .level(log::LevelFilter::Info);

    // Mirror to stdout only in dev builds — Finder-launched .app discards stdout,
    // so keeping it on in release adds nothing useful and wastes a write syscall
    // per log call.
    if cfg!(debug_assertions) {
        builder = builder
            .target(Target::new(TargetKind::Stdout))
            .level(log::LevelFilter::Debug);
    }

    builder.build()
}

/// Writes a standalone crash report next to the app data dir. Used when the app
/// is about to `exit(1)` during startup — at that point tauri-plugin-log's sink
/// may not have flushed, and Finder-launched apps discard stderr. A separate
/// file gives users something to email to support.
pub fn write_crash_file(app_data_dir: &Path, error_context: &str, message: &str) {
    if let Err(e) = fs::create_dir_all(app_data_dir) {
        eprintln!("Could not create app data dir for crash file: {e}");
        return;
    }

    let timestamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
    let crash_path = app_data_dir.join(format!("crash-{timestamp}.txt"));

    let body = format!(
        "People Partner crash report\n\
         ---\n\
         Timestamp (UTC): {}\n\
         Context: {}\n\
         Error:\n{}\n",
        chrono::Utc::now().to_rfc3339(),
        error_context,
        message,
    );

    match fs::File::create(&crash_path) {
        Ok(mut f) => {
            if let Err(e) = f.write_all(body.as_bytes()) {
                eprintln!("Failed to write crash file {}: {e}", crash_path.display());
            }
        }
        Err(e) => eprintln!("Failed to open crash file {}: {e}", crash_path.display()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn write_crash_file_creates_timestamped_file() {
        let dir = TempDir::new().unwrap();
        write_crash_file(dir.path(), "db_init", "disk full");

        let files: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("crash-")
            })
            .collect();

        assert_eq!(files.len(), 1, "expected exactly one crash file");
        let contents = fs::read_to_string(files[0].path()).unwrap();
        assert!(contents.contains("Context: db_init"));
        assert!(contents.contains("disk full"));
    }

    #[test]
    fn write_crash_file_creates_missing_dir() {
        let dir = TempDir::new().unwrap();
        let nested = dir.path().join("does/not/exist");
        write_crash_file(&nested, "boot", "test");
        assert!(nested.exists());
        assert!(fs::read_dir(&nested).unwrap().count() >= 1);
    }
}
