//! Dictionary-only evidence over published index snapshots, computed off the MCP loop.
use super::normalize::word_key;
use super::RelatedTermsLookup;
use crate::index::{PublishedIndexSnapshot, SnapshotSource};
use crate::parser::ExtractedFile;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::watch;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EvidenceScope {
    #[default]
    Symbols,
    #[cfg(feature = "related-terms-eval")]
    Docstrings,
    #[cfg(feature = "related-terms-eval")]
    Literals,
}

impl EvidenceScope {
    fn includes_literals(self) -> bool {
        #[cfg(feature = "related-terms-eval")]
        {
            matches!(self, Self::Literals)
        }
        #[cfg(not(feature = "related-terms-eval"))]
        {
            false
        }
    }
}

// Keep removal off until independent judgments and the user's threshold decision exist.
const SHOULD_REMOVE_UNSUPPORTED_SENSES: bool = false;

#[derive(Clone)]
pub(crate) struct EvidenceOptions {
    pub scope: EvidenceScope,
    pub is_overlay_enabled: bool,
    pub should_remove: bool,
    #[cfg(feature = "related-terms-eval")]
    pub report_path: Option<std::path::PathBuf>,
}

impl Default for EvidenceOptions {
    fn default() -> Self {
        Self {
            scope: EvidenceScope::Symbols,
            is_overlay_enabled: true,
            should_remove: SHOULD_REMOVE_UNSUPPORTED_SENSES,
            #[cfg(feature = "related-terms-eval")]
            report_path: None,
        }
    }
}

impl EvidenceOptions {
    pub fn for_runtime() -> Self {
        #[cfg(feature = "related-terms-eval")]
        {
            let scope = match std::env::var("CODEMAP_RELATED_TERMS_EVAL_SCOPE").as_deref() {
                Ok("docstrings") => EvidenceScope::Docstrings,
                Ok("literals") => EvidenceScope::Literals,
                _ => EvidenceScope::Symbols,
            };
            Self {
                scope,
                is_overlay_enabled: true,
                should_remove: std::env::var("CODEMAP_RELATED_TERMS_EVAL_REMOVE").as_deref()
                    == Ok("1"),
                report_path: std::env::var_os("CODEMAP_RELATED_TERMS_EVAL_REPORT")
                    .map(std::path::PathBuf::from),
            }
        }
        #[cfg(not(feature = "related-terms-eval"))]
        {
            Self::default()
        }
    }
}

pub(crate) struct Dictionary {
    pub keys: HashSet<String>,
    pub fields_by_word: HashMap<String, Vec<String>>,
}

impl Dictionary {
    pub fn load(lookup: &mut RelatedTermsLookup) -> Result<Self, String> {
        let connection = lookup.connection()?;
        let mut statement = connection
            .prepare(
                "SELECT word_key FROM terms UNION SELECT word_key FROM relations
            UNION SELECT word_key FROM input_senses UNION SELECT input_word_key FROM input_relations
            UNION SELECT related_word_key FROM input_relations",
            )
            .map_err(|error| error.to_string())?;
        let keys = statement
            .query_map([], |row| row.get(0))
            .map_err(|error| error.to_string())?
            .collect::<Result<HashSet<String>, _>>()
            .map_err(|error| error.to_string())?;
        let mut statement = connection.prepare("SELECT tf.field,t.word_key FROM personas p JOIN terms t USING(term_id)
            JOIN topic_fields tf ON tf.topic=p.topic AND tf.rank=0
            UNION SELECT tf.field,r.word_key FROM relations r JOIN personas p USING(persona)
            JOIN topic_fields tf ON tf.topic=p.topic AND tf.rank=0
            UNION SELECT tf.field,i.word_key FROM input_senses i JOIN personas p USING(persona)
            JOIN topic_fields tf ON tf.topic=p.topic AND tf.rank=0
            UNION SELECT tf.field,i.related_word_key FROM input_relations i JOIN personas p ON p.persona=i.source_persona
            JOIN topic_fields tf ON tf.topic=p.topic AND tf.rank=0").map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|error| error.to_string())?;
        let mut fields_by_word: HashMap<String, Vec<String>> = HashMap::new();
        for row in rows {
            let (field, key) = row.map_err(|error| error.to_string())?;
            fields_by_word.entry(key).or_default().push(field);
        }
        Ok(Self {
            keys,
            fields_by_word,
        })
    }
}

#[derive(Clone, Debug, Default, Serialize)]
pub(crate) struct WordEvidence {
    pub file_count: u64,
    pub occurrence_count: u64,
}

#[derive(Clone, Serialize)]
pub(crate) struct EvidenceSnapshot {
    pub generation: u64,
    pub scope: EvidenceScope,
    pub file_count: usize,
    pub changed_file_count: usize,
    pub removed_file_count: usize,
    pub elapsed_ms: f64,
    pub index_pass_ms: f64,
    pub words: BTreeMap<String, WordEvidence>,
    pub field_word_counts: BTreeMap<String, usize>,
}

struct FileEvidence {
    fingerprint: blake3::Hash,
    words: HashMap<String, u64>,
}

#[derive(Default)]
struct EvidenceStore {
    generation: u64,
    files: HashMap<String, FileEvidence>,
    words: BTreeMap<String, WordEvidence>,
    field_word_counts: BTreeMap<String, usize>,
}

fn texts(file: &ExtractedFile, scope: EvidenceScope) -> impl Iterator<Item = (&str, bool)> {
    let docstrings = if matches!(scope, EvidenceScope::Symbols) {
        &[][..]
    } else {
        &file.docstrings[..]
    };
    let literals = if scope.includes_literals() {
        &file.literals[..]
    } else {
        &[][..]
    };
    // Data members count as declarations: languages that index them as symbols already do.
    file.symbols
        .iter()
        .map(|symbol| (symbol.name.as_str(), true))
        .chain(file.member_names.iter().map(|name| (name.as_str(), true)))
        .chain(docstrings.iter().map(|text| (text.as_str(), false)))
        .chain(
            literals
                .iter()
                .map(|literal| (literal.text.as_str(), false)),
        )
}

fn fingerprint(file: &ExtractedFile, scope: EvidenceScope) -> blake3::Hash {
    let mut hasher = blake3::Hasher::new();
    for (text, is_symbol) in texts(file, scope) {
        hasher.update(&[u8::from(is_symbol)]);
        hasher.update(&(text.len() as u64).to_le_bytes());
        hasher.update(text.as_bytes());
    }
    hasher.finalize()
}

fn collect_words(
    file: &ExtractedFile,
    scope: EvidenceScope,
    dictionary: &Dictionary,
) -> HashMap<String, u64> {
    let mut counts = HashMap::new();
    for (text, is_symbol) in texts(file, scope) {
        let tokens = if is_symbol {
            crate::parser::split_identifier(text)
        } else {
            text.split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '-'))
                .filter(|part| !part.is_empty())
                .flat_map(crate::parser::split_identifier)
                .collect::<Vec<_>>()
        };
        for start in 0..tokens.len() {
            let mut phrase = String::new();
            for token in tokens.iter().skip(start).take(4) {
                if !phrase.is_empty() {
                    phrase.push(' ');
                }
                phrase.push_str(token);
                let key = word_key(&phrase);
                if dictionary.keys.contains(&key) {
                    *counts.entry(key).or_default() += 1;
                }
            }
        }
    }
    counts
}

impl EvidenceStore {
    fn remove_file(&mut self, path: &str, dictionary: &Dictionary) {
        let Some(file) = self.files.remove(path) else {
            return;
        };
        for (key, occurrences) in file.words {
            if let Some(counts) = self.words.get_mut(&key) {
                counts.file_count -= 1;
                counts.occurrence_count -= occurrences;
                if counts.file_count == 0 {
                    self.words.remove(&key);
                    for field in dictionary.fields_by_word.get(&key).into_iter().flatten() {
                        if let Some(count) = self.field_word_counts.get_mut(field) {
                            *count -= 1;
                            if *count == 0 {
                                self.field_word_counts.remove(field);
                            }
                        }
                    }
                }
            }
        }
    }

    fn update(
        &mut self,
        snapshot: &PublishedIndexSnapshot,
        dictionary: &Dictionary,
        scope: EvidenceScope,
    ) -> EvidenceSnapshot {
        let started = Instant::now();
        let mut removed = self.files.keys().cloned().collect::<HashSet<_>>();
        let mut changed_file_count = 0;
        for file in snapshot.codemap().iter() {
            removed.remove(&file.file_path);
            let fingerprint = fingerprint(file, scope);
            if self
                .files
                .get(&file.file_path)
                .is_some_and(|old| old.fingerprint == fingerprint)
            {
                continue;
            }
            changed_file_count += 1;
            self.remove_file(&file.file_path, dictionary);
            let words = collect_words(file, scope, dictionary);
            for (key, occurrences) in &words {
                let counts = self.words.entry(key.clone()).or_default();
                if counts.file_count == 0 {
                    for field in dictionary.fields_by_word.get(key).into_iter().flatten() {
                        *self.field_word_counts.entry(field.clone()).or_default() += 1;
                    }
                }
                counts.file_count += 1;
                counts.occurrence_count += occurrences;
            }
            self.files
                .insert(file.file_path.clone(), FileEvidence { fingerprint, words });
        }
        for path in &removed {
            self.remove_file(path, dictionary);
        }
        self.generation = snapshot.generation();
        let words = self.words.clone();
        let field_word_counts = self.field_word_counts.clone();
        EvidenceSnapshot {
            generation: self.generation,
            scope,
            file_count: self.files.len(),
            changed_file_count,
            removed_file_count: removed.len(),
            elapsed_ms: started.elapsed().as_secs_f64() * 1000.0,
            index_pass_ms: snapshot.index_pass_ms(),
            words,
            field_word_counts,
        }
    }
}

pub(crate) enum EvidenceView {
    Preparing,
    Unavailable(String),
    Ready(Arc<EvidenceSnapshot>),
}

pub(crate) struct EvidenceHandle {
    pub source: SnapshotSource,
    pub dictionary: Arc<Dictionary>,
    pub options: EvidenceOptions,
    results: watch::Receiver<Option<Result<Arc<EvidenceSnapshot>, String>>>,
    _cancellation: watch::Sender<()>,
}

impl EvidenceHandle {
    pub fn start(
        source: SnapshotSource,
        dictionary: Arc<Dictionary>,
        options: EvidenceOptions,
    ) -> Result<Self, String> {
        let (result_sender, results) = watch::channel(None);
        let (cancellation, mut cancelled) = watch::channel(());
        let mut publications = source.receiver.clone();
        let thread_dictionary = Arc::clone(&dictionary);
        let thread_options = options.clone();
        std::thread::Builder::new()
            .name("codemap-related-evidence".into())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_current_thread().build() {
                    Ok(runtime) => runtime,
                    Err(error) => {
                        result_sender.send_replace(Some(Err(error.to_string())));
                        return;
                    }
                };
                runtime.block_on(async move {
                    let mut store = EvidenceStore::default();
                    loop {
                        let snapshot = publications.borrow_and_update().clone();
                        if snapshot.generation() > store.generation {
                            let evidence = Arc::new(store.update(
                                &snapshot,
                                &thread_dictionary,
                                thread_options.scope,
                            ));
                            if cancelled.has_changed().is_err() {
                                break;
                            }
                            // A newer publish wins even if it arrived during a long computation.
                            if publications.borrow().generation() == evidence.generation {
                                #[cfg(feature = "related-terms-eval")]
                                if let Some(path) = &thread_options.report_path {
                                    if let Err(error) = write_evaluation_report(path, &evidence) {
                                        result_sender.send_replace(Some(Err(error)));
                                        break;
                                    }
                                }
                                result_sender.send_replace(Some(Ok(evidence)));
                            }
                        }
                        if publications.has_changed().unwrap_or(false) {
                            continue;
                        }
                        tokio::select! {
                            _ = cancelled.changed() => break,
                            changed = publications.changed() => if changed.is_err() { break; },
                        }
                    }
                });
            })
            .map_err(|error| format!("cannot start codebase evidence: {error}"))?;
        Ok(Self {
            source,
            dictionary,
            options,
            results,
            _cancellation: cancellation,
        })
    }

    pub fn view(&self) -> EvidenceView {
        let generation = self.source.receiver.borrow().generation();
        match self.results.borrow().clone() {
            Some(Err(error)) => EvidenceView::Unavailable(error),
            _ if self.results.has_changed().is_err() => {
                EvidenceView::Unavailable("evidence worker stopped".into())
            }
            Some(Ok(snapshot)) if snapshot.generation == generation => {
                EvidenceView::Ready(snapshot)
            }
            _ => EvidenceView::Preparing,
        }
    }
}

#[cfg(feature = "related-terms-eval")]
fn write_evaluation_report(
    path: &std::path::Path,
    evidence: &EvidenceSnapshot,
) -> Result<(), String> {
    use std::io::Write;
    let parent = path.parent().ok_or("evaluation report has no directory")?;
    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|error| error.to_string())?;
    serde_json::to_writer(file.as_file_mut(), evidence).map_err(|error| error.to_string())?;
    file.flush().map_err(|error| error.to_string())?;
    file.persist(path).map_err(|error| error.to_string())?;
    Ok(())
}
