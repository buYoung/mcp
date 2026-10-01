//! Record ownership and update only the single instruction line managed by this server.

use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

const MANAGED_MARKER: &str = "<!-- codemap-search:managed -->";
const GLOBAL_INSTRUCTION: &str = include_str!("../../tools/instructions/global.md");

#[derive(Clone, Default, Deserialize, PartialEq, Serialize)]
struct Registry {
    files: Vec<ManagedFile>,
}

#[derive(Clone, Deserialize, PartialEq, Serialize)]
struct ManagedFile {
    path: PathBuf,
    lines: Vec<String>,
    line_offset_bytes: usize,
    prefix_fingerprint: String,
    inserted_separator: Option<InsertedSeparator>,
}

#[derive(Clone, Deserialize, PartialEq, Serialize)]
struct InsertedSeparator {
    original_length_bytes: usize,
    original_fingerprint: String,
    newline: String,
}

pub(super) fn synchronize(directory: &Path, instruction_path: Option<&Path>) -> Result<(), String> {
    let registry_path = directory.join("global-instructions.json");
    if instruction_path.is_none() && !registry_path.exists() {
        return Ok(());
    }
    fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    let _lock = lock_registry(&directory.join("global-instructions.lock"))?;
    let original_registry = read_document(&registry_path)?;
    let mut registry: Registry = match original_registry.as_deref() {
        Some(content) => serde_json::from_str(content).map_err(|error| {
            format!(
                "invalid global instruction registry: {}: {error}",
                registry_path.display()
            )
        })?,
        None => Registry::default(),
    };
    if registry.files.iter().any(|file| {
        !file.path.is_absolute()
            || file.lines.is_empty()
            || file
                .lines
                .iter()
                .any(|line| !line.ends_with(MANAGED_MARKER) || line.contains(['\r', '\n']))
            || file
                .inserted_separator
                .as_ref()
                .is_some_and(|separator| !matches!(separator.newline.as_str(), "\n" | "\r\n"))
    }) {
        return Err(format!(
            "invalid ownership records in global instruction registry: {}",
            registry_path.display()
        ));
    }
    if let Some(path) = instruction_path {
        add_instruction(
            path,
            &registry_path,
            original_registry.as_deref(),
            &mut registry,
        )
    } else {
        remove_instructions(&registry_path, original_registry.as_deref(), &mut registry)
    }
}

pub(super) fn read_document(path: &Path) -> Result<Option<String>, String> {
    match fs::read_to_string(path) {
        Ok(content) => Ok(Some(content)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!(
            "파일을 읽지 못했습니다: {}: {error}",
            path.display()
        )),
    }
}

fn lock_registry(path: &Path) -> Result<File, String> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options
        .open(path)
        .map_err(|error| format!("전역 지침 잠금 파일을 열지 못했습니다: {error}"))?;
    for attempt in 0..20 {
        match file.try_lock() {
            Ok(()) => return Ok(file),
            Err(TryLockError::WouldBlock) if attempt < 19 => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(error) => return Err(format!("cannot lock global instruction registry: {error}")),
        }
    }
    Err("global instruction registry is busy".into())
}

fn add_instruction(
    path: &Path,
    registry_path: &Path,
    original_registry: Option<&str>,
    registry: &mut Registry,
) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or("전역 지침 파일의 상위 디렉터리가 없습니다.")?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("전역 지침 디렉터리를 만들지 못했습니다: {error}"))?;
    // 기존 심볼릭 링크를 교체하지 않고, 링크가 가리키는 실제 파일을 관리합니다.
    let path = if fs::symlink_metadata(path).is_ok() {
        fs::canonicalize(path)
            .map_err(|error| format!("전역 지침 파일의 실제 경로를 확인하지 못했습니다: {error}"))?
    } else {
        parent
            .canonicalize()
            .map_err(|error| error.to_string())?
            .join(path.file_name().ok_or("전역 지침 파일 이름이 없습니다.")?)
    };
    let original = read_document(&path)?;
    let content = original.as_deref().unwrap_or("");
    let existing = registry.files.iter().position(|file| file.path == path);
    let mut entry = existing
        .map(|index| registry.files[index].clone())
        .unwrap_or_else(|| ManagedFile {
            path: path.clone(),
            lines: Vec::new(),
            line_offset_bytes: 0,
            prefix_fingerprint: String::new(),
            inserted_separator: None,
        });

    let location = locate_instruction(content, &entry)?;
    let updated = if let Some((start_bytes, end_bytes)) = location {
        let ending = line_ending(&content[start_bytes..end_bytes]);
        entry.line_offset_bytes = start_bytes;
        entry.prefix_fingerprint = fingerprint(&content[..start_bytes]);
        format!(
            "{}{}{}{}",
            &content[..start_bytes],
            GLOBAL_INSTRUCTION.trim_end(),
            ending,
            &content[end_bytes..]
        )
    } else {
        if content.contains(MANAGED_MARKER) {
            return Err(format!(
                "관리 표식이 있지만 추가한 줄을 확인하지 못했습니다: {}",
                path.display()
            ));
        }
        let newline = if content.contains("\r\n") {
            "\r\n"
        } else {
            "\n"
        };
        let mut updated = content.to_string();
        entry.inserted_separator = if !content.is_empty() && !content.ends_with('\n') {
            updated.push_str(newline);
            Some(InsertedSeparator {
                original_length_bytes: content.len(),
                original_fingerprint: fingerprint(content),
                newline: newline.to_string(),
            })
        } else {
            None
        };
        entry.line_offset_bytes = updated.len();
        entry.prefix_fingerprint = fingerprint(&updated);
        updated.push_str(GLOBAL_INSTRUCTION.trim_end());
        updated.push_str(newline);
        updated
    };

    if !entry
        .lines
        .iter()
        .any(|line| line == GLOBAL_INSTRUCTION.trim_end())
    {
        entry.lines.push(GLOBAL_INSTRUCTION.trim_end().to_string());
    }
    match existing {
        Some(index) => registry.files[index] = entry,
        None => registry.files.push(entry),
    }
    // 파일 갱신보다 기록을 먼저 저장해 중단 이후에도 관리 줄을 제거할 수 있게 합니다.
    save_registry(registry_path, original_registry, registry)?;
    if updated != content {
        atomic_write(&path, original.as_deref(), &updated)?;
    }
    Ok(())
}

fn locate_instruction(
    content: &str,
    entry: &ManagedFile,
) -> Result<Option<(usize, usize)>, String> {
    let mut offset_bytes = 0;
    let mut candidates = Vec::new();
    for line in content.split_inclusive('\n') {
        let body = line.strip_suffix(line_ending(line)).unwrap_or(line);
        if entry.lines.iter().any(|owned| owned == body) {
            if offset_bytes == entry.line_offset_bytes
                && fingerprint(&content[..offset_bytes]) == entry.prefix_fingerprint
            {
                return Ok(Some((offset_bytes, offset_bytes + line.len())));
            }
            candidates.push((offset_bytes, offset_bytes + line.len()));
        }
        offset_bytes += line.len();
    }
    match candidates.len() {
        0 => Ok(None),
        1 => Ok(candidates.pop()),
        _ => Err(format!(
            "같은 관리 줄이 여러 곳에 있어 변경 대상을 확인하지 못했습니다: {}",
            entry.path.display()
        )),
    }
}

fn line_ending(line: &str) -> &str {
    if line.ends_with("\r\n") {
        "\r\n"
    } else if line.ends_with('\n') {
        "\n"
    } else {
        ""
    }
}

fn fingerprint(content: &str) -> String {
    blake3::hash(content.as_bytes()).to_hex().to_string()
}

fn remove_instructions(
    registry_path: &Path,
    original_registry: Option<&str>,
    registry: &mut Registry,
) -> Result<(), String> {
    let previous = registry.clone();
    let mut remaining = Vec::new();
    let mut failures = Vec::new();
    for entry in &registry.files {
        if let Err(error) = remove_instruction(entry) {
            remaining.push(entry.clone());
            failures.push(error);
        }
    }
    registry.files = remaining;
    if *registry != previous {
        save_registry(registry_path, original_registry, registry)?;
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("\n"))
    }
}

fn remove_instruction(entry: &ManagedFile) -> Result<(), String> {
    let Some(original) = read_document(&entry.path)? else {
        return Ok(());
    };
    let Some((start_bytes, end_bytes)) = locate_instruction(&original, entry)? else {
        if original.contains(MANAGED_MARKER) {
            return Err(format!(
                "관리 줄이 수정되어 자동으로 제거하지 않았습니다: {}",
                entry.path.display()
            ));
        }
        return Ok(());
    };
    let mut updated = format!("{}{}", &original[..start_bytes], &original[end_bytes..]);
    if end_bytes == original.len() {
        if let Some(separator) = &entry.inserted_separator {
            if let Some(prefix) = updated.strip_suffix(&separator.newline) {
                if prefix.len() == separator.original_length_bytes
                    && fingerprint(prefix) == separator.original_fingerprint
                {
                    updated = prefix.to_string();
                }
            }
        }
    }
    atomic_write(&entry.path, Some(&original), &updated)
}

fn save_registry(path: &Path, original: Option<&str>, registry: &Registry) -> Result<(), String> {
    let mut content = serde_json::to_string_pretty(registry).map_err(|error| error.to_string())?;
    content.push('\n');
    if original != Some(content.as_str()) {
        atomic_write(path, original, &content)?;
    }
    Ok(())
}

fn atomic_write(path: &Path, original: Option<&str>, content: &str) -> Result<(), String> {
    // Resolve existing symlinks for both instruction files and the registry.
    let target = match fs::symlink_metadata(path) {
        Ok(_) => path.canonicalize().map_err(|error| error.to_string())?,
        Err(error) if error.kind() == ErrorKind::NotFound => path.to_path_buf(),
        Err(error) => return Err(error.to_string()),
    };
    let parent = target.parent().ok_or("file has no parent directory")?;
    let mut temporary =
        tempfile::NamedTempFile::new_in(parent).map_err(|error| error.to_string())?;
    if let Ok(metadata) = fs::metadata(&target) {
        temporary
            .as_file()
            .set_permissions(metadata.permissions())
            .map_err(|error| error.to_string())?;
    }
    temporary
        .write_all(content.as_bytes())
        .map_err(|error| error.to_string())?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| error.to_string())?;
    if read_document(&target)?.as_deref() != original {
        return Err(format!(
            "file changed during global instruction sync: {}",
            target.display()
        ));
    }
    temporary
        .persist(&target)
        .map_err(|error| error.to_string())?;
    Ok(())
}
