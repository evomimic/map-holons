use std::{
    collections::BTreeSet,
    fs,
    path::{Component, Path, PathBuf},
};

use super::{SourceDiscovery, SourceFile, SourceIssue, SourceIssueKind};

/// Discover in path order, deduplicating overlapping roots before reading content.
/// Symlinks are excluded; hard links with different paths remain distinct sources.
pub fn discover_sources(paths: Vec<PathBuf>) -> SourceDiscovery {
    let mut result = SourceDiscovery::default();
    let mut pending = BTreeSet::new();
    for path in paths {
        match normalize(&path) {
            Some(path) => {
                pending.insert(path);
            }
            None => issue(
                &mut result,
                &path,
                SourceIssueKind::InvalidPath,
                "Native sources must have absolute paths without parent traversal",
            ),
        }
    }
    let mut visited = BTreeSet::new();
    while let Some(path) = pending.pop_first() {
        if !visited.insert(path.clone()) {
            continue;
        }
        let Some(display) = path.to_str() else {
            issue(
                &mut result,
                &path,
                SourceIssueKind::InvalidPath,
                "Source path is not valid UTF-8",
            );
            continue;
        };
        // Check ancestors as well: an explicitly selected file may be beneath a symlink.
        if path.ancestors().any(|ancestor| {
            fs::symlink_metadata(ancestor).is_ok_and(|metadata| metadata.file_type().is_symlink())
        }) {
            issue(
                &mut result,
                &path,
                SourceIssueKind::SymlinkSkipped,
                "Symbolic links are not followed",
            );
            continue;
        }
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) => {
                issue(&mut result, &path, SourceIssueKind::Unreadable, &error.to_string());
                continue;
            }
        };
        if metadata.is_dir() {
            match fs::read_dir(&path) {
                Ok(entries) => {
                    for entry in entries {
                        match entry {
                            Ok(entry) => {
                                pending.insert(entry.path());
                            }
                            Err(error) => issue(
                                &mut result,
                                &path,
                                SourceIssueKind::Unreadable,
                                &error.to_string(),
                            ),
                        }
                    }
                }
                Err(error) => {
                    issue(&mut result, &path, SourceIssueKind::Unreadable, &error.to_string())
                }
            }
        } else if metadata.is_file() {
            if !path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
            {
                continue;
            }
            match fs::read_to_string(&path) {
                Ok(content) => result.sources.push(SourceFile {
                    id: display.into(),
                    path: display.into(),
                    content,
                }),
                Err(error) => {
                    issue(&mut result, &path, SourceIssueKind::Unreadable, &error.to_string())
                }
            }
        } else {
            issue(
                &mut result,
                &path,
                SourceIssueKind::UnsupportedFile,
                "Only regular files and directories are supported",
            );
        }
    }
    result.sources.sort_by(|a, b| a.path.cmp(&b.path));
    result.issues.sort_by(|a, b| a.path.cmp(&b.path).then(a.message.cmp(&b.message)));
    result
}

fn normalize(path: &Path) -> Option<PathBuf> {
    if !path.is_absolute() {
        return None;
    }
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => return None,
            Component::CurDir => {}
            component => normalized.push(component.as_os_str()),
        }
    }
    Some(normalized)
}

fn issue(result: &mut SourceDiscovery, path: &Path, kind: SourceIssueKind, message: &str) {
    result.issues.push(SourceIssue {
        path: path.to_string_lossy().into_owned(),
        kind,
        message: message.into(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_overlapping_roots_preserve_paths_and_snapshots_in_stable_order() {
        let temp = tempdir::TempDir::new("source-discovery").unwrap();
        let root = temp.path().canonicalize().unwrap();
        fs::create_dir(root.join("nested")).unwrap();
        fs::write(root.join("same.json"), "first").unwrap();
        fs::write(root.join("nested/same.json"), "second").unwrap();
        fs::write(root.join("upper.JSON"), "").unwrap();
        fs::write(root.join("ignore.txt"), "ignored").unwrap();
        let result =
            discover_sources(vec![root.join("nested"), root.clone(), root.join("same.json")]);
        assert_eq!(result, discover_sources(vec![root.clone()]));
        assert_eq!(result.sources.len(), 3);
        assert!(result.issues.is_empty());
        assert_ne!(result.sources[0].id, result.sources[1].id);
        assert!(result.sources.iter().all(|source| source.path == source.id));
        fs::write(root.join("same.json"), "changed").unwrap();
        assert_eq!(result.sources[1].content, "first");
        assert_eq!(result.sources[2].content, "");
    }

    #[test]
    fn read_failures_do_not_discard_readable_sources() {
        let temp = tempdir::TempDir::new("source-discovery").unwrap();
        let root = temp.path().canonicalize().unwrap();
        fs::write(root.join("good.json"), "{}").unwrap();
        fs::write(root.join("invalid.json"), [0xff]).unwrap();
        let result = discover_sources(vec![root.clone(), root.join("missing.json")]);
        assert_eq!(result.sources.len(), 1);
        assert_eq!(result.issues.len(), 2);
        assert!(result.issues.iter().all(|issue| issue.kind == SourceIssueKind::Unreadable));
    }

    #[test]
    fn empty_and_invalid_roots_are_explicit() {
        let temp = tempdir::TempDir::new("source-discovery").unwrap();
        assert_eq!(
            discover_sources(vec![temp.path().canonicalize().unwrap()]),
            SourceDiscovery::default()
        );
        let result = discover_sources(vec![
            PathBuf::from("relative.json"),
            temp.path().join("../escape.json"),
        ]);
        assert_eq!(result.issues.len(), 2);
        assert!(result.sources.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn unreadable_file_and_directory_are_reported() {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempdir::TempDir::new("source-permissions").unwrap();
        let root = temp.path().canonicalize().unwrap();
        let file = root.join("denied.json");
        let directory = root.join("denied");
        fs::write(&file, "{}").unwrap();
        fs::create_dir(&directory).unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0)).unwrap();
        fs::set_permissions(&directory, fs::Permissions::from_mode(0)).unwrap();
        let file_denied = fs::read_to_string(&file).is_err();
        let directory_denied = fs::read_dir(&directory).is_err();
        let result = discover_sources(vec![root]);
        fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).unwrap();
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
        // Privileged test runners may bypass POSIX mode bits.
        assert_eq!(result.issues.len(), usize::from(file_denied) + usize::from(directory_denied));
        assert!(result.issues.iter().all(|issue| issue.kind == SourceIssueKind::Unreadable));
    }

    #[cfg(unix)]
    #[test]
    fn symlink_files_directories_and_cycles_are_skipped() {
        use std::os::unix::fs::symlink;
        let temp = tempdir::TempDir::new("source-discovery").unwrap();
        let root = temp.path().canonicalize().unwrap();
        fs::write(root.join("good.json"), "{}").unwrap();
        symlink(root.join("good.json"), root.join("alias.json")).unwrap();
        symlink(&root, root.join("cycle")).unwrap();
        let result = discover_sources(vec![root.clone(), root.join("cycle/good.json")]);
        assert_eq!(result.sources.len(), 1);
        assert_eq!(result.issues.len(), 3);
        assert!(result.issues.iter().all(|issue| issue.kind == SourceIssueKind::SymlinkSkipped));
    }
}
