//! Host boundary for the optional Jev stages: credential resolution from the configured
//! environment variable, the shared HTTPS evaluator lifecycle, the one absolute deadline
//! per tool call, and the per-tool glue that turns adapter results into response text plus
//! secret-free stderr diagnostics. Nothing here runs unless a stage is enabled in
//! `[analysis.jev]`. Enabled filters use the task registered for this connection; missing
//! registration is an error. Other bypasses/fallbacks preserve the base output. The record of what
//! happened is the `jev stage` diagnostic line, never inline text.

use crate::config::{JevConfig, ResolvedConfig};
use crate::jev::{Evaluator, JevEvaluator, SecretString, Timing, Usage};
use crate::tools::search::jev::FilterPolicy;
use crate::tools::search::SearchOutput;
use crate::tools::ToolContext;
use std::sync::Arc;
use tokio::time::Instant;

/// The transport-relevant settings an HTTPS evaluator was built with. A change rebuilds
/// the evaluator on the next enabled request; the enable flags and the filter threshold are
/// read per request and never require a rebuild.
#[derive(Clone, Debug, PartialEq, Eq)]
struct TransportFingerprint {
    model: String,
    api_key_env: String,
    timeout_ms: u64,
    max_in_flight_requests: usize,
    request_spacing_ms: u64,
    max_batch_bytes: usize,
    pool_idle_timeout_ms: u64,
}

impl TransportFingerprint {
    fn of(config: &JevConfig) -> Self {
        Self {
            model: config.model.clone(),
            api_key_env: config.api_key_env.clone(),
            timeout_ms: config.timeout_ms,
            max_in_flight_requests: config.max_in_flight_requests,
            request_spacing_ms: config.request_spacing_ms,
            max_batch_bytes: config.max_batch_bytes,
            pool_idle_timeout_ms: config.pool_idle_timeout_ms,
        }
    }
}

struct BuiltEvaluator {
    fingerprint: TransportFingerprint,
    api_key: SecretString,
    evaluator: Arc<JevEvaluator>,
}

/// Owns the evaluator used by the enabled tools for the lifetime of the server: the HTTPS
/// evaluator (connection pool, in-flight permits, request spacing) is built lazily on the
/// first enabled request that has credentials and reused until its settings change. Tests
/// inject an offline evaluator instead; an injected evaluator never touches the environment.
#[derive(Default)]
pub struct JevHost {
    injected: Option<Arc<dyn Evaluator>>,
    built: Option<BuiltEvaluator>,
}

impl JevHost {
    pub fn injected(evaluator: Arc<dyn Evaluator>) -> Self {
        Self {
            injected: Some(evaluator),
            built: None,
        }
    }

    /// Resolve the evaluator for one request. `Err(reason)` is a bypass label
    /// (`missing_credentials`, `invalid_config`); nothing is sent in that case.
    pub fn resolve(&mut self, config: &JevConfig) -> Result<Arc<dyn Evaluator>, &'static str> {
        if let Some(injected) = &self.injected {
            return Ok(Arc::clone(injected));
        }
        let api_key = match std::env::var(&config.api_key_env) {
            Ok(value) if !value.trim().is_empty() => SecretString::new(value),
            _ => return Err("missing_credentials"),
        };
        let fingerprint = TransportFingerprint::of(config);
        let is_current = self.built.as_ref().is_some_and(|built| {
            built.fingerprint == fingerprint && built.api_key.expose() == api_key.expose()
        });
        if !is_current {
            let evaluator = JevEvaluator::https(
                api_key.clone(),
                config.evaluator_config(),
                config.https_settings(),
            )
            .map_err(|error| {
                tracing::warn!(kind = error.kind(), "jev evaluator unavailable: {error}");
                "invalid_config"
            })?;
            self.built = Some(BuiltEvaluator {
                fingerprint,
                api_key,
                evaluator: Arc::new(evaluator),
            });
        }
        let built = self.built.as_ref().expect("evaluator was just built");
        Ok(Arc::clone(&built.evaluator) as Arc<dyn Evaluator>)
    }
}

/// The one absolute deadline of a tool call's Jev work, fixed when the stage's
/// preparation starts and shared by every request of that call. `None` when the
/// configured timeout cannot be represented on the monotonic clock (the stage bypasses).
fn deadline_at(config: &JevConfig) -> Option<Instant> {
    Instant::now().checked_add(config.deadline())
}

/// Everything one `jev stage` diagnostic line carries. No field ever holds the intent,
/// evidence, provider error text or a credential.
struct StageLog<'a> {
    tool: &'static str,
    outcome: &'a str,
    status: &'a str,
    detail: &'a str,
    model: &'a str,
    versions: &'a str,
    usage: Usage,
    timing: Timing,
    attempts: usize,
}

/// One secret-free log line per stage run: what happened, what it cost, how long it took.
fn log_stage(log: StageLog<'_>) {
    tracing::info!(
        tool = log.tool,
        outcome = log.outcome,
        status = log.status,
        detail = log.detail,
        model = log.model,
        versions = log.versions,
        input_tokens = ?log.usage.input_tokens,
        output_tokens = ?log.usage.output_tokens,
        reported_responses = log.usage.reported_responses,
        unreported_responses = log.usage.unreported_responses,
        attempts = log.attempts,
        elapsed_ms = log.timing.elapsed.as_secs_f64() * 1000.0,
        http_ms = log.timing.http.as_secs_f64() * 1000.0,
        queue_ms = log.timing.queue_wait.as_secs_f64() * 1000.0,
        "jev stage"
    );
}

fn log_bypass(tool: &'static str, reason: &str, model: &str) {
    log_stage(StageLog {
        tool,
        outcome: "bypassed",
        status: reason,
        detail: "",
        model,
        versions: "",
        usage: Usage::default(),
        timing: Timing::default(),
        attempts: 0,
    });
}

/// Mode #2 for one `search` call: the structured body filter over the selected evidence,
/// using the registered task. Missing registration is an error; unavailable credentials,
/// ineligible evidence or provider failures preserve the byte-identical plain output.
pub(super) async fn search(
    host: &mut JevHost,
    ctx: &ToolContext<'_>,
    task: Option<&crate::tools::task::RegisteredTask>,
    config: &ResolvedConfig,
) -> Result<SearchOutput, (i64, String)> {
    let jev = &config.jev;
    let model = jev.model.as_str();
    let task = require_task(task, "search")?;
    let Some(deadline_at) = deadline_at(jev) else {
        log_bypass("search", "invalid_config", model);
        return crate::tools::search::run_with_metadata(ctx);
    };
    let evaluator = match host.resolve(jev) {
        Ok(evaluator) => evaluator,
        Err(reason) => {
            log_bypass("search", reason, model);
            return crate::tools::search::run_with_metadata(ctx);
        }
    };
    let policy = FilterPolicy {
        min_unrelated_probability: jev.search_filter_min_unrelated_probability,
        max_group_bytes: jev.max_batch_bytes,
        model: jev.model.clone(),
        deadline_at: Some(deadline_at),
        cancel: None,
    };
    let (output, result) =
        crate::tools::search::run_with_filter(ctx, task, evaluator.as_ref(), &policy).await?;
    if let Some(result) = result {
        log_filter_result("search", result, model);
    } else {
        let reason = if ctx.arguments.get("event_key").is_some() {
            "exact_event_map"
        } else {
            "no_ranked_candidates"
        };
        log_bypass("search", reason, model);
    }
    Ok(output)
}

fn require_task<'a>(
    task: Option<&'a crate::tools::task::RegisteredTask>,
    tool: &str,
) -> Result<&'a crate::tools::task::RegisteredTask, (i64, String)> {
    task.ok_or_else(|| (-32602, format!("Jev {tool} requires task_query and questions registered through initial_instructions. Register the user's complete task and focused yes/no criteria, then retry.")))
}

fn log_filter_result(
    tool: &'static str,
    result: crate::tools::search::jev::FilterResult,
    model: &str,
) {
    // Numeric provider answers and their consumed decisions are reproducible without
    // logging task text, source, user criterion ids or credentials.
    for judgment in &result.judgments {
        let decision = &result.decisions[judgment.entity];
        tracing::info!(group_index=judgment.group, candidate_index=judgment.entity,
            criterion_index=judgment.criterion, yes_probability=judgment.noul.noul,
            match_state=?decision.match_state, is_retained=decision.is_retained,
            reason=decision.reason.as_ref().map_or("no_match", |reason| reason.label()),
            "jev consumed judgment");
    }
    let detail = format!(
        "{} note_inline={}",
        result.diagnostic(),
        result.is_note_inline
    );
    let versions = format!(
        "{} {} {}",
        result.evidence_version, result.question_version, result.policy_version
    );
    let status = match &result.status {
        crate::tools::search::jev::FilterStatus::Applied { .. } => "applied".to_string(),
        crate::tools::search::jev::FilterStatus::Bypassed(reason) => {
            format!("bypassed:{reason}")
        }
        crate::tools::search::jev::FilterStatus::Fallback(kind) => format!("fallback:{kind}"),
    };
    log_stage(StageLog {
        tool,
        outcome: result.status.label(),
        status: &status,
        detail: &detail,
        model,
        versions: &versions,
        usage: result.usage,
        timing: result.timing,
        attempts: result.requests.len(),
    });
}

/// Live tools retain their filesystem permissions, selected windows/pages and final base
/// rendering. Every enabled source request uses the registered task, without a per-call opt-in.
pub(super) async fn live(
    host: &mut JevHost,
    engine: &crate::index::EngineSupervisor,
    tool: &'static str,
    arguments: &serde_json::Value,
    task: Option<&crate::tools::task::RegisteredTask>,
    config: &ResolvedConfig,
) -> Result<crate::tools::live_symbols::PreparedLiveOutput, (i64, String)> {
    use crate::tools::live_options::{LiveOptions, LiveView};
    use crate::tools::live_symbols::{self, PreparedLiveOutput};
    crate::tools::reject_body_task_query(arguments)?;
    let is_read = tool == "read";
    let options = if is_read {
        LiveOptions::parse(arguments)?
    } else {
        LiveOptions::parse_grep(arguments)?
    };
    let is_content = is_read
        || arguments
            .get("output_mode")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("content")
            == "content";
    let is_enabled = if is_read {
        config.jev.read_filter_enabled
    } else {
        config.jev.grep_filter_enabled
    };
    let is_source = is_content
        && matches!(
            options.view,
            LiveView::Full | LiveView::Source | LiveView::SourceGrouped
        );
    let started = Instant::now();
    let mut evaluation = None;
    if is_enabled && is_source {
        require_task(task, tool)?;
        if let Some(deadline_at) = deadline_at(&config.jev) {
            match host.resolve(&config.jev) {
                Ok(evaluator) => {
                    evaluation = Some((
                        evaluator,
                        FilterPolicy {
                            min_unrelated_probability: config
                                .jev
                                .search_filter_min_unrelated_probability,
                            max_group_bytes: config.jev.max_batch_bytes,
                            model: config.jev.model.clone(),
                            deadline_at: Some(deadline_at),
                            cancel: None,
                        },
                    ))
                }
                Err(reason) => log_bypass(tool, reason, &config.jev.model),
            }
        } else {
            log_bypass(tool, "invalid_config", &config.jev.model);
        }
    }
    let should_capture = evaluation.is_some();
    let output = match (is_read, should_capture) {
        (true, true) => crate::tools::read::read_file_for_filter(arguments)?,
        (true, false) => crate::tools::read::read_file_with_metadata(arguments)?,
        (false, true) => crate::tools::grep::grep_for_filter(arguments)?,
        (false, false) => crate::tools::grep::grep_with_metadata(arguments)?,
    };
    if !is_content {
        return Ok(PreparedLiveOutput {
            text: output.text,
            source_files: Vec::new(),
            capture: Default::default(),
            copies: Vec::new(),
        });
    }
    let cap = if is_read {
        Some(config.read_output_byte_cap)
    } else {
        options
            .should_expand_callable
            .then_some(config.grep_output_byte_cap)
    };
    let mut prepared = live_symbols::prepare(engine, output, cap, options)?;
    if evaluation.is_some()
        && !is_read
        && config
            .grep_response_byte_cap
            .is_some_and(|cap| prepared.text.len() > cap)
    {
        // Let the ordinary post-redaction response-cap check decide the original outcome.
        // Do not send evidence from an already oversized base merely to make it fit.
        log_bypass(tool, "base_output_over_cap", &config.jev.model);
        return Ok(prepared);
    }
    if let Some((evaluator, policy)) = evaluation {
        let plan = std::mem::take(&mut prepared.capture).prepare(
            &prepared.text,
            &prepared.copies,
            require_task(task, tool)?,
            arguments,
        );
        let mut result =
            crate::tools::search::jev::evaluate(&plan.input, evaluator.as_ref(), &policy).await;
        plan.apply(&mut prepared.text, &mut prepared.source_files, &mut result);
        result.timing.elapsed = started.elapsed();
        log_filter_result(tool, result, &config.jev.model);
    }
    Ok(prepared)
}
