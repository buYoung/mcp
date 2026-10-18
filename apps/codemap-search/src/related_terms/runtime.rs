//! Startup work transfers a warmed lookup to the MCP thread through a channel.
//! No download/preparation lock is shared with request handling.
use super::evidence::{Dictionary, EvidenceHandle, EvidenceOptions, EvidenceView};
use super::fetch::{fetch_dataset, FetchStatus, PINNED_DATASET};
use super::{prepare_dataset, LookupResult, RelatedTermsLookup};
use crate::config::RelatedTermsConfig;
use crate::index::SnapshotSource;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Duration;

pub(crate) enum LookupStatus {
    Downloading,
    Preparing,
    Unavailable(String),
    Ready(Vec<LookupResult>),
}

struct PreparedLookup {
    lookup: RelatedTermsLookup,
    evidence: Result<EvidenceHandle, String>,
}

enum PreparationMessage {
    Preparing,
    Ready(Result<Box<PreparedLookup>, String>),
}

pub(crate) struct RelatedTermsRuntime {
    settings: RelatedTermsConfig,
    global_dir: PathBuf,
    source: Option<PathBuf>,
    index_source: SnapshotSource,
    evidence_options: EvidenceOptions,
    evidence: Option<EvidenceHandle>,
    evidence_failure: Option<String>,
    receiver: Option<mpsc::Receiver<PreparationMessage>>,
    is_cancelled: Arc<AtomicBool>,
    is_preparing: bool,
    lookup: Option<RelatedTermsLookup>,
    failure: Option<String>,
}

impl RelatedTermsRuntime {
    /// The caller must gate construction on enabled, including on configuration reload.
    pub(crate) fn start(settings: RelatedTermsConfig, index_source: SnapshotSource) -> Self {
        let cwd = std::env::current_dir();
        let absolute = |path: PathBuf| match &cwd {
            Ok(cwd) if path.is_relative() => cwd.join(path),
            _ => path,
        };
        let mut runtime = Self {
            global_dir: absolute(crate::config::global_dir()),
            source: settings.dataset_path.clone().map(absolute),
            settings,
            index_source,
            evidence_options: EvidenceOptions::for_runtime(),
            evidence: None,
            evidence_failure: None,
            receiver: None,
            is_cancelled: Arc::new(AtomicBool::new(false)),
            is_preparing: false,
            lookup: None,
            failure: None,
        };
        match cwd {
            Ok(_) => runtime.begin_preparation(),
            Err(error) => {
                runtime.failure = Some(format!("cannot resolve dataset working directory: {error}"))
            }
        }
        runtime
    }

    pub(crate) fn matches(&self, settings: &RelatedTermsConfig) -> bool {
        self.settings.dataset_path == settings.dataset_path
    }

    pub(crate) fn set_index_source(&mut self, source: SnapshotSource) {
        self.index_source = source;
    }

    pub(crate) fn evidence(&self) -> EvidenceView {
        if let Some(reason) = &self.evidence_failure {
            return EvidenceView::Unavailable(reason.clone());
        }
        self.evidence
            .as_ref()
            .map_or(EvidenceView::Preparing, EvidenceHandle::view)
    }

    pub(crate) fn dictionary(&self) -> Option<&Dictionary> {
        self.evidence
            .as_ref()
            .map(|handle| handle.dictionary.as_ref())
    }

    pub(crate) fn is_overlay_enabled(&self) -> bool {
        self.evidence_options.is_overlay_enabled
    }
    pub(crate) fn should_remove(&self) -> bool {
        self.evidence_options.should_remove
    }

    #[cfg(feature = "related-terms-eval")]
    pub(crate) fn configure_evaluation(&mut self, scope: &str, mode: &str) -> Result<(), String> {
        use super::evidence::EvidenceScope;
        let scope = match scope {
            "symbols" => EvidenceScope::Symbols,
            "docstrings" => EvidenceScope::Docstrings,
            "literals" => EvidenceScope::Literals,
            _ => return Err("scope must be symbols, docstrings or literals".into()),
        };
        if !matches!(mode, "off" | "demote" | "remove") {
            return Err("mode must be off, demote or remove".into());
        }
        self.evidence_options.scope = scope;
        self.evidence_options.is_overlay_enabled = mode != "off";
        self.evidence_options.should_remove = mode == "remove";
        self.synchronize_evidence();
        Ok(())
    }

    #[cfg(feature = "related-terms-eval")]
    pub(crate) fn index_generation(&self) -> u64 {
        self.index_source.receiver.borrow().generation()
    }

    fn synchronize_evidence(&mut self) {
        if let Some(handle) = &self.evidence {
            if handle.source.id != self.index_source.id
                || handle.options.scope != self.evidence_options.scope
            {
                let dictionary = Arc::clone(&handle.dictionary);
                let options = self.evidence_options.clone();
                self.evidence = None;
                match EvidenceHandle::start(self.index_source.clone(), dictionary, options) {
                    Ok(handle) => self.evidence = Some(handle),
                    Err(reason) => self.evidence_failure = Some(reason),
                }
            }
        }
    }

    fn begin_preparation(&mut self) {
        let (sender, receiver) = mpsc::channel();
        let global_dir = self.global_dir.clone();
        let source = self.source.clone();
        let index_source = self.index_source.clone();
        let evidence_options = self.evidence_options.clone();
        let is_cancelled = Arc::clone(&self.is_cancelled);
        self.is_preparing = false;
        let worker = std::thread::Builder::new()
            .name("codemap-related-terms".into())
            .spawn(move || {
                let result = (|| {
                    loop {
                        if is_cancelled.load(Ordering::Relaxed) {
                            return Err("preparation cancelled".into());
                        }
                        match fetch_dataset(&global_dir, source.as_deref(), &PINNED_DATASET) {
                            FetchStatus::Ready { path, user_owned } => {
                                if is_cancelled.load(Ordering::Relaxed) {
                                    return Err("preparation cancelled".into());
                                }
                                let _ = sender.send(PreparationMessage::Preparing);
                                let prepared = prepare_dataset(&path, &global_dir, user_owned)?;
                                if is_cancelled.load(Ordering::Relaxed) {
                                    return Err("preparation cancelled".into());
                                }
                                let mut lookup = RelatedTermsLookup::new(prepared);
                                // Warm lookup and evidence share one open file, off the MCP loop.
                                lookup.lookup("token")?;
                                let evidence =
                                    Dictionary::load(&mut lookup).and_then(|dictionary| {
                                        if is_cancelled.load(Ordering::Relaxed) {
                                            return Err("preparation cancelled".into());
                                        }
                                        EvidenceHandle::start(
                                            index_source,
                                            Arc::new(dictionary),
                                            evidence_options,
                                        )
                                    });
                                return Ok(PreparedLookup { lookup, evidence });
                            }
                            FetchStatus::InProgress => {
                                std::thread::sleep(Duration::from_millis(100))
                            }
                            FetchStatus::Failed { reason } => return Err(reason),
                        }
                    }
                })();
                let _ = sender.send(PreparationMessage::Ready(result.map(Box::new)));
            });
        match worker {
            Ok(_) => self.receiver = Some(receiver),
            Err(error) => {
                self.failure = Some(format!("cannot start related-terms preparation: {error}"))
            }
        }
    }

    fn poll(&mut self) {
        while let Some(receiver) = &self.receiver {
            match receiver.try_recv() {
                Ok(PreparationMessage::Preparing) => self.is_preparing = true,
                Ok(PreparationMessage::Ready(result)) => {
                    self.receiver = None;
                    match result {
                        Ok(prepared) => {
                            let PreparedLookup { lookup, evidence } = *prepared;
                            self.lookup = Some(lookup);
                            self.evidence_failure = None;
                            match evidence {
                                Ok(handle) => self.evidence = Some(handle),
                                Err(reason) => self.evidence_failure = Some(reason),
                            }
                        }
                        Err(error) => self.failure = Some(error),
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.receiver = None;
                    self.failure =
                        Some("related-terms preparation worker stopped unexpectedly".into());
                }
            }
        }
    }

    pub(crate) fn lookup(&mut self, terms: &[String]) -> LookupStatus {
        self.poll();
        self.synchronize_evidence();
        if let Some(reason) = self.failure.take() {
            self.lookup = None;
            self.evidence = None;
            self.evidence_failure = None;
            self.begin_preparation();
            return LookupStatus::Unavailable(reason);
        }
        if let Some(lookup) = &mut self.lookup {
            return match lookup.lookup_batch(terms) {
                Ok(answers) => LookupStatus::Ready(answers),
                Err(reason) => {
                    self.failure = Some(reason.clone());
                    LookupStatus::Unavailable(reason)
                }
            };
        }
        if self.is_preparing {
            LookupStatus::Preparing
        } else {
            LookupStatus::Downloading
        }
    }
}

impl Drop for RelatedTermsRuntime {
    fn drop(&mut self) {
        // An already-started bounded HTTP request may finish, but cannot start preparation
        // or retry after disable/shutdown. Never join a downloading thread on the MCP loop.
        self.is_cancelled.store(true, Ordering::Relaxed);
    }
}
