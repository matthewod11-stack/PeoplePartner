//! File discovery: walking the watched root, filtering by extension and size,
//! computing content hashes for change detection.

use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

use super::ingest::DocumentError;

/// Supported file extensions for indexing
const SUPPORTED_EXTENSIONS: &[&str] = &["md", "txt", "csv", "pdf", "docx", "xlsx", "xls"];

/// Maximum file size to index (50 MB)
const MAX_FILE_SIZE: u64 = 50 * 1024 * 1024;

/// Walk a directory and return all supported files
pub fn discover_files(folder_path: &Path) -> Result<Vec<PathBuf>, DocumentError> {
    let mut files = Vec::new();
    walk_dir(folder_path, &mut files)?;
    files.sort();
    Ok(files)
}

fn walk_dir(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), DocumentError> {
    if !dir.is_dir() {
        return Ok(());
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            log::warn!("[Documents] Cannot read directory {}: {}", dir.display(), e);
            return Ok(()); // Skip inaccessible directories gracefully
        }
    };
    for entry_result in entries {
        let entry = match entry_result {
            Ok(e) => e,
            Err(e) => {
                log::warn!("[Documents] Skipping unreadable entry in {}: {}", dir.display(), e);
                continue;
            }
        };
        let path = entry.path();
        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(e) => {
                log::warn!("[Documents] Cannot read metadata for {}: {}", path.display(), e);
                continue;
            }
        };

        // Skip symlinks
        if metadata.file_type().is_symlink() {
            continue;
        }

        if metadata.is_dir() {
            // Skip hidden directories
            if path.file_name().map_or(false, |n| n.to_string_lossy().starts_with('.')) {
                continue;
            }
            walk_dir(&path, files)?;
        } else if is_supported_file(&path) {
            // Skip oversized files
            if metadata.len() > MAX_FILE_SIZE {
                log::info!("[Documents] Skipping oversized file ({} bytes): {}", metadata.len(), path.display());
                continue;
            }
            files.push(path);
        }
    }
    Ok(())
}

fn is_supported_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| SUPPORTED_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
        .unwrap_or(false)
}

/// Compute SHA-256 hash of file contents
pub fn hash_file(path: &Path) -> Result<String, DocumentError> {
    let contents = std::fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&contents);
    Ok(hex::encode(hasher.finalize()))
}

/// Get the file extension as a lowercase string
pub fn file_type(path: &Path) -> String {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_lowercase())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_is_supported_file() {
        assert!(is_supported_file(Path::new("test.md")));
        assert!(is_supported_file(Path::new("test.pdf")));
        assert!(is_supported_file(Path::new("test.docx")));
        assert!(is_supported_file(Path::new("test.xlsx")));
        assert!(is_supported_file(Path::new("test.csv")));
        assert!(is_supported_file(Path::new("test.txt")));
        assert!(!is_supported_file(Path::new("test.jpg")));
        assert!(!is_supported_file(Path::new("test.exe")));
        assert!(!is_supported_file(Path::new("noextension")));
    }

    #[test]
    fn test_discover_files() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("policy.md"), "# Policy").unwrap();
        fs::write(dir.path().join("handbook.pdf"), "fake pdf").unwrap();
        fs::write(dir.path().join("photo.jpg"), "fake jpg").unwrap();
        fs::create_dir(dir.path().join(".hidden")).unwrap();
        fs::write(dir.path().join(".hidden/secret.md"), "hidden").unwrap();

        let files = discover_files(dir.path()).unwrap();
        assert_eq!(files.len(), 2); // .md and .pdf, not .jpg or hidden
    }

    #[test]
    fn test_discover_files_nested() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("subfolder")).unwrap();
        fs::write(dir.path().join("root.md"), "root").unwrap();
        fs::write(dir.path().join("subfolder/nested.txt"), "nested").unwrap();

        let files = discover_files(dir.path()).unwrap();
        assert_eq!(files.len(), 2);
    }

    #[test]
    fn test_hash_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.txt");
        fs::write(&path, "hello world").unwrap();

        let hash1 = hash_file(&path).unwrap();
        let hash2 = hash_file(&path).unwrap();
        assert_eq!(hash1, hash2); // Deterministic

        fs::write(&path, "changed content").unwrap();
        let hash3 = hash_file(&path).unwrap();
        assert_ne!(hash1, hash3); // Different content = different hash
    }

    #[test]
    fn test_file_type() {
        assert_eq!(file_type(Path::new("test.md")), "md");
        assert_eq!(file_type(Path::new("test.PDF")), "pdf");
        assert_eq!(file_type(Path::new("noext")), "");
    }

    #[test]
    fn test_walk_dir_skips_symlinks() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("real.md"), "# Real file").unwrap();

        // Create a symlink to a file
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(
                dir.path().join("real.md"),
                dir.path().join("link.md"),
            ).unwrap();
        }

        let files = discover_files(dir.path()).unwrap();
        // Should only find real.md, not the symlink
        assert_eq!(files.len(), 1);
        assert!(files[0].file_name().unwrap().to_str().unwrap() == "real.md");
    }
}
