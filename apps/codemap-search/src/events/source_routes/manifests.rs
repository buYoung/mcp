//! Module entrypoints are selected only from already captured source files.
//! Ambiguous exports stay unresolved; dependency directories are never opened.
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Component, Path, PathBuf};

fn relative(manifest: &str, target: &str) -> Option<String> {
    if target.contains('\\') || Path::new(target).is_absolute() {
        return None;
    }
    let path = Path::new(manifest).parent()?.join(target);
    let mut result = PathBuf::new();
    for part in path.components() {
        match part {
            Component::ParentDir => {
                if !result.pop() {
                    return None;
                }
            }
            Component::CurDir => {}
            Component::Normal(name) => result.push(name),
            _ => return None,
        }
    }
    Some(result.to_string_lossy().replace('\\', "/"))
}
fn entry(manifest: &str, target: &str, sources: &HashMap<String, String>) -> Option<String> {
    let path = relative(manifest, target)?;
    let mut choices = vec![path.clone()];
    for ext in ["ts", "tsx", "js", "jsx", "mts", "cts", "mjs", "cjs"] {
        choices.push(format!("{path}.{ext}"));
        choices.push(format!("{path}/index.{ext}"));
    }
    if path.ends_with(".js") {
        choices.push(format!("{}.ts", path.trim_end_matches(".js")));
        choices.push(format!("{}.tsx", path.trim_end_matches(".js")));
    }
    let found: BTreeSet<_> = choices
        .into_iter()
        .filter(|p| sources.contains_key(p))
        .collect();
    (found.len() == 1).then(|| found.into_iter().next().unwrap())
}
fn strings(value: &serde_json::Value, depth: usize) -> Vec<&str> {
    if depth >= 8 {
        return Vec::new();
    }
    match value {
        serde_json::Value::String(s) => vec![s],
        serde_json::Value::Object(map) => map
            .values()
            .take(32)
            .flat_map(|v| strings(v, depth + 1))
            .collect(),
        serde_json::Value::Array(array) => array
            .iter()
            .take(32)
            .flat_map(|v| strings(v, depth + 1))
            .collect(),
        _ => Vec::new(),
    }
}
pub(super) fn bindings(sources: &HashMap<String, String>) -> BTreeMap<String, String> {
    let mut candidates: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (path, source) in sources {
        if Path::new(path)
            .file_name()
            .is_some_and(|n| n == "Cargo.toml")
        {
            let Ok(value) = toml::from_str::<toml::Value>(source) else {
                continue;
            };
            let Some(package) = value
                .get("package")
                .and_then(|v| v.get("name"))
                .and_then(toml::Value::as_str)
            else {
                continue;
            };
            let lib = value.get("lib");
            let name = lib
                .and_then(|v| v.get("name"))
                .and_then(toml::Value::as_str)
                .unwrap_or(package)
                .replace('-', "_");
            let requested = lib
                .and_then(|v| v.get("path"))
                .and_then(toml::Value::as_str)
                .unwrap_or("src/lib.rs");
            if let Some(path) = relative(path, requested).filter(|p| sources.contains_key(p)) {
                candidates.entry(name).or_default().insert(path);
            }
        } else if Path::new(path)
            .file_name()
            .is_some_and(|n| n == "package.json")
        {
            let Ok(value) = serde_json::from_str::<serde_json::Value>(source) else {
                continue;
            };
            let Some(name) = value["name"].as_str() else {
                continue;
            };
            let mut exports = Vec::new();
            if let Some(map) = value["exports"]
                .as_object()
                .filter(|m| m.keys().any(|k| k.starts_with('.')))
            {
                for (key, value) in map.iter().take(256) {
                    if key == "." || key.starts_with("./") {
                        exports.push((
                            if key == "." {
                                name.to_owned()
                            } else {
                                format!("{name}/{}", &key[2..])
                            },
                            value,
                        ));
                    }
                }
            } else if !value["exports"].is_null() {
                exports.push((name.into(), &value["exports"]));
            } else {
                for key in ["source", "module", "main"] {
                    if value.get(key).is_some() {
                        exports.push((name.into(), &value[key]));
                    }
                }
            }
            for (key, value) in exports {
                for target in strings(value, 0) {
                    if key.matches('*').count() == 1 && target.matches('*').count() == 1 {
                        let Some(pattern) = relative(path, target) else {
                            continue;
                        };
                        let (prefix, suffix) = pattern.split_once('*').unwrap();
                        let (prefix_key, suffix_key) = key.split_once('*').unwrap();
                        for candidate in sources.keys() {
                            if let Some(middle) = candidate
                                .strip_prefix(prefix)
                                .and_then(|v| v.strip_suffix(suffix))
                            {
                                candidates
                                    .entry(format!("{prefix_key}{middle}{suffix_key}"))
                                    .or_default()
                                    .insert(candidate.clone());
                            }
                        }
                    } else if !key.contains('*') && !target.contains('*') {
                        if let Some(target) = entry(path, target, sources) {
                            candidates.entry(key.clone()).or_default().insert(target);
                        }
                    }
                }
            }
        }
    }
    candidates
        .into_iter()
        .filter_map(|(name, mut paths)| {
            (paths.len() == 1).then(|| (name, paths.pop_first().unwrap()))
        })
        .collect()
}
