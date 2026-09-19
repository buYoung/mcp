//! Indexed-file language statistics for `overview`.
//!
//! Counts are derived from the same published codemap population used to render the
//! requested view. Physical source bytes are read only for files already present in
//! that snapshot; tokei never walks the filesystem itself.

use crate::parser::ExtractedFile;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

const STATS_LANGUAGE_ROW_LIMIT: usize = 30;
const STATS_REQUEST_FILE_LIMIT: usize = 256;
const STATS_REQUEST_BYTE_LIMIT: u64 = 64 * 1024 * 1024;
const STATS_COUNTING_MODE: &str = "tokei-15.0.0:summarise:code-comments-blanks";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum StatsScope {
    Root,
    WorkspaceRoot(String),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct FileStats {
    code: usize,
    comments: usize,
    blanks: usize,
}

struct CountedFile {
    language: &'static str,
    stats: FileStats,
}

struct CollectionBudget {
    remaining_files: usize,
    remaining_bytes: u64,
    cache_entry_limit: usize,
}

impl FileStats {
    fn total(&self) -> usize {
        self.code + self.comments + self.blanks
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct LanguageStats {
    files: usize,
    code: usize,
    comments: usize,
    blanks: usize,
}

impl LanguageStats {
    fn record(&mut self, stats: FileStats) {
        self.files += 1;
        self.code += stats.code;
        self.comments += stats.comments;
        self.blanks += stats.blanks;
    }

    fn total(&self) -> usize {
        self.code + self.comments + self.blanks
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct CacheKey {
    workspace: String,
    snapshot_id: usize,
    path: String,
    size_bytes: u64,
    source_mtime_nanos: u64,
    language: &'static str,
    tokei_language: &'static str,
    counting_mode: &'static str,
}

static CACHE: OnceLock<Mutex<HashMap<CacheKey, FileStats>>> = OnceLock::new();

fn cache() -> &'static Mutex<HashMap<CacheKey, FileStats>> {
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn lock_cache() -> std::sync::MutexGuard<'static, HashMap<CacheKey, FileStats>> {
    cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn prune_cache(cache: &mut HashMap<CacheKey, FileStats>, workspace: &str, snapshot_id: usize) {
    cache.retain(|key, _| key.workspace == workspace && key.snapshot_id == snapshot_id);
}

fn normalized_path(path: &str) -> String {
    crate::workspace::normalize_workspace_key(path)
}

fn selected_files<'a>(files: &'a [ExtractedFile], scope: &StatsScope) -> Vec<&'a ExtractedFile> {
    match scope {
        StatsScope::Root => files.iter().collect(),
        StatsScope::WorkspaceRoot(root) => {
            let root = normalized_path(root);
            if root.is_empty() {
                return files.iter().collect();
            }
            let prefix = format!("{root}/");
            files
                .iter()
                .filter(|file| {
                    let path = normalized_path(&file.file_path);
                    path == root || path.starts_with(&prefix)
                })
                .collect()
        }
    }
}

fn scope_description(scope: &StatsScope) -> String {
    match scope {
        StatsScope::Root => "root (all indexed files)".to_string(),
        StatsScope::WorkspaceRoot(root) => format!("workspace root {}", normalized_path(root)),
    }
}

fn physical_path(cwd: &Path, file_path: &str) -> PathBuf {
    let path = Path::new(file_path);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

fn metadata_fingerprint(metadata: &std::fs::Metadata) -> (u64, Option<SystemTime>) {
    (metadata.len(), metadata.modified().ok())
}

#[allow(clippy::too_many_arguments)]
fn count_file(
    cwd: &Path,
    workspace: &str,
    snapshot_id: usize,
    indexed_mtime: Option<u64>,
    file: &ExtractedFile,
    config: &tokei::Config,
    budget: &mut CollectionBudget,
) -> Result<Option<CountedFile>, &'static str> {
    let path = physical_path(cwd, &file.file_path);
    let before = std::fs::metadata(&path).map_err(|_| "missing on disk or metadata unavailable")?;
    if before.len() > crate::config::get().max_file_size {
        return Err("exceeds the configured index size cap");
    }
    if !before.is_file() {
        return Err("no longer a regular file");
    }
    let source_mtime_nanos = before
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos() as u64)
        .ok_or("source modification time unavailable")?;
    let indexed_mtime = indexed_mtime.ok_or("source version unavailable in published snapshot")?;
    if source_mtime_nanos != indexed_mtime {
        return Err("source changed since the published snapshot");
    }

    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .ok_or("unsupported by tokei")?;
    let tokei_language =
        tokei::LanguageType::from_file_extension(extension).ok_or("unsupported by tokei")?;
    let language = crate::lang::language_name_for_path(Path::new(&file.file_path))
        .unwrap_or_else(|| tokei_language.name());
    let cache_key = CacheKey {
        workspace: workspace.to_string(),
        snapshot_id,
        path: normalized_path(&file.file_path),
        size_bytes: before.len(),
        source_mtime_nanos,
        language,
        tokei_language: tokei_language.name(),
        counting_mode: STATS_COUNTING_MODE,
    };

    {
        let cache = lock_cache();
        if let Some(stats) = cache.get(&cache_key) {
            return Ok(Some(CountedFile {
                language,
                stats: *stats,
            }));
        }
    }

    if before.len() > STATS_REQUEST_BYTE_LIMIT {
        return Err("exceeds the statistics per-request byte limit");
    }
    if budget.remaining_files == 0 || before.len() > budget.remaining_bytes {
        return Ok(None);
    }
    budget.remaining_files -= 1;
    budget.remaining_bytes -= before.len();
    // Limit the read itself so a file growing after metadata cannot allocate past the cap.
    let mut bytes = Vec::new();
    std::fs::File::open(&path)
        .map_err(|_| "read error")?
        .take(before.len().saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| "read error")?;
    let after = std::fs::metadata(&path).map_err(|_| "source changed while reading")?;
    if metadata_fingerprint(&before) != metadata_fingerprint(&after)
        || bytes.len() as u64 != before.len()
    {
        return Err("source changed while reading");
    }

    let stats = tokei_language.parse_from_slice(&bytes, config).summarise();
    let value = FileStats {
        code: stats.code,
        comments: stats.comments,
        blanks: stats.blanks,
    };

    {
        let mut cache = lock_cache();
        if cache.len() < budget.cache_entry_limit {
            cache.insert(cache_key, value);
        }
    }
    Ok(Some(CountedFile {
        language,
        stats: value,
    }))
}

pub(crate) fn render(
    cwd: &Path,
    workspace: &str,
    snapshot_id: usize,
    published: &crate::index::PublishedIndexSnapshot,
    files: &[ExtractedFile],
    scope: StatsScope,
) -> String {
    let selected: Vec<&ExtractedFile> = selected_files(files, &scope);
    let mut languages: BTreeMap<&'static str, LanguageStats> = BTreeMap::new();
    let mut unavailable: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut seen_paths: HashSet<String> = HashSet::new();
    let config = tokei::Config::default();
    let mut budget = CollectionBudget {
        remaining_files: STATS_REQUEST_FILE_LIMIT,
        remaining_bytes: STATS_REQUEST_BYTE_LIMIT,
        cache_entry_limit: files.len(),
    };
    let mut pending = 0;

    {
        let mut cache = lock_cache();
        prune_cache(&mut cache, workspace, snapshot_id);
    }

    for file in selected {
        let path = normalized_path(&file.file_path);
        if !seen_paths.insert(path) {
            continue;
        }
        match count_file(
            cwd,
            workspace,
            snapshot_id,
            published.source_mtime(&file.file_path),
            file,
            &config,
            &mut budget,
        ) {
            Ok(Some(counted)) => {
                languages
                    .entry(counted.language)
                    .or_default()
                    .record(counted.stats);
            }
            Ok(None) => pending += 1,
            Err(reason) => {
                *unavailable.entry(reason).or_insert(0) += 1;
            }
        }
    }

    let counted: usize = languages.values().map(|stats| stats.files).sum();
    let indexed = seen_paths.len();
    let unavailable_count: usize = unavailable.values().sum();
    let mut rows: Vec<(&'static str, &LanguageStats)> = languages
        .iter()
        .map(|(name, stats)| (*name, stats))
        .collect();
    rows.sort_by(|(left_name, left), (right_name, right)| {
        right
            .total()
            .cmp(&left.total())
            .then_with(|| left_name.cmp(right_name))
    });

    let mut output = String::new();
    output.push_str("## Indexed Statistics\n\n");
    output.push_str(&format!("Scope: {}\n", scope_description(&scope)));
    output.push_str("Population: indexed physical files only; generated macro-expansion buffers are not separate files.\n");
    output.push_str(&format!(
        "Coverage: {indexed} indexed, {counted} counted, {unavailable_count} unavailable.\n"
    ));
    if pending > 0 {
        output.push_str(&format!("Collection: partial; {pending} files pending within the request budget. Retry overview to reuse completed counts.\n"));
    } else if unavailable_count > 0 {
        output.push_str("Collection: partial; some indexed files are unavailable.\n");
    } else {
        output.push_str("Collection: complete.\n");
    }
    output.push_str("Convention: tokei 15.0.0; total = code + comments + blanks; embedded blobs are folded once into the physical file's language bucket.\n");

    if unavailable.is_empty() {
        output.push_str("Unavailable: none.\n");
    } else {
        output.push_str("Unavailable:\n");
        for (reason, count) in &unavailable {
            output.push_str(&format!("- {count} {reason}.\n"));
        }
    }

    output.push('\n');
    for (language, stats) in rows.iter().take(STATS_LANGUAGE_ROW_LIMIT) {
        output.push_str(&format!(
            "- {language}: {} code, {} comments, {} blanks, {} total ({} files)\n",
            stats.code,
            stats.comments,
            stats.blanks,
            stats.total(),
            stats.files
        ));
    }
    if rows.len() > STATS_LANGUAGE_ROW_LIMIT {
        output.push_str(&format!(
            "- {} language rows omitted by the statistics output limit; the total below still covers every counted file.\n",
            rows.len() - STATS_LANGUAGE_ROW_LIMIT
        ));
    }

    let total = rows
        .iter()
        .fold(FileStats::default(), |mut total, (_, stats)| {
            total.code += stats.code;
            total.comments += stats.comments;
            total.blanks += stats.blanks;
            total
        });
    output.push_str(&format!(
        "- Total: {} code, {} comments, {} blanks, {} total ({counted} counted files)\n",
        total.code,
        total.comments,
        total.blanks,
        total.total()
    ));

    output
}
