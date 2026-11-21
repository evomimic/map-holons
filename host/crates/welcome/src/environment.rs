//! Local environment selection; changing selection never moves or deletes an existing store.
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

const ORIGINAL_ENVIRONMENT: &str = "local-constitution";
const SELECTION_FILE: &str = "active-local-environment";

/// Display-only inventory. Compatibility is verified by ordinary startup after selection.
#[derive(Debug, Serialize)]
pub struct SavedEnvironment {
    pub id: String,
    pub active: bool,
    pub created_seconds: Option<u64>,
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Default)]
pub struct EnvironmentMismatch(pub RwLock<Option<String>>);

/// Resolve the selected environment. Absence preserves the original installation's path.
pub fn selected_root(base: &Path) -> anyhow::Result<PathBuf> {
    match std::fs::read_to_string(base.join(SELECTION_FILE)) {
        Ok(name) => environment_path(base, &name),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(base.join(ORIGINAL_ENVIRONMENT))
        }
        Err(error) => Err(error.into()),
    }
}

/// Resolve only a known, direct directory; IPC never supplies filesystem paths.
fn environment_path(base: &Path, id: &str) -> anyhow::Result<PathBuf> {
    let root = if id == ORIGINAL_ENVIRONMENT {
        base.join(id)
    } else {
        anyhow::ensure!(valid_name(id), "Invalid saved environment");
        let parent = base.join("environments");
        let metadata = std::fs::symlink_metadata(&parent)?;
        anyhow::ensure!(
            metadata.is_dir() && !metadata.file_type().is_symlink(),
            "Invalid environment directory"
        );
        parent.join(id)
    };
    let metadata = std::fs::symlink_metadata(&root)?;
    anyhow::ensure!(
        metadata.is_dir() && !metadata.file_type().is_symlink(),
        "Saved environment is missing or is not a directory"
    );
    Ok(root)
}

fn directory_bytes(root: &Path) -> anyhow::Result<u64> {
    let mut bytes = 0u64;
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let metadata = std::fs::symlink_metadata(entry.path())?;
        let size = if metadata.file_type().is_symlink() {
            0
        } else if metadata.is_dir() {
            directory_bytes(&entry.path())?
        } else {
            metadata.len()
        };
        bytes =
            bytes.checked_add(size).ok_or_else(|| anyhow::anyhow!("Environment size overflow"))?;
    }
    Ok(bytes)
}

pub fn list_environments(base: &Path) -> anyhow::Result<Vec<SavedEnvironment>> {
    let active = selected_root(base)?;
    let mut ids = Vec::new();
    if base.join(ORIGINAL_ENVIRONMENT).exists() {
        ids.push(ORIGINAL_ENVIRONMENT.to_owned());
    }
    let parent = base.join("environments");
    if parent.exists() {
        let metadata = std::fs::symlink_metadata(&parent)?;
        anyhow::ensure!(
            metadata.is_dir() && !metadata.file_type().is_symlink(),
            "Invalid environment directory"
        );
        for entry in std::fs::read_dir(parent)? {
            let entry = entry?;
            if let Some(id) = entry.file_name().to_str().filter(|id| valid_name(id)) {
                if environment_path(base, id).is_ok() {
                    ids.push(id.to_owned());
                }
            }
        }
    }
    let mut environments = Vec::new();
    for id in ids {
        let root = environment_path(base, &id)?;
        let created_seconds = std::fs::metadata(&root)?
            .created()
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|time| time.as_secs());
        environments.push(SavedEnvironment {
            id,
            active: root == active,
            created_seconds,
            size_bytes: directory_bytes(&root).ok(),
        });
    }
    environments.sort_by(|a, b| {
        b.active
            .cmp(&a.active)
            .then(b.created_seconds.cmp(&a.created_seconds))
            .then(a.id.cmp(&b.id))
    });
    Ok(environments)
}

/// Atomically select an existing environment. It is opened and checked only after restart.
pub fn activate_environment(base: &Path, id: &str) -> anyhow::Result<()> {
    let root = environment_path(base, id)?;
    anyhow::ensure!(root != selected_root(base)?, "This environment is already active");
    write_selection(base, &root, id)
}

fn write_selection(base: &Path, root: &Path, id: &str) -> anyhow::Result<()> {
    use std::io::Write;
    let pending = root.join(format!("selection-{}", std::process::id()));
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(&pending)?;
    let result = (|| {
        file.write_all(id.as_bytes())?;
        file.sync_all()?;
        std::fs::rename(&pending, base.join(SELECTION_FILE))
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&pending);
    }
    Ok(result?)
}

/// Cleanup cannot select a path or touch the active store; the platform owns disposal.
fn clean_inactive_environment(
    base: &Path,
    id: &str,
    dispose: impl FnOnce(&Path) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let root = environment_path(base, id)?;
    anyhow::ensure!(root != selected_root(base)?, "The active environment cannot be removed");
    dispose(&root)
}

pub fn trash_environment(base: &Path, id: &str) -> anyhow::Result<()> {
    clean_inactive_environment(base, id, move_to_trash)
}

#[cfg(target_os = "macos")]
fn move_to_trash(root: &Path) -> anyhow::Result<()> {
    use objc2_foundation::{NSFileManager, NSURL};
    let url = NSURL::from_file_path(root)
        .ok_or_else(|| anyhow::anyhow!("Invalid environment location"))?;
    NSFileManager::defaultManager()
        .trashItemAtURL_resultingItemURL_error(&url, None)
        .map_err(|error| anyhow::anyhow!("Could not move environment to Trash: {error}"))
}

#[cfg(not(target_os = "macos"))]
fn move_to_trash(_root: &Path) -> anyhow::Result<()> {
    anyhow::bail!("Environment Trash is unavailable on this platform")
}

fn valid_name(name: &str) -> bool {
    name.starts_with("environment-") && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

/// Reserve a separate store, then atomically select it for the next process launch.
pub fn create_environment(base: &Path) -> anyhow::Result<PathBuf> {
    let environments = base.join("environments");
    std::fs::create_dir_all(&environments)?;
    let name = format!(
        "environment-{}-{}",
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos(),
        std::process::id()
    );
    let root = environments.join(&name);
    std::fs::create_dir(&root)?;
    write_selection(base, &root, &name)?;
    Ok(root)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inventory_reactivates_original_without_replacing_any_store() {
        let temp = tempdir::TempDir::new("map-saved-environments").unwrap();
        let base = temp.path();
        let original = selected_root(base).unwrap();
        std::fs::create_dir_all(&original).unwrap();
        std::fs::write(original.join("identity"), b"original").unwrap();
        let first = create_environment(base).unwrap();
        std::fs::write(first.join("identity"), b"first").unwrap();
        let second = create_environment(base).unwrap();
        let catalog = list_environments(base).unwrap();
        assert_eq!(catalog.len(), 3);
        assert_eq!(catalog.iter().filter(|e| e.active).count(), 1);
        assert_eq!(catalog[0].id, second.file_name().unwrap().to_str().unwrap());
        assert_eq!(
            catalog.iter().find(|e| e.id == ORIGINAL_ENVIRONMENT).unwrap().size_bytes,
            Some(8)
        );
        activate_environment(base, ORIGINAL_ENVIRONMENT).unwrap();
        assert_eq!(selected_root(base).unwrap(), original);
        assert_eq!(std::fs::read(original.join("identity")).unwrap(), b"original");
        assert_eq!(std::fs::read(first.join("identity")).unwrap(), b"first");
        activate_environment(base, first.file_name().unwrap().to_str().unwrap()).unwrap();
        assert_eq!(selected_root(base).unwrap(), first);
    }

    #[test]
    fn cleanup_is_inactive_only_and_rejects_paths_and_unknown_environments() {
        let temp = tempdir::TempDir::new("map-environment-cleanup").unwrap();
        let base = temp.path();
        let original = selected_root(base).unwrap();
        std::fs::create_dir_all(&original).unwrap();
        let active = create_environment(base).unwrap();
        let active_id = active.file_name().unwrap().to_str().unwrap();
        for id in [active_id, "../local-constitution", "/tmp", "environment-missing"] {
            assert!(
                clean_inactive_environment(base, id, |_| panic!("must not dispose {id}")).is_err()
            );
        }
        let recoverable = base.join("test-trash");
        clean_inactive_environment(base, ORIGINAL_ENVIRONMENT, |path| {
            std::fs::rename(path, &recoverable)?;
            Ok(())
        })
        .unwrap();
        assert!(recoverable.exists());
        assert_eq!(selected_root(base).unwrap(), active);
        assert_eq!(list_environments(base).unwrap().len(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn environment_operations_never_follow_directory_symlinks() {
        let temp = tempdir::TempDir::new("map-environment-links").unwrap();
        let base = temp.path();
        create_environment(base).unwrap();
        let outside = tempdir::TempDir::new("map-outside-environments").unwrap();
        std::os::unix::fs::symlink(outside.path(), base.join("environments/environment-linked"))
            .unwrap();
        assert!(activate_environment(base, "environment-linked").is_err());
        assert!(clean_inactive_environment(base, "environment-linked", |_| panic!(
            "must not dispose external data"
        ))
        .is_err());
        assert_eq!(list_environments(base).unwrap().len(), 1);
    }

    #[test]
    fn new_selection_preserves_original_and_previous_environments() {
        let temp = tempdir::TempDir::new("map-environment-selection").unwrap();
        let base = temp.path();
        let original = selected_root(base).unwrap();
        std::fs::create_dir_all(&original).unwrap();
        std::fs::write(original.join("content"), b"retained").unwrap();
        let first = create_environment(base).unwrap();
        assert_eq!(selected_root(base).unwrap(), first);
        std::fs::write(first.join("identity"), b"first").unwrap();
        let second = create_environment(base).unwrap();
        assert_ne!(first, second);
        assert_eq!(selected_root(base).unwrap(), second);
        assert_eq!(std::fs::read(original.join("content")).unwrap(), b"retained");
        assert_eq!(std::fs::read(first.join("identity")).unwrap(), b"first");
        assert!(!second.join("identity").exists());
    }
    #[test]
    fn malformed_or_missing_selection_does_not_fall_back_to_another_store() {
        let temp = tempdir::TempDir::new("map-invalid-environment").unwrap();
        for name in ["../elsewhere", "environment-missing", "", "/absolute"] {
            std::fs::write(temp.path().join("active-local-environment"), name).unwrap();
            assert!(selected_root(temp.path()).is_err());
        }
    }
}
