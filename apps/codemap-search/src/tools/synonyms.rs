use crate::related_terms::evidence::EvidenceView;
use crate::related_terms::runtime::{LookupStatus, RelatedTermsRuntime};
#[cfg(feature = "related-terms-eval")]
use crate::related_terms::LookupResult;
use serde_json::{json, Value};

mod presentation;

pub(crate) const MAX_TERMS: usize = 16;

pub(crate) fn definition() -> Value {
    json!({
        "name": "synonyms",
        "description": include_str!("instructions/tools/synonyms.md").trim_end(),
        "annotations": {"readOnlyHint": true, "openWorldHint": false},
        "inputSchema": {
            "type": "object", "additionalProperties": false,
            "properties": {"terms": {"type": "array", "minItems": 1, "maxItems": MAX_TERMS,
                "items": {"type": "string", "minLength": 1},
                "description": "Preselect lowercase target words, operation compounds or one-concept technical terms (`abort signal`). Keep inflections and useful find/find_by separately; add no role, alias or instruction words."}},
            "required": ["terms"]
        }
    })
}

fn parse_terms(arguments: &Value) -> Result<Vec<String>, (i64, String)> {
    let invalid = |message: String| (-32602, message);
    let object = arguments
        .as_object()
        .ok_or_else(|| invalid("synonyms arguments must be an object".into()))?;
    if object.keys().any(|key| key != "terms") {
        return Err(invalid("synonyms accepts only terms".into()));
    }
    let terms = object
        .get("terms")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("synonyms requires a terms array".into()))?;
    if terms.is_empty() || terms.len() > MAX_TERMS {
        return Err(invalid(format!(
            "synonyms requires 1 to {MAX_TERMS} terms per call"
        )));
    }
    terms
        .iter()
        .map(|term| {
            term.as_str()
                .filter(|term| !term.trim().is_empty())
                .map(str::to_owned)
                .ok_or_else(|| invalid("every synonyms term must be a non-empty string".into()))
        })
        .collect()
}

pub(crate) struct SynonymsOutput {
    pub text: String,
    #[cfg(feature = "related-terms-eval")]
    pub evaluation: Value,
}

pub(crate) fn run(
    arguments: &Value,
    runtime: &mut RelatedTermsRuntime,
) -> Result<SynonymsOutput, (i64, String)> {
    let terms = parse_terms(arguments)?;
    let mut output = SynonymsOutput {
        text: String::new(),
        #[cfg(feature = "related-terms-eval")]
        evaluation: Value::Null,
    };
    output.text = match runtime.lookup(&terms) {
        LookupStatus::Downloading => "Related terms: downloading (or waiting for another process). Retry shortly; other tools remain available.".into(),
        LookupStatus::Preparing => "Related terms: preparing local indexes. Retry shortly; other tools remain available.".into(),
        LookupStatus::Unavailable(reason) => format!("Related terms: unavailable: {reason}. Preparation is retried in the background on the next call after failure."),
        LookupStatus::Ready(mut answers) => {
            let evidence = runtime.evidence();
            let readiness = match &evidence {
                EvidenceView::Ready(_) => "Dictionary ready; codebase evidence ready.\n".to_string(),
                EvidenceView::Preparing => "Dictionary ready. Evidence preparing; unfiltered.\n".to_string(),
                EvidenceView::Unavailable(reason) => format!("Dictionary ready. Evidence unavailable: {reason} (unfiltered).\n"),
            };
            let mut text = String::new();
            #[cfg(feature = "related-terms-eval")]
            let mut measurements = Vec::new();
            for answer in &mut answers {
                #[cfg(feature = "related-terms-eval")]
                let before = json!({"fields": answer.senses.iter().map(|sense| &sense.field).collect::<Vec<_>>(),
                    "word_keys": candidate_keys(answer), "resolution": answer.resolution,
                    "senses": sense_memberships(answer)});
                let decision = match (&evidence, runtime.dictionary()) {
                    (EvidenceView::Ready(snapshot), Some(dictionary)) => Some(crate::related_terms::overlay::apply(
                        answer, snapshot, dictionary, runtime.is_overlay_enabled(), runtime.should_remove())),
                    _ => None,
                };
                let note = match (&evidence, &decision) {
                    (EvidenceView::Ready(snapshot), Some(decision)) => Some(decision.note(snapshot)),
                    _ => None,
                };
                // Word presence is display-only, so it also helps when senses stay unfiltered.
                let word_evidence = match &evidence {
                    EvidenceView::Ready(snapshot) if runtime.is_overlay_enabled() => Some(snapshot.as_ref()),
                    _ => None,
                };
                text.push_str(&presentation::render(answer,
                    decision.as_ref().map_or(0, |decision| decision.supported_sense_count),
                    word_evidence,
                    decision.as_ref().map_or(&[][..], |decision| &decision.sense_evidence),
                    &readiness, note.as_deref()));
                #[cfg(feature = "related-terms-eval")]
                measurements.push(json!({
                    "input": answer.input, "before": before,
                    "after": {"fields": answer.senses.iter().map(|sense| &sense.field).collect::<Vec<_>>(), "word_keys": candidate_keys(answer)},
                    "input_evidence": match &evidence {
                        EvidenceView::Ready(snapshot) => snapshot.words.get(&crate::related_terms::normalize::word_key(&answer.input)),
                        _ => None,
                    },
                    "decision": decision,
                }));
            }
            #[cfg(feature = "related-terms-eval")]
            {
                output.evaluation = json!({
                    "status": match &evidence { EvidenceView::Ready(_) => "ready", EvidenceView::Preparing => "preparing", EvidenceView::Unavailable(_) => "unavailable" },
                    "index_generation": runtime.index_generation(),
                    "evidence_generation": match &evidence { EvidenceView::Ready(snapshot) => Some(snapshot.generation), _ => None },
                    "scope": match &evidence { EvidenceView::Ready(snapshot) => Some(snapshot.scope), _ => None },
                    "is_overlay_enabled": runtime.is_overlay_enabled(),
                    "should_remove": runtime.should_remove(), "answers": measurements,
                });
            }
            text
        }
    };
    output.text = fit_output(output.text);
    Ok(output)
}

#[cfg(feature = "related-terms-eval")]
fn candidate_keys(answer: &LookupResult) -> std::collections::BTreeSet<String> {
    answer
        .senses
        .iter()
        .flat_map(|sense| {
            sense
                .direct
                .iter()
                .chain(&sense.near)
                .chain(&sense.same_topic)
        })
        .chain(
            answer
                .unsorted_direct
                .iter()
                .flat_map(|group| &group.direct),
        )
        .map(|word| crate::related_terms::normalize::word_key(word))
        .collect()
}

/// Full relation memberships per sense, so evaluations can audit evidence judgments.
#[cfg(feature = "related-terms-eval")]
fn sense_memberships(answer: &LookupResult) -> Vec<Value> {
    let keys = |words: &[String]| {
        words
            .iter()
            .map(|word| crate::related_terms::normalize::word_key(word))
            .collect::<Vec<_>>()
    };
    answer
        .senses
        .iter()
        .map(|sense| {
            json!({
                "span": sense.span, "field": sense.field,
                "topics": sense.topics.iter().map(|topic| &topic.topic).collect::<Vec<_>>(),
                "direct": keys(&sense.full_groups.direct), "near": keys(&sense.full_groups.near),
                "same_topic": keys(&sense.full_groups.same_topic),
            })
        })
        .collect()
}

fn fit_output(mut text: String) -> String {
    let config = crate::config::get();
    let cap = config
        .output_byte_cap
        .unwrap_or(100_000)
        .min(config.client_output.delivery_byte_cap().unwrap_or(100_000));
    if text.len() <= cap {
        return text;
    }
    let note = "\n[Output truncated to the byte limit; request fewer terms.]";
    if cap < note.len() {
        return "truncated"[..cap.min("truncated".len())].to_string();
    }
    let mut end = cap - note.len();
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text.truncate(end);
    text.push_str(note);
    text
}
