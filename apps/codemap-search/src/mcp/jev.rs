//! Host-owned activation, credentials, and bounded diagnostics for native tool calls.
use crate::jev::{EvaluationOptions, Evaluator, HttpsTransport, Metrics, Policy, Runtime};
use serde_json::{json, Value};
use std::sync::Arc;

#[derive(Default)]
pub(super) struct EvaluatorHost {
    injected: Option<Arc<dyn Evaluator>>,
    live: Option<(Policy, String, blake3::Hash, Arc<dyn Evaluator>)>,
}

impl EvaluatorHost {
    pub fn injected(evaluator: Arc<dyn Evaluator>) -> Self {
        Self {
            injected: Some(evaluator),
            live: None,
        }
    }
    fn evaluator(
        &mut self,
        config: &crate::config::JevConfig,
    ) -> Result<Arc<dyn Evaluator>, &'static str> {
        if let Some(evaluator) = &self.injected {
            return Ok(evaluator.clone());
        }
        let key = std::env::var(&config.api_key_env)
            .ok()
            .filter(|key| !key.trim().is_empty())
            .ok_or("missing_credentials")?;
        let policy = config.policy();
        let identity = blake3::hash(key.as_bytes());
        if let Some((old_policy, old_env, old_identity, evaluator)) = &self.live {
            if old_policy == &policy && old_env == &config.api_key_env && old_identity == &identity
            {
                return Ok(evaluator.clone());
            }
        }
        let transport =
            HttpsTransport::new(&key, &policy).map_err(|_| "invalid_transport_configuration")?;
        let evaluator: Arc<dyn Evaluator> = Arc::new(
            Runtime::new(Arc::new(transport), policy.clone())
                .map_err(|_| "invalid_transport_configuration")?,
        );
        self.live = Some((
            policy,
            config.api_key_env.clone(),
            identity,
            evaluator.clone(),
        ));
        Ok(evaluator)
    }

    pub async fn overview(
        &mut self,
        prepared: crate::tools::overview::jev::PreparedOverview,
        arguments: &Value,
        options: EvaluationOptions,
    ) -> Result<Value, (i64, String)> {
        let config = crate::config::get();
        let settings = &config.jev;
        let task_query = crate::tools::task_query(arguments)?;
        if !settings.is_overview_enabled {
            return Ok(envelope(prepared.base_text, None));
        }
        let mut report = Report::new(
            "overview",
            crate::tools::overview::jev::POLICY_VERSION,
            None,
        );
        let cap = config
            .overview_output_byte_cap
            .unwrap_or_else(|| prepared.base_text.len().saturating_add(65_536));
        let mut text = prepared.base_text;
        let reason = if task_query.is_none() {
            Some("missing_task_query")
        } else {
            prepared.bypass_reason
        };
        if let Some(reason) = reason {
            report.reason = reason.into();
            return Ok(envelope_with_report(text, report, cap));
        }
        let Some(snapshot) = prepared.snapshot else {
            report.reason = "unavailable_snapshot".into();
            return Ok(envelope_with_report(text, report, cap));
        };
        if cap.saturating_sub(text.len()) < 512 {
            report.reason = "insufficient_output_room".into();
            return Ok(envelope_with_report(text, report, cap));
        }
        let evaluator = match self.evaluator(settings) {
            Ok(evaluator) => evaluator,
            Err(reason) => {
                report.reason = reason.into();
                return Ok(envelope_with_report(text, report, cap));
            }
        };
        match crate::tools::overview::jev::evaluate(
            snapshot,
            task_query.unwrap(),
            evaluator.as_ref(),
            options,
        )
        .await
        {
            Ok(result) => {
                report.metrics = result.metrics.clone();
                report.recommendation_status = Some(result.status.as_str());
                report.evaluated_count = result.candidates.len();
                match crate::tools::overview::jev::render(
                    &result,
                    cap.saturating_sub(text.len() + 512),
                ) {
                    Ok(addition) => {
                        text.push_str(&addition);
                        report.outcome = "applied";
                        report.reason = "complete_evaluation".into();
                        report.selected_count = result.recommendations.len();
                    }
                    Err(reason) => {
                        report.outcome = "fallback";
                        report.reason = reason.into();
                    }
                }
            }
            Err(failure) => {
                report.outcome = "fallback";
                report.reason = failure.kind.to_string();
                report.metrics = failure.metrics;
            }
        }
        Ok(envelope_with_report(text, report, cap))
    }

    pub async fn search(
        &mut self,
        mut output: crate::tools::search::SearchOutput,
        arguments: &Value,
        options: EvaluationOptions,
    ) -> Result<(Value, Vec<crate::analyze::FileObservation>), (i64, String)> {
        let config = crate::config::get();
        let settings = &config.jev;
        let task_query = crate::tools::task_query(arguments)?;
        if !settings.is_search_filter_enabled {
            return Ok((envelope(output.text, None), output.source_files));
        }
        let cap = config.search_detail_byte_cap;
        let threshold = settings.search_filter_min_unrelated_probability;
        let mut report = Report::new(
            "search_filter",
            crate::tools::search::jev::POLICY_VERSION,
            Some(threshold),
        );
        let reason = if task_query.is_none() {
            Some("missing_task_query")
        } else if output
            .prepared
            .as_ref()
            .is_none_or(|prepared| prepared.complete_body_count() == 0)
        {
            Some("no_complete_selected_bodies")
        } else {
            None
        };
        if let Some(reason) = reason {
            report.reason = reason.into();
            return Ok((
                envelope_with_report(output.text, report, cap),
                output.source_files,
            ));
        }
        let evaluator = match self.evaluator(settings) {
            Ok(evaluator) => evaluator,
            Err(reason) => {
                report.reason = reason.into();
                return Ok((
                    envelope_with_report(output.text, report, cap),
                    output.source_files,
                ));
            }
        };
        let prepared = output.prepared.as_ref().unwrap();
        let mut search_arguments = arguments.clone();
        if let Some(args) = search_arguments.as_object_mut() {
            args.remove("task_query");
        }
        match crate::tools::search::jev::evaluate(
            prepared,
            task_query.unwrap(),
            search_arguments,
            threshold,
            evaluator.as_ref(),
            options,
        )
        .await
        {
            Ok(result) => {
                report.metrics = result.metrics;
                report.evaluated_count = prepared.complete_body_count();
                report.selected_count = result.retention.keep.iter().filter(|keep| !**keep).count();
                match crate::tools::search::jev::render(&output, &result.retention, cap) {
                    Ok(rendered) => {
                        output.text = rendered.text;
                        output.source_files = rendered.source_files;
                        report.outcome = "applied";
                        report.reason = "complete_evaluation".into();
                    }
                    Err(reason) => {
                        report.outcome = "fallback";
                        report.reason = reason.into();
                        report.selected_count = 0;
                    }
                }
            }
            Err(failure) => {
                report.outcome = "fallback";
                report.reason = failure.kind.to_string();
                report.metrics = failure.metrics;
            }
        }
        Ok((
            envelope_with_report(output.text, report, cap),
            output.source_files,
        ))
    }
}

struct Report {
    mode: &'static str,
    outcome: &'static str,
    reason: String,
    policy: &'static str,
    threshold: Option<f64>,
    recommendation_status: Option<&'static str>,
    metrics: Metrics,
    evaluated_count: usize,
    selected_count: usize,
}
impl Report {
    fn new(mode: &'static str, policy: &'static str, threshold: Option<f64>) -> Self {
        Self {
            mode,
            outcome: "bypassed",
            reason: String::new(),
            policy,
            threshold,
            recommendation_status: None,
            metrics: Metrics::default(),
            evaluated_count: 0,
            selected_count: 0,
        }
    }
    fn value(&self) -> Value {
        json!({"mode":self.mode,"outcome":self.outcome,"reason":self.reason,"model":crate::jev::MODEL,"policy_version":self.policy,
        "search_filter_min_unrelated_probability":self.threshold,"recommendation_status":self.recommendation_status,"metrics":self.metrics,
        "usage_scope":"reported_responses","evaluated_count":self.evaluated_count,"selected_count":self.selected_count})
    }
}

fn envelope(text: String, report: Option<Value>) -> Value {
    let mut value = json!({"content":[{"type":"text","text":text}]});
    if let Some(report) = report {
        value["_meta"] = json!({"jev":report});
    }
    value
}

fn envelope_with_report(mut text: String, report: Report, cap: usize) -> Value {
    let note=format!("\n_Jev {}: {}; reason={}; input_tokens={}; output_tokens={}; elapsed_ms={}; http_elapsed_ms={}; policy={}{}._\n",
        report.mode,report.outcome,report.reason,report.metrics.usage.input_tokens,report.metrics.usage.output_tokens,report.metrics.elapsed_ms,report.metrics.http_elapsed_ms,report.policy,
        report.threshold.map(|threshold|format!("; min_unrelated_probability={threshold:.2}")).unwrap_or_default());
    // _meta remains inspectable when the content cap leaves no room for a mirrored note.
    if text.len().saturating_add(note.len()) <= cap {
        text.push_str(&note);
    }
    envelope(text, Some(report.value()))
}
