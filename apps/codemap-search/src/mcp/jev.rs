//! Native activation of the optional Jev modes inside MCP `overview` and `search` calls.
//!
//! A mode runs only when the request's pinned config enables it, the call carries an
//! explicit `task_query`, and the environment variable named by the global
//! `analysis.jev.api_key_env` holds a key; the key is read at that point, per call. Nothing
//! is sent at startup or by enablement alone. One evaluator is shared per transport policy
//! and credential, so its in-flight permits and request spacing apply across calls.
//!
//! Evaluation is awaited inside the existing sequential request boundary: the pinned config
//! and redaction scope of `McpServer::handle_request` stay active across the awaits, the
//! threshold and deadline are captured before the first await, and the prepared snapshot or
//! search output is owned, so no index or config lock is held during network work.

use std::sync::Arc;
use std::time::Duration;

use tokio::time::Instant;

use crate::config::JevConfig;
use crate::jev::{ApiKey, Evaluator, JevError, JevEvaluator, Timing, TransportPolicy, Usage};
use crate::tools::jev_note;
use crate::tools::overview::jev::{self as overview_jev, RecommendOptions, RecommendationOutcome};
use crate::tools::overview::PreparedOverview;
use crate::tools::search::jev::{self as search_jev, FilterOptions, FilterOutcome};
use crate::tools::search::{PreparedSearch, SearchOutput};

#[cfg(debug_assertions)]
mod scripted;

/// Where evaluator requests go. Release builds only have the provider endpoint.
enum TransportSource {
    Https,
    /// Offline answers for the end-to-end tests; see [`scripted`].
    #[cfg(debug_assertions)]
    Scripted(Arc<scripted::ScriptedTransport>),
}

impl TransportSource {
    fn from_environment() -> Self {
        #[cfg(debug_assertions)]
        if let Some(transport) = scripted::ScriptedTransport::from_environment() {
            return Self::Scripted(Arc::new(transport));
        }
        Self::Https
    }
}

struct SharedEvaluator {
    policy: TransportPolicy,
    api_key: ApiKey,
    evaluator: Arc<dyn JevEvaluator>,
}

/// Why a call has no evaluator.
enum Unavailable {
    /// The key variable is unset or blank; nothing was attempted.
    MissingCredential,
    /// The key or policy is unusable, or the client could not be built.
    Failed(JevError),
}

pub(crate) struct JevHost {
    transport: TransportSource,
    shared: Option<SharedEvaluator>,
}

impl JevHost {
    pub(crate) fn from_environment() -> Self {
        Self {
            transport: TransportSource::from_environment(),
            shared: None,
        }
    }

    /// The overview text of this call: the base text, plus either the recommendation
    /// section or an outcome note when the overview mode is enabled.
    pub(crate) async fn overview(
        &mut self,
        prepared: PreparedOverview,
        task_query: Option<&str>,
    ) -> String {
        let config = crate::config::get();
        let settings = &config.jev;
        let mut text = prepared.text;
        if !settings.is_overview_enabled {
            return text;
        }
        let output_cap = config.overview_output_byte_cap;
        let bypass = |text: &mut String, reason: &str| {
            log_bypass("overview", reason);
            overview_jev::append_note(text, &jev_note::bypassed("overview", reason), output_cap);
        };
        let Some(task_query) = task_query else {
            // Only the root view could have used an intent; point that out there.
            if prepared.root_snapshot.is_some() {
                bypass(&mut text, "no_task_query");
            }
            return text;
        };
        if prepared.is_index_warming {
            bypass(&mut text, "index_warming");
            return text;
        }
        let Some(snapshot) = prepared.root_snapshot else {
            bypass(&mut text, "not_repository_root");
            return text;
        };
        let evaluator = match self.evaluator(settings) {
            Ok(evaluator) => evaluator,
            Err(Unavailable::MissingCredential) => {
                bypass(&mut text, "missing_credential");
                return text;
            }
            Err(Unavailable::Failed(error)) => {
                let note = unavailable_note("overview", &error);
                overview_jev::append_note(&mut text, &note, output_cap);
                return text;
            }
        };
        let candidates = overview_jev::RootCandidates::project(&snapshot);
        let options = RecommendOptions {
            task_query: task_query.to_string(),
            deadline: deadline(settings),
            cancellation: None,
            output_room_bytes: output_cap.map(|cap| cap.saturating_sub(text.len())),
        };
        match overview_jev::recommend(&candidates, &text, evaluator.as_ref(), options).await {
            RecommendationOutcome::Applied(recommendation) => {
                tracing::info!(
                    tool = "overview",
                    outcome = "applied",
                    status = recommendation.status.label(),
                    evaluated_files = recommendation.evaluated_files,
                    recommended_files = recommendation.recommended.len(),
                    requests = recommendation.request_count,
                    input_tokens = recommendation.usage.input_tokens,
                    output_tokens = recommendation.usage.output_tokens,
                    elapsed_ms = recommendation.timing.elapsed.as_millis() as u64,
                    http_ms = recommendation.timing.http_elapsed.as_millis() as u64,
                    versions = ?recommendation.versions(),
                    "jev evaluation"
                );
                text.push_str(&recommendation.section);
            }
            RecommendationOutcome::OutputBudgetBypass => bypass(&mut text, "output_budget"),
            RecommendationOutcome::Fallback(fallback) => {
                log_fallback(
                    "overview",
                    &fallback.error,
                    fallback.usage,
                    fallback.timing,
                    fallback.request_count,
                );
                overview_jev::append_note(&mut text, &fallback.note(), output_cap);
            }
        }
        text
    }

    /// The search output of this call: the regular output, the filtered output with its
    /// omission summary, or the regular output with an outcome note.
    pub(crate) async fn search(
        &mut self,
        prepared: PreparedSearch,
        task_query: Option<&str>,
    ) -> SearchOutput {
        let config = crate::config::get();
        let settings = &config.jev;
        if !settings.is_search_filter_enabled {
            return prepared.into_output();
        }
        let byte_cap = prepared.byte_cap();
        let bypass = |prepared: PreparedSearch, reason: &str| {
            log_bypass("search", reason);
            with_note(
                prepared.into_output(),
                &jev_note::bypassed("search", reason),
                byte_cap,
            )
        };
        let Some(task_query) = task_query else {
            return bypass(prepared, "no_task_query");
        };
        if let Some(reason) = search_jev::bypass_reason(&prepared) {
            log_bypass("search", reason.label());
            return with_note(prepared.into_output(), &reason.note(), byte_cap);
        }
        let evaluator = match self.evaluator(settings) {
            Ok(evaluator) => evaluator,
            Err(Unavailable::MissingCredential) => return bypass(prepared, "missing_credential"),
            Err(Unavailable::Failed(error)) => {
                let note = unavailable_note("search", &error);
                return with_note(prepared.into_output(), &note, byte_cap);
            }
        };
        let options = FilterOptions {
            task_query: task_query.to_string(),
            min_unrelated_probability: settings.search_filter_min_unrelated_probability,
            deadline: deadline(settings),
            cancellation: None,
            max_batch_bytes: settings.max_batch_bytes,
        };
        match search_jev::filter(&prepared, evaluator.as_ref(), options).await {
            FilterOutcome::Applied(filter) => {
                tracing::info!(
                    tool = "search",
                    outcome = "applied",
                    model = %filter.evaluation.model,
                    omitted = filter.decision.omitted_count(),
                    delivered_bodies = filter.evidence.delivered_body_count(),
                    judged = filter.judgments.len(),
                    threshold = filter.decision.min_unrelated_probability,
                    completeness = ?filter.evidence.completeness_counts(),
                    retention = ?filter.decision.retention_counts(),
                    requests = filter.request_count,
                    input_tokens = filter.usage.input_tokens,
                    output_tokens = filter.usage.output_tokens,
                    elapsed_ms = filter.timing.elapsed.as_millis() as u64,
                    http_ms = filter.timing.http_elapsed.as_millis() as u64,
                    versions = ?filter.versions(),
                    "jev evaluation"
                );
                search_jev::render(&prepared, &filter).unwrap_or_else(|| {
                    tracing::warn!(
                        tool = "search",
                        "jev result discarded: its note does not fit output.search.max_bytes"
                    );
                    prepared.into_output()
                })
            }
            FilterOutcome::Bypassed(reason) => {
                log_bypass("search", reason.label());
                with_note(prepared.into_output(), &reason.note(), byte_cap)
            }
            FilterOutcome::Fallback(fallback) => {
                log_fallback(
                    "search",
                    &fallback.error,
                    fallback.usage,
                    fallback.timing,
                    fallback.request_count,
                );
                with_note(prepared.into_output(), &fallback.note(), byte_cap)
            }
        }
    }

    /// The shared evaluator for the call's policy and the key currently in the environment.
    fn evaluator(&mut self, settings: &JevConfig) -> Result<Arc<dyn JevEvaluator>, Unavailable> {
        let api_key = match std::env::var(&settings.api_key_env) {
            Ok(value) if value.trim().is_empty() => return Err(Unavailable::MissingCredential),
            Ok(value) => ApiKey::new(value).map_err(Unavailable::Failed)?,
            Err(std::env::VarError::NotPresent) => return Err(Unavailable::MissingCredential),
            Err(std::env::VarError::NotUnicode(_)) => {
                return Err(Unavailable::Failed(JevError::InvalidCredential {
                    reason: "the API key must be printable ASCII without spaces",
                }))
            }
        };
        let policy = settings.transport_policy();
        if let Some(shared) = &self.shared {
            if shared.policy == policy && shared.api_key == api_key {
                return Ok(Arc::clone(&shared.evaluator));
            }
        }
        let evaluator: Arc<dyn JevEvaluator> = match &self.transport {
            TransportSource::Https => {
                Arc::new(Evaluator::https(&api_key, policy.clone()).map_err(Unavailable::Failed)?)
            }
            #[cfg(debug_assertions)]
            TransportSource::Scripted(transport) => Arc::new(
                Evaluator::new(Arc::clone(transport), policy.clone())
                    .map_err(Unavailable::Failed)?,
            ),
        };
        self.shared = Some(SharedEvaluator {
            policy,
            api_key,
            evaluator: Arc::clone(&evaluator),
        });
        Ok(evaluator)
    }
}

/// The configured per-call deadline; the evaluator never extends it.
fn deadline(settings: &JevConfig) -> Instant {
    Instant::now() + Duration::from_millis(settings.timeout_ms)
}

/// Search notes must fit the search output cap; the regular output is kept whole otherwise.
fn with_note(mut output: SearchOutput, note: &str, byte_cap: usize) -> SearchOutput {
    if output.text.len() + note.len() <= byte_cap {
        output.text.push_str(note);
    }
    output
}

/// A fallback note for an evaluator that could not be created; no request was sent.
fn unavailable_note(tool: &str, error: &JevError) -> String {
    log_fallback(tool, error, Usage::default(), Timing::default(), 0);
    jev_note::fallback(tool, error.label(), Usage::default(), Timing::default(), 0)
}

fn log_bypass(tool: &str, reason: &str) {
    tracing::info!(tool, outcome = "bypassed", reason, "jev evaluation");
}

fn log_fallback(tool: &str, error: &JevError, usage: Usage, timing: Timing, request_count: usize) {
    tracing::warn!(
        tool,
        outcome = "fallback",
        reason = error.label(),
        error = %error,
        requests = request_count,
        input_tokens = usage.input_tokens,
        output_tokens = usage.output_tokens,
        elapsed_ms = timing.elapsed.as_millis() as u64,
        http_ms = timing.http_elapsed.as_millis() as u64,
        "jev evaluation"
    );
}
