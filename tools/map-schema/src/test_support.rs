use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

pub fn assert_dir_tree_eq(expected_root: &Path, actual_root: &Path) {
    let expected = collect_files(expected_root);
    let actual = collect_files(actual_root);

    let expected_paths: Vec<_> = expected.keys().cloned().collect();
    let actual_paths: Vec<_> = actual.keys().cloned().collect();
    assert_eq!(
        expected_paths, actual_paths,
        "directory trees differ:\nexpected: {:?}\nactual:   {:?}",
        expected_paths, actual_paths
    );

    for path in expected_paths {
        let expected_bytes = expected.get(&path).expect("expected tree entry");
        let actual_bytes = actual.get(&path).expect("actual tree entry");
        assert_eq!(
            expected_bytes,
            actual_bytes,
            "file contents differ for relative path {}",
            path.display()
        );
    }
}

pub fn assert_json_dir_trees_eq_ignoring_meta(expected_root: &Path, actual_root: &Path) {
    let expected = collect_json_files(expected_root);
    let actual = collect_json_files(actual_root);

    let expected_paths: Vec<_> = expected.keys().cloned().collect();
    let actual_paths: Vec<_> = actual.keys().cloned().collect();
    assert_eq!(
        expected_paths, actual_paths,
        "JSON directory trees differ:\nexpected: {:?}\nactual:   {:?}",
        expected_paths, actual_paths
    );

    for path in expected_paths {
        let mut expected_value = expected.get(&path).cloned().expect("expected JSON tree entry");
        let mut actual_value = actual.get(&path).cloned().expect("actual JSON tree entry");
        strip_meta_section(&mut expected_value);
        strip_meta_section(&mut actual_value);
        assert_ordered_json_eq(&expected_value, &actual_value, path.to_string_lossy().as_ref());
    }
}

fn collect_files(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut files = BTreeMap::new();
    collect_files_recursive(root, root, &mut files);
    files
}

fn collect_json_files(root: &Path) -> BTreeMap<PathBuf, Value> {
    let mut files = BTreeMap::new();
    collect_json_files_recursive(root, root, &mut files);
    files
}

fn collect_json_files_recursive(root: &Path, current: &Path, files: &mut BTreeMap<PathBuf, Value>) {
    let entries = fs::read_dir(current)
        .unwrap_or_else(|error| panic!("failed to read directory {}: {error}", current.display()));

    for entry in entries {
        let entry = entry.unwrap_or_else(|error| {
            panic!("failed to read directory entry while scanning {}: {error}", current.display())
        });
        let path = entry.path();
        if path.is_dir() {
            collect_json_files_recursive(root, &path, files);
            continue;
        }

        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }

        let raw = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read file {}: {error}", path.display()));
        let value: Value = serde_json::from_str(&raw).unwrap_or_else(|error| {
            panic!("failed to parse JSON file {}: {error}", path.display())
        });
        let relative = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
        files.insert(relative, value);
    }
}

fn strip_meta_section(value: &mut Value) {
    if let Value::Object(root) = value {
        root.remove("meta");
    }
}

fn assert_ordered_json_eq(expected: &Value, actual: &Value, path: &str) {
    if let Some(mismatch) = find_ordered_json_mismatch(expected, actual, path) {
        panic!("{mismatch}");
    }
}

fn find_ordered_json_mismatch(expected: &Value, actual: &Value, path: &str) -> Option<String> {
    match (expected, actual) {
        (Value::Object(expected_object), Value::Object(actual_object)) => {
            let expected_keys: Vec<_> = expected_object.keys().cloned().collect();
            let actual_keys: Vec<_> = actual_object.keys().cloned().collect();
            if expected_keys != actual_keys {
                return Some(format!(
                    "JSON object key order mismatch at {path}\nexpected: {:?}\nactual:   {:?}",
                    expected_keys, actual_keys
                ));
            }

            for key in expected_keys {
                let next_path = format!("{path}.{key}");
                if let Some(mismatch) =
                    find_ordered_json_mismatch(&expected_object[&key], &actual_object[&key], &next_path)
                {
                    return Some(mismatch);
                }
            }
            None
        }
        (Value::Array(expected_array), Value::Array(actual_array)) => {
            if expected_array.len() != actual_array.len() {
                return Some(format!(
                    "JSON array length mismatch at {path}\nexpected: {}\nactual:   {}",
                    expected_array.len(),
                    actual_array.len()
                ));
            }
            for (index, (expected_item, actual_item)) in
                expected_array.iter().zip(actual_array.iter()).enumerate()
            {
                if let Some(mismatch) =
                    find_ordered_json_mismatch(expected_item, actual_item, &format!("{path}[{index}]"))
                {
                    return Some(mismatch);
                }
            }
            None
        }
        _ => {
            if expected != actual {
                return Some(format!(
                    "JSON value mismatch at {path}\nexpected: {expected}\nactual:   {actual}"
                ));
            }
            None
        }
    }
}

fn collect_files_recursive(root: &Path, current: &Path, files: &mut BTreeMap<PathBuf, Vec<u8>>) {
    let entries = fs::read_dir(current)
        .unwrap_or_else(|error| panic!("failed to read directory {}: {error}", current.display()));

    for entry in entries {
        let entry = entry.unwrap_or_else(|error| {
            panic!("failed to read directory entry while scanning {}: {error}", current.display())
        });
        let path = entry.path();
        if path.is_dir() {
            collect_files_recursive(root, &path, files);
            continue;
        }

        let bytes = fs::read(&path)
            .unwrap_or_else(|error| panic!("failed to read file {}: {error}", path.display()));
        let relative = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
        files.insert(relative, bytes);
    }
}
