use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use serde_json::{json, Map, Value};
use tokio::time::Instant;

use super::*;
use crate::index::{PublishedIndexSnapshot, SearchResult};
use crate::jev::{
    BoxFuture, Evaluator, JevTransport, TransportError, TransportPolicy, TransportResponse,
    DEFAULT_MAX_BATCH_BYTES, DEFAULT_MODEL,
};
use crate::parser::{CodeExtractor, CodeRange, SymbolFlags, TreeSitterExtractor};
use crate::tools::search::{render_ranked_results, SearchArguments};

const UPLOAD_RS: &str = r#"pub const MAX_RETRIES: u32 = 3;

pub struct RetryPolicy {
    pub attempts: u32,
}

impl RetryPolicy {
    pub fn new(attempts: u32) -> Self {
        Self { attempts }
    }

    pub fn allows(&self, attempt: u32) -> bool {
        attempt < self.attempts
    }
}

pub fn retry_upload(policy: &RetryPolicy, attempt: u32) -> bool {
    if policy.allows(attempt) {
        return send_chunk(attempt);
    }
    false
}

fn send_chunk(attempt: u32) -> bool {
    attempt % 2 == 0
}

pub fn format_size(bytes: u64) -> String {
    format!("{bytes} B")
}

pub fn render_banner() -> String {
    fn banner_line() -> String {
        String::from("upload tool")
    }
    let line = banner_line();
    line.to_uppercase()
}
"#;

const SESSION_TS: &str = r#"export class UploadSession {
  private attempts = 0;

  constructor(private readonly limit: number) {}

  retry(): boolean {
    this.attempts += 1;
    return this.canRetry();
  }

  canRetry(): boolean {
    return this.attempts < this.limit;
  }

  describe(): string {
    return `session with ${this.limit} attempts`;
  }
}

export class LocalStore {
  run(): void {
    console.log("local");
  }
}

export class RemoteStore {
  run(): void {
    console.log("remote");
  }
}

export function syncStores(store: LocalStore | RemoteStore): void {
  store.run();
}

export function formatDuration(ms: number): string {
  return `${ms} ms`;
}
"#;

const SETTINGS_JSON: &str = r#"{
  "retry": {
    "attempts": 3
  }
}
"#;

const UPLOAD_SYMBOLS: &[&str] = &[
    "MAX_RETRIES",
    "RetryPolicy",
    "new",
    "allows",
    "retry_upload",
    "send_chunk",
    "format_size",
    "render_banner",
    "banner_line",
];
const SESSION_SYMBOLS: &[&str] = &[
    "UploadSession",
    "retry",
    "canRetry",
    "describe",
    "LocalStore",
    "RemoteStore",
    "run",
    "syncStores",
    "formatDuration",
];
const MIXED_QUERY: &str = "retry_upload send_chunk allows new format_size banner_line MAX_RETRIES RetryPolicy retry canRetry describe run syncStores formatDuration";

/// One prepared search over real parser output and the real renderer, backed by files in
/// a temporary directory.
struct Fixture {
    _directory: tempfile::TempDir,
    paths: Vec<String>,
    sources: HashMap<String, String>,
    prepared: PreparedSearch,
}

impl Fixture {
    fn path(&self, file_name: &str) -> String {
        self.paths
            .iter()
            .find(|path| path.ends_with(&format!("/{file_name}")))
            .unwrap()
            .clone()
    }

    fn declaration(&self, evidence: &SearchEvidence, file_name: &str, name: &str) -> usize {
        let path = self.path(file_name);
        evidence
            .declarations
            .iter()
            .position(|declaration| {
                declaration.path == path && declaration.qualified_name() == name
            })
            .unwrap_or_else(|| panic!("no displayed declaration {name} in {file_name}"))
    }
}

/// `files` are `(file name, source, displayed symbol names in rank order)`; results are
/// ranked in the given order.
fn prepared_fixture(files: &[(&str, &str, &[&str])], query: &str) -> Fixture {
    let directory = tempfile::tempdir().unwrap();
    let mut paths = Vec::new();
    let mut sources = HashMap::new();
    let mut extracted = Vec::new();
    let mut results = Vec::new();
    for (rank, (file_name, source, names)) in files.iter().enumerate() {
        let path = directory
            .path()
            .join(file_name)
            .to_string_lossy()
            .into_owned();
        std::fs::write(&path, source).unwrap();
        let file = TreeSitterExtractor::new().extract(source, &path).unwrap();
        let matched_symbols = names
            .iter()
            .flat_map(|name| {
                file.symbols
                    .iter()
                    .filter(move |symbol| symbol.name == *name)
            })
            .cloned()
            .collect();
        results.push(SearchResult {
            file_path: path.clone(),
            score: (files.len() - rank) as f32,
            total_lines: file.total_lines,
            matched_symbols,
            matched_literals: Vec::new(),
            symbol_fallback: false,
            ranking_signal: None,
            qualified_literal_hit: None,
        });
        extracted.push((file, Vec::new()));
        sources.insert(path.clone(), source.to_string());
        paths.push(path);
    }
    let snapshot = PublishedIndexSnapshot::from_files_and_edges(extracted);
    let rendered = render_ranked_results(String::new(), query, &results, &snapshot, None);
    let prepared = PreparedSearch {
        shape: SearchShape::RankedResults,
        arguments: SearchArguments {
            query: query.to_string(),
            ..SearchArguments::default()
        },
        text: rendered.text,
        relation_insertions: Vec::new(),
        files: rendered.files,
        retained_primary_bytes: rendered.retained_primary_bytes,
        byte_cap: crate::config::get().search_detail_byte_cap,
        is_index_warming: false,
    };
    Fixture {
        _directory: directory,
        paths,
        sources,
        prepared,
    }
}

fn mixed_fixture() -> Fixture {
    prepared_fixture(
        &[
            ("upload.rs", UPLOAD_RS, UPLOAD_SYMBOLS),
            ("session.ts", SESSION_TS, SESSION_SYMBOLS),
            ("settings.json", SETTINGS_JSON, &["retry"]),
        ],
        MIXED_QUERY,
    )
}

/// Answers every Noul question by the declaration's qualified name; unknown names get the
/// default. Overrides make the provider fail or answer one question invalidly.
struct NoulTransport {
    by_name: HashMap<String, f64>,
    default: f64,
    failing_status: Option<u16>,
    raw_answer: Option<(String, Value)>,
    delay: Duration,
    calls: AtomicUsize,
    bodies: Mutex<Vec<Value>>,
}

impl NoulTransport {
    fn new(default: f64) -> Self {
        Self {
            by_name: HashMap::new(),
            default,
            failing_status: None,
            raw_answer: None,
            delay: Duration::ZERO,
            calls: AtomicUsize::new(0),
            bodies: Mutex::new(Vec::new()),
        }
    }

    fn with(mut self, name: &str, noul: f64) -> Self {
        self.by_name.insert(name.to_string(), noul);
        self
    }

    fn failing(mut self, status: u16) -> Self {
        self.failing_status = Some(status);
        self
    }

    fn answering(mut self, name: &str, answer: Value) -> Self {
        self.raw_answer = Some((name.to_string(), answer));
        self
    }

    fn with_delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }

    fn call_count(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }

    fn bodies(&self) -> Vec<Value> {
        self.bodies.lock().unwrap().clone()
    }
}

impl JevTransport for NoulTransport {
    fn post(&self, body: Vec<u8>) -> BoxFuture<'_, Result<TransportResponse, TransportError>> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            tokio::time::sleep(self.delay).await;
            let request: Value = serde_json::from_slice(&body).unwrap();
            self.bodies.lock().unwrap().push(request.clone());
            if let Some(status) = self.failing_status {
                return Ok(TransportResponse {
                    status,
                    body: Vec::new(),
                });
            }
            let answers: Map<String, Value> = request["questions"]
                .as_object()
                .unwrap()
                .iter()
                .map(|(id, question)| {
                    let name = question["instructions"]["declaration"]["name"].as_str().unwrap();
                    let answer = match &self.raw_answer {
                        Some((raw_name, answer)) if raw_name == name => answer.clone(),
                        _ => json!({"type": "noul",
                                    "noul": self.by_name.get(name).copied().unwrap_or(self.default)}),
                    };
                    (id.clone(), answer)
                })
                .collect();
            let response = json!({"model": DEFAULT_MODEL, "answers": answers,
                                  "usage": {"input_tokens": 900, "output_tokens": 12}});
            Ok(TransportResponse {
                status: 200,
                body: serde_json::to_vec(&response).unwrap(),
            })
        })
    }
}

const TASK_QUERY: &str = "Where does a failed upload get retried, and what limits the retries?";

fn evaluator(transport: NoulTransport) -> Evaluator<NoulTransport> {
    let policy = TransportPolicy {
        request_spacing: Duration::ZERO,
        ..TransportPolicy::default()
    };
    Evaluator::new(transport, policy).unwrap()
}

fn options(min_unrelated_probability: f64) -> FilterOptions {
    FilterOptions {
        task_query: TASK_QUERY.to_string(),
        min_unrelated_probability,
        deadline: Instant::now() + Duration::from_secs(45),
        cancellation: None,
        max_batch_bytes: DEFAULT_MAX_BATCH_BYTES,
    }
}

async fn run_filter(
    prepared: &PreparedSearch,
    transport: NoulTransport,
    options: FilterOptions,
) -> (FilterOutcome, Evaluator<NoulTransport>) {
    let evaluator = evaluator(transport);
    let outcome = filter(prepared, &evaluator, options).await;
    (outcome, evaluator)
}

fn applied(outcome: FilterOutcome) -> SearchFilter {
    match outcome {
        FilterOutcome::Applied(filter) => *filter,
        other => panic!("expected an applied filter, got {other:?}"),
    }
}

fn fallback(outcome: FilterOutcome) -> FilterFallback {
    match outcome {
        FilterOutcome::Fallback(fallback) => fallback,
        other => panic!("expected a fallback, got {other:?}"),
    }
}

fn observations(output: &SearchOutput) -> Vec<(String, u64)> {
    output
        .source_files
        .iter()
        .map(|observation| (observation.path.clone(), observation.result_bytes))
        .collect()
}

fn symbol_rows(text: &str) -> Vec<&str> {
    text.lines()
        .filter(|line| line.trim_start().starts_with("- Symbol: "))
        .collect()
}

/// The text under the `## N. path` heading of one detail file.
fn section_of<'a>(text: &'a str, paths: &[String], path: &str) -> &'a str {
    let number = paths
        .iter()
        .position(|candidate| candidate == path)
        .unwrap()
        + 1;
    let start = text.find(&format!("\n## {number}. {path}\n")).unwrap() + 1;
    let end = text[start..]
        .find(&format!("\n## {}. ", number + 1))
        .or_else(|| text[start..].find("\n## Other matches"))
        .map_or(text.len(), |offset| start + offset);
    &text[start..end]
}

/// `(line number, content)` of every `{number}→{content}` line.
fn numbered_lines(section: &str) -> impl Iterator<Item = (usize, &str)> {
    section.lines().filter_map(|line| {
        let (number, content) = line.split_once('→')?;
        Some((number.trim().parse().ok()?, content))
    })
}

fn synthetic(
    name: &str,
    kind: &str,
    completeness: Completeness,
    lines: (usize, usize),
) -> DisplayedDeclaration {
    let symbol = ExtractedSymbol {
        name: name.to_string(),
        kind: kind.to_string(),
        range: CodeRange {
            start_line: lines.0,
            start_col: 1,
            end_line: lines.1,
            end_col: 2,
        },
        docstring: None,
        flags: SymbolFlags {
            has_todo: false,
            has_fixme: false,
            is_test: false,
            is_exported: true,
            is_deprecated: false,
        },
        owner: None,
    };
    DisplayedDeclaration {
        id: String::new(),
        file_index: 0,
        path: "src/lib.rs".to_string(),
        name: name.to_string(),
        kind: kind.to_string(),
        owner: None,
        start_line: lines.0,
        end_line: lines.1,
        kind_class: KindClass::of(&symbol),
        completeness,
        body_block: None,
        displayed_lines: (completeness != Completeness::Missing).then_some(lines),
        body_source: None,
    }
}

fn evidence_of(declarations: Vec<DisplayedDeclaration>) -> SearchEvidence {
    SearchEvidence {
        declarations: declarations
            .into_iter()
            .enumerate()
            .map(|(index, mut declaration)| {
                declaration.id = format!("d{index:04}");
                declaration
            })
            .collect(),
        ..SearchEvidence::default()
    }
}

fn judged(pairs: &[(&str, f64)]) -> BTreeMap<String, f64> {
    pairs
        .iter()
        .map(|(id, noul)| (id.to_string(), *noul))
        .collect()
}

use Completeness::{Complete, Missing, Oversized, Partial};
use Retention::{Omitted, Retained};
use RetentionReason::*;

#[test]
fn threshold_boundaries_keep_uncertain_bodies_and_omit_at_or_above() {
    let evidence = evidence_of(vec![synthetic("handler", "fn", Complete, (1, 5))]);
    for (noul, expected) in [
        (0.00, Retained(BelowThreshold)),
        (0.50, Retained(BelowThreshold)),
        (0.69, Retained(BelowThreshold)),
        (0.70, Omitted),
        (0.71, Omitted),
        (1.00, Omitted),
    ] {
        let decision = apply_policy(&evidence, &judged(&[("d0000", noul)]), 0.70).unwrap();
        assert_eq!(decision.retention, vec![expected], "noul {noul}");
        assert_eq!(decision.policy_version, POLICY_VERSION);
        assert_eq!(decision.min_unrelated_probability, 0.70);
    }
}

#[test]
fn thresholds_outside_the_half_open_range_are_rejected() {
    let evidence = evidence_of(vec![synthetic("handler", "fn", Complete, (1, 5))]);
    for threshold in [
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        -0.7,
        0.0,
        0.5,
        1.000_001,
        2.0,
    ] {
        assert!(
            validate_min_unrelated_probability(threshold).is_err(),
            "{threshold}"
        );
        assert!(
            matches!(
                apply_policy(&evidence, &BTreeMap::new(), threshold),
                Err(JevError::InvalidPolicy { .. })
            ),
            "{threshold}"
        );
    }
    for threshold in [0.500_001, DEFAULT_MIN_UNRELATED_PROBABILITY, 1.0] {
        assert!(
            apply_policy(&evidence, &BTreeMap::new(), threshold).is_ok(),
            "{threshold}"
        );
    }
}

#[test]
fn invalid_or_missing_judgments_keep_the_body() {
    let evidence = evidence_of(vec![
        synthetic("not_a_number", "fn", Complete, (1, 3)),
        synthetic("negative", "fn", Complete, (5, 7)),
        synthetic("above_one", "fn", Complete, (9, 11)),
        synthetic("infinite", "fn", Complete, (13, 15)),
        synthetic("unanswered", "fn", Complete, (17, 19)),
        synthetic("window", "fn", Partial, (21, 40)),
    ]);
    let judgments = judged(&[
        ("d0000", f64::NAN),
        ("d0001", -0.1),
        ("d0002", 1.2),
        ("d0003", f64::INFINITY),
    ]);
    let decision = apply_policy(&evidence, &judgments, 0.70).unwrap();
    assert_eq!(decision.retention[..5], [Retained(UnavailableJudgment); 5]);
    assert_eq!(decision.retention[5], Retained(PartialBody));

    for foreign in ["d0099", "d0005"] {
        let error = apply_policy(&evidence, &judged(&[(foreign, 1.0)]), 0.70).unwrap_err();
        assert!(
            matches!(&error, JevError::InvalidAnswer { question_id, .. } if question_id == foreign),
            "{error:?}"
        );
    }
}

#[test]
fn incomplete_oversized_and_unknown_evidence_survives_certain_answers() {
    let evidence = evidence_of(vec![
        synthetic("window", "fn", Partial, (1, 40)),
        synthetic("row_only", "fn", Missing, (42, 60)),
        synthetic("huge", "fn", Oversized, (62, 600)),
        synthetic("section", "key", Complete, (602, 610)),
        synthetic("plain", "fn", Complete, (612, 620)),
    ]);
    let decision =
        apply_policy(&evidence, &judged(&[("d0003", 1.0), ("d0004", 1.0)]), 0.70).unwrap();
    assert_eq!(
        decision.retention,
        vec![
            Retained(PartialBody),
            Retained(MissingBody),
            Retained(OversizedBody),
            Retained(UnknownKind),
            Omitted,
        ]
    );
}

#[test]
fn retention_closes_over_links_nesting_and_ambiguity() {
    let mut evidence = evidence_of(vec![
        synthetic("entry", "fn", Partial, (1, 40)),           // 0
        synthetic("middle", "fn", Complete, (42, 50)),        // 1
        synthetic("leaf", "fn", Complete, (52, 60)),          // 2
        synthetic("isolated", "fn", Complete, (62, 70)),      // 3
        synthetic("caller", "fn", Complete, (72, 80)),        // 4
        synthetic("related", "fn", Complete, (82, 90)),       // 5
        synthetic("outer", "fn", Partial, (100, 150)),        // 6
        synthetic("inner", "fn", Complete, (110, 120)),       // 7
        synthetic("Holder", "class", Partial, (200, 260)),    // 8
        synthetic("member", "fn", Complete, (210, 220)),      // 9
        synthetic("dispatch", "fn", Complete, (270, 280)),    // 10
        synthetic("run", "fn", Complete, (282, 285)),         // 11
        synthetic("run", "fn", Complete, (287, 290)),         // 12
        synthetic("Record", "struct", Complete, (300, 320)),  // 13
        synthetic("record_hook", "fn", Complete, (305, 310)), // 14
    ]);
    evidence.links = BTreeSet::from([(0, 1), (1, 2), (4, 5)]);
    evidence.ambiguous_participants = BTreeSet::from([10, 11, 12]);
    let mut judgments: BTreeMap<String, f64> = evidence
        .question_ids()
        .into_iter()
        .map(|id| (id.to_string(), 1.0))
        .collect();
    judgments.insert("d0005".to_string(), 0.2);
    judgments.insert("d0014".to_string(), 0.1);

    let decision = apply_policy(&evidence, &judgments, 0.70).unwrap();
    assert_eq!(
        decision.retention,
        vec![
            Retained(PartialBody),
            Retained(ConnectedToRetainedCallable),
            Retained(ConnectedToRetainedCallable),
            Omitted,
            Retained(ConnectedToRetainedCallable),
            Retained(BelowThreshold),
            Retained(PartialBody),
            Retained(NestedInRetainedCallable),
            Retained(PartialBody),
            Omitted,
            Retained(AmbiguousLink),
            Retained(AmbiguousLink),
            Retained(AmbiguousLink),
            Retained(EnclosesRetainedDeclaration),
            Retained(BelowThreshold),
        ]
    );
    assert_eq!(decision.retention.len(), evidence.declarations.len());
}

#[tokio::test]
async fn certain_unrelated_answers_keep_protected_and_incomplete_bodies() {
    let fixture = mixed_fixture();
    let transport = NoulTransport::new(1.0)
        .with("retry_upload", 0.1)
        .with("UploadSession.retry", 0.1);
    let (outcome, evaluator) = run_filter(&fixture.prepared, transport, options(0.70)).await;
    let filter = applied(outcome);
    assert_eq!(evaluator.transport().call_count(), 1);

    let expected = [
        ("upload.rs", "MAX_RETRIES", Omitted),
        ("upload.rs", "RetryPolicy", Omitted),
        ("upload.rs", "RetryPolicy.new", Omitted),
        (
            "upload.rs",
            "RetryPolicy.allows",
            Retained(ConnectedToRetainedCallable),
        ),
        ("upload.rs", "retry_upload", Retained(BelowThreshold)),
        (
            "upload.rs",
            "send_chunk",
            Retained(ConnectedToRetainedCallable),
        ),
        ("upload.rs", "format_size", Omitted),
        ("upload.rs", "render_banner", Retained(PartialBody)),
        (
            "upload.rs",
            "banner_line",
            Retained(NestedInRetainedCallable),
        ),
        ("session.ts", "UploadSession", Retained(PartialBody)),
        (
            "session.ts",
            "UploadSession.retry",
            Retained(BelowThreshold),
        ),
        (
            "session.ts",
            "UploadSession.canRetry",
            Retained(ConnectedToRetainedCallable),
        ),
        ("session.ts", "UploadSession.describe", Omitted),
        ("session.ts", "LocalStore", Retained(PartialBody)),
        ("session.ts", "LocalStore.run", Retained(AmbiguousLink)),
        ("session.ts", "RemoteStore", Retained(PartialBody)),
        ("session.ts", "RemoteStore.run", Retained(AmbiguousLink)),
        ("session.ts", "syncStores", Retained(AmbiguousLink)),
        ("session.ts", "formatDuration", Omitted),
        ("settings.json", "retry", Retained(UnknownKind)),
    ];
    assert_eq!(filter.evidence.declarations.len(), expected.len());
    for (file_name, name, retention) in expected {
        let index = fixture.declaration(&filter.evidence, file_name, name);
        assert_eq!(
            filter.decision.retention[index], retention,
            "{file_name} {name}"
        );
    }
    let question_ids: BTreeSet<String> = filter
        .evidence
        .question_ids()
        .into_iter()
        .map(str::to_string)
        .collect();
    assert_eq!(
        filter.judgments.keys().cloned().collect::<BTreeSet<_>>(),
        question_ids
    );
    assert_eq!(question_ids.len(), 16);

    let base = fixture.prepared.render_without(&BTreeSet::new());
    let output = render(&fixture.prepared, &filter).expect("the note fits the output cap");
    for (number, path) in fixture.paths.iter().enumerate() {
        assert!(
            output
                .text
                .contains(&format!("\n\n## {}. {path}\n", number + 1)),
            "{path}"
        );
    }
    assert_eq!(symbol_rows(&output.text), symbol_rows(&base.text));
    for path in &fixture.paths {
        let section = section_of(&output.text, &fixture.paths, path);
        assert_eq!(
            section.lines().filter(|line| line.trim() == "```").count() % 2,
            0,
            "{path}"
        );
        let source: Vec<&str> = fixture.sources[path].lines().collect();
        for (line_number, content) in numbered_lines(section) {
            assert_eq!(source[line_number - 1], content, "{path}:{line_number}");
        }
    }
    for (declaration, retention) in filter
        .evidence
        .declarations
        .iter()
        .zip(&filter.decision.retention)
    {
        let section = section_of(&output.text, &fixture.paths, &declaration.path);
        let shown: BTreeSet<usize> = numbered_lines(section).map(|(number, _)| number).collect();
        let (first, last) = declaration.displayed_lines.unwrap();
        match retention {
            Retained(_) => assert!(
                (first..=last).all(|line| shown.contains(&line)),
                "{} lost displayed lines",
                declaration.qualified_name()
            ),
            Omitted => assert!(
                !shown.contains(&first),
                "{} still shown",
                declaration.qualified_name()
            ),
        }
    }
    let summary = output
        .text
        .lines()
        .find(|line| {
            line.starts_with("_Jev filter omitted 6 displayed bodies judged unrelated to the task")
        })
        .expect("omission summary");
    let upload = fixture.path("upload.rs");
    let session = fixture.path("session.ts");
    for entry in [
        format!("MAX_RETRIES `{upload}` L1-1"),
        format!("RetryPolicy `{upload}` L3-5"),
        format!("new `{upload}` L8-10"),
        format!("format_size `{upload}` L28-30"),
        format!("describe `{session}` L15-17"),
        format!("formatDuration `{session}` L36-38"),
    ] {
        assert!(summary.contains(&entry), "{entry}");
    }
    assert!(filter
        .note()
        .contains("applied · omitted=6/20 bodies · judged=16 · threshold=0.70"));
    assert!(output.text.ends_with(&filter.note()));

    let removed_bytes = |path: &str| -> u64 {
        filter
            .evidence
            .declarations
            .iter()
            .zip(&filter.decision.retention)
            .filter(|(declaration, retention)| declaration.path == path && **retention == Omitted)
            .map(|(declaration, _)| {
                let range = fixture
                    .prepared
                    .block_range(declaration.file_index, declaration.body_block.unwrap())
                    .unwrap();
                range.len() as u64
            })
            .sum()
    };
    let base_observations = observations(&base);
    let filtered_observations = observations(&output);
    assert_eq!(base_observations.len(), 3);
    assert_eq!(filtered_observations.len(), 3);
    for ((base_path, base_bytes), (path, bytes)) in
        base_observations.iter().zip(&filtered_observations)
    {
        assert_eq!(base_path, path);
        assert_eq!(*bytes, base_bytes - removed_bytes(path), "{path}");
    }
}

#[tokio::test]
async fn all_keep_answers_return_the_regular_output() {
    let fixture = mixed_fixture();
    let (outcome, _) = run_filter(&fixture.prepared, NoulTransport::new(0.0), options(0.70)).await;
    let filter = applied(outcome);
    assert_eq!(filter.decision.omitted_count(), 0);
    let output = render(&fixture.prepared, &filter).expect("the note fits the output cap");
    let unfiltered = fixture.prepared.render_without(&BTreeSet::new());
    assert_eq!(output.text, format!("{}{}", unfiltered.text, filter.note()));
    assert_eq!(observations(&output), observations(&unfiltered));

    let Fixture { mut prepared, .. } = fixture;
    prepared.byte_cap = unfiltered.text.len() + filter.note().len() - 1;
    assert!(
        render(&prepared, &filter).is_none(),
        "an unannounced result is never returned"
    );
    let regular = prepared.into_output();
    assert_eq!(regular.text, unfiltered.text);
    assert_eq!(observations(&regular), observations(&unfiltered));
    assert!(!regular.text.contains("[jev"));
}

#[test]
fn relation_hints_stay_with_their_sections_after_omission() {
    let mut fixture = mixed_fixture();
    let text = fixture.prepared.text.clone();
    fixture.prepared.relation_insertions = fixture
        .paths
        .iter()
        .enumerate()
        .map(|(index, path)| {
            let heading = text.find(&format!("\n## {}. {path}\n", index + 1)).unwrap();
            let results = heading + text[heading..].find("\n### results\n").unwrap();
            (results, format!("\n#### relation hint {}\n", index + 1))
        })
        .collect();
    let evidence = collect_evidence(&fixture.prepared);
    let every_complete_body: BTreeSet<(usize, usize)> = evidence
        .declarations
        .iter()
        .filter(|declaration| declaration.completeness == Complete)
        .map(|declaration| (declaration.file_index, declaration.body_block.unwrap()))
        .collect();
    let output = fixture.prepared.render_without(&every_complete_body);
    for (index, path) in fixture.paths.iter().enumerate() {
        let section = section_of(&output.text, &fixture.paths, path);
        assert!(
            section.contains(&format!(
                "\n#### relation hint {}\n\n### results\n",
                index + 1
            )),
            "{path}"
        );
    }
    let Fixture { prepared, .. } = fixture;
    let regular = prepared.into_output();
    for index in 1..=3 {
        assert!(regular
            .text
            .contains(&format!("\n#### relation hint {index}\n\n### results\n")));
    }
}

#[tokio::test]
async fn filtered_away_files_and_the_ranked_tail_are_not_observed() {
    let _config = crate::config::pin_test_config(crate::config::ResolvedConfig {
        result_threshold: 2,
        ..crate::config::ResolvedConfig::default()
    });
    let fixture = prepared_fixture(
        &[
            ("upload.rs", UPLOAD_RS, &["retry_upload", "send_chunk"]),
            (
                "format.rs",
                "pub fn format_size(bytes: u64) -> String {\n    format!(\"{bytes} B\")\n}\n",
                &["format_size"],
            ),
            ("later.rs", "pub fn retry_later() {}\n", &["retry_later"]),
        ],
        "retry_upload send_chunk format_size retry_later",
    );
    let (outcome, _) = run_filter(
        &fixture.prepared,
        NoulTransport::new(1.0).with("retry_upload", 0.1),
        options(0.70),
    )
    .await;
    let filter = applied(outcome);
    assert!(filter
        .evidence
        .declarations
        .iter()
        .all(|declaration| declaration.path != fixture.path("later.rs")));
    let base = fixture.prepared.render_without(&BTreeSet::new());
    let output = render(&fixture.prepared, &filter).expect("the note fits the output cap");
    let observed = |output: &SearchOutput| {
        observations(output)
            .into_iter()
            .map(|(path, _)| path)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        observed(&base),
        vec![fixture.path("upload.rs"), fixture.path("format.rs")]
    );
    assert_eq!(observed(&output), vec![fixture.path("upload.rs")]);
    for text in [&base.text, &output.text] {
        assert!(text.contains("## Other matches"));
        assert!(text.contains(&fixture.path("later.rs")));
    }
}

#[tokio::test]
async fn bodies_cut_by_the_output_cap_stay_partial() {
    let _config = crate::config::pin_test_config(crate::config::ResolvedConfig {
        search_detail_byte_cap: 1_600,
        ..crate::config::ResolvedConfig::default()
    });
    let fixture = mixed_fixture();
    assert!(fixture.prepared.retained_primary_bytes < fixture.prepared.text.len());
    let evidence = collect_evidence(&fixture.prepared);
    assert!(evidence
        .declarations
        .iter()
        .any(|declaration| declaration.completeness != Complete));
    for declaration in &evidence.declarations {
        if declaration.completeness == Complete {
            let range = fixture
                .prepared
                .block_range(declaration.file_index, declaration.body_block.unwrap())
                .unwrap();
            assert!(range.end <= fixture.prepared.retained_primary_bytes);
        }
    }

    let (outcome, _) = run_filter(&fixture.prepared, NoulTransport::new(1.0), options(0.70)).await;
    let filter = applied(outcome);
    let base = fixture.prepared.render_without(&BTreeSet::new());
    let output = render(&fixture.prepared, &filter).expect("the note fits the output cap");
    assert!(filter.decision.omitted_count() > 0);
    assert!(output.text.len() <= base.text.len() + filter.note().len());
    assert!(output.text.contains("_Partial search output"));
    assert_eq!(
        output
            .text
            .lines()
            .filter(|line| line.trim() == "```")
            .count()
            % 2,
        0
    );
    for path in &fixture.paths {
        if !output.text.contains(&format!(". {path}\n")) {
            continue;
        }
        let section = section_of(&output.text, &fixture.paths, path);
        let source: Vec<&str> = fixture.sources[path].lines().collect();
        for (line_number, content) in numbered_lines(section) {
            assert!(
                source[line_number - 1].starts_with(content),
                "{path}:{line_number}"
            );
        }
    }
}

#[tokio::test]
async fn provider_failures_and_invalid_answers_fall_back() {
    let fixture = mixed_fixture();

    let (outcome, _) = run_filter(
        &fixture.prepared,
        NoulTransport::new(1.0).failing(529),
        options(0.70),
    )
    .await;
    let failure = fallback(outcome);
    assert_eq!(failure.error.label(), "http_status");
    assert_eq!(failure.request_count, 1);
    assert!(failure
        .note()
        .contains("[jev search: fallback (http_status) · original output preserved"));

    for answer in [
        json!({"type": "noul", "noul": 1.5}),
        json!({"type": "noul", "noul": "high"}),
        json!({"type": "choice", "choice": "omit", "probabilities": {"omit": 1.0}, "confidence": 0.9}),
    ] {
        let transport = NoulTransport::new(1.0).answering("format_size", answer.clone());
        let (outcome, _) = run_filter(&fixture.prepared, transport, options(0.70)).await;
        let failure = fallback(outcome);
        assert_eq!(failure.error.label(), "invalid_answer", "{answer}");
        assert_eq!(failure.usage.input_tokens, 900);
        assert_eq!(failure.request_count, 1);
    }

    let (outcome, evaluator) =
        run_filter(&fixture.prepared, NoulTransport::new(1.0), options(0.5)).await;
    assert_eq!(fallback(outcome).error.label(), "invalid_policy");
    assert_eq!(evaluator.transport().call_count(), 0);
}

#[tokio::test(start_paused = true)]
async fn deadline_falls_back_without_a_partial_mask() {
    let fixture = mixed_fixture();
    let mut deadline_options = options(0.70);
    deadline_options.deadline = Instant::now() + Duration::from_secs(5);
    let transport = NoulTransport::new(1.0).with_delay(Duration::from_secs(60));
    let (outcome, _) = run_filter(&fixture.prepared, transport, deadline_options).await;
    let failure = fallback(outcome);
    assert_eq!(failure.error.label(), "deadline_exceeded");
    assert_eq!(failure.request_count, 0);
    assert_eq!(failure.usage, Usage::default());
}

#[tokio::test]
async fn event_lookups_empty_results_and_windows_only_bypass_without_requests() {
    for (shape, reason) in [
        (SearchShape::EventLookup, BypassReason::EventLookup),
        (SearchShape::NoResults, BypassReason::NoResults),
    ] {
        let prepared = PreparedSearch::passthrough(
            shape,
            SearchArguments::default(),
            "No indexed matches.".into(),
        );
        let (outcome, evaluator) =
            run_filter(&prepared, NoulTransport::new(1.0), options(0.70)).await;
        assert!(matches!(outcome, FilterOutcome::Bypassed(bypass) if bypass == reason));
        assert_eq!(evaluator.transport().call_count(), 0);
        assert_eq!(prepared.into_output().text, "No indexed matches.");
    }

    let mut warming = mixed_fixture();
    warming.prepared.is_index_warming = true;
    assert_eq!(
        bypass_reason(&warming.prepared),
        Some(BypassReason::IndexWarming)
    );
    let (outcome, evaluator) =
        run_filter(&warming.prepared, NoulTransport::new(1.0), options(0.70)).await;
    assert!(matches!(
        outcome,
        FilterOutcome::Bypassed(BypassReason::IndexWarming)
    ));
    assert_eq!(evaluator.transport().call_count(), 0);

    let _config = crate::config::pin_test_config(crate::config::ResolvedConfig {
        search_detail_snippet_max_lines: 2,
        ..crate::config::ResolvedConfig::default()
    });
    let fixture = prepared_fixture(
        &[("upload.rs", UPLOAD_RS, &["retry_upload"])],
        "retry_upload",
    );
    let (outcome, evaluator) =
        run_filter(&fixture.prepared, NoulTransport::new(1.0), options(0.70)).await;
    assert!(matches!(
        outcome,
        FilterOutcome::Bypassed(BypassReason::NoEligibleBodies)
    ));
    assert_eq!(evaluator.transport().call_count(), 0);
    assert!(BypassReason::NoEligibleBodies
        .note()
        .contains("bypassed (no_eligible_bodies)"));
}

#[tokio::test]
async fn questions_name_their_fields_and_carry_the_displayed_body() {
    let fixture = mixed_fixture();
    let (outcome, evaluator) =
        run_filter(&fixture.prepared, NoulTransport::new(0.0), options(0.70)).await;
    let filter = applied(outcome);
    let bodies = evaluator.transport().bodies();
    assert_eq!(bodies.len(), 1);
    let body = &bodies[0];
    assert_eq!(
        body["state"],
        json!({"task_query": TASK_QUERY, "search_arguments": {"query": MIXED_QUERY}})
    );
    let questions = body["questions"].as_object().unwrap();
    let question_ids: BTreeSet<&str> = questions.keys().map(String::as_str).collect();
    assert_eq!(question_ids, filter.evidence.question_ids());

    for (id, question) in questions {
        let declaration = filter
            .evidence
            .declarations
            .iter()
            .find(|declaration| &declaration.id == id)
            .unwrap();
        assert_eq!(question["type"], "noul");
        assert_eq!(question["criteria"]["true"], json!(WHEN_UNRELATED));
        assert_eq!(question["criteria"]["false"], json!(WHEN_RELATED));
        let instructions = &question["instructions"];
        assert_eq!(instructions["judgment"], "is_unrelated_to_task");
        assert_eq!(instructions["question"], QUESTION);
        assert_eq!(
            instructions["declaration"],
            json!({"file": declaration.path, "kind": declaration.kind, "name": declaration.qualified_name(),
                   "start_line": declaration.start_line, "end_line": declaration.end_line})
        );
        let source: Vec<&str> = fixture.sources[&declaration.path].lines().collect();
        let displayed: Vec<String> = (declaration.start_line..=declaration.end_line)
            .map(|line| format!("{line:>6}→{}", source[line - 1]))
            .collect();
        assert_eq!(
            instructions["displayed_body"],
            json!(displayed.join("\n")),
            "{id}"
        );
    }

    let upload = fixture.path("upload.rs");
    let question_of = |name: &str| {
        let index = fixture.declaration(&filter.evidence, "upload.rs", name);
        &questions[&filter.evidence.declarations[index].id]["instructions"]
    };
    assert_eq!(
        question_of("retry_upload")["displayed_callees"],
        json!([
            format!("{upload}:RetryPolicy.allows (fn) L12-14"),
            format!("{upload}:send_chunk (fn) L24-26")
        ])
    );
    assert_eq!(
        question_of("RetryPolicy.allows")["displayed_callers"],
        json!([format!("{upload}:retry_upload (fn) L17-22")])
    );
    assert_eq!(question_of("format_size")["displayed_callers"], json!([]));
}

#[tokio::test]
async fn displayed_link_context_is_bounded() {
    let mut source = String::from("pub fn hub() -> u32 {\n    1\n}\n");
    let mut names = vec!["hub".to_string()];
    for index in 0..10 {
        source.push_str(&format!(
            "\npub fn caller_{index:02}() -> u32 {{\n    hub()\n}}\n"
        ));
        names.push(format!("caller_{index:02}"));
    }
    let name_refs: Vec<&str> = names.iter().map(String::as_str).collect();
    let fixture = prepared_fixture(&[("hub.rs", &source, &name_refs)], &names.join(" "));
    let (outcome, evaluator) =
        run_filter(&fixture.prepared, NoulTransport::new(0.0), options(0.70)).await;
    let filter = applied(outcome);
    assert_eq!(filter.evidence.links.len(), 10);
    let body = &evaluator.transport().bodies()[0];
    let hub = &body["questions"]
        [&filter.evidence.declarations[fixture.declaration(&filter.evidence, "hub.rs", "hub")].id];
    assert_eq!(
        hub["instructions"]["displayed_callers"]
            .as_array()
            .unwrap()
            .len(),
        MAX_LINKED_DECLARATIONS
    );
}

#[tokio::test]
async fn bodies_too_large_for_one_question_are_kept_without_one() {
    let mut source = String::from("pub fn large_table() -> usize {\n");
    for index in 0..400 {
        source.push_str(&format!(
            "    let _entry_{index:03} = \"filler text that makes this generated body intentionally long\";\n"
        ));
    }
    source.push_str("    400\n}\n\npub fn small_helper() -> usize {\n    1\n}\n");
    let fixture = prepared_fixture(
        &[("table.rs", &source, &["large_table", "small_helper"])],
        "large_table small_helper",
    );
    let (outcome, _) = run_filter(&fixture.prepared, NoulTransport::new(1.0), options(0.70)).await;
    let filter = applied(outcome);
    let large = fixture.declaration(&filter.evidence, "table.rs", "large_table");
    let small = fixture.declaration(&filter.evidence, "table.rs", "small_helper");
    assert_eq!(filter.evidence.declarations[large].completeness, Oversized);
    assert_eq!(filter.decision.retention[large], Retained(OversizedBody));
    assert_eq!(filter.decision.retention[small], Omitted);
    assert_eq!(
        filter.judgments.keys().collect::<Vec<_>>(),
        vec![&filter.evidence.declarations[small].id]
    );

    let mut medium = String::from("pub fn medium_table() -> usize {\n");
    for index in 0..40 {
        medium.push_str(&format!(
            "    let _entry_{index:03} = \"filler text for a body near the batch ceiling\";\n"
        ));
    }
    medium.push_str("    40\n}\n");
    let fixture = prepared_fixture(&[("medium.rs", &medium, &["medium_table"])], "medium_table");
    let mut small_batches = options(0.70);
    small_batches.max_batch_bytes = crate::jev::MIN_BATCH_BYTES;
    let (outcome, evaluator) =
        run_filter(&fixture.prepared, NoulTransport::new(1.0), small_batches).await;
    assert!(matches!(
        outcome,
        FilterOutcome::Bypassed(BypassReason::NoEligibleBodies)
    ));
    assert_eq!(evaluator.transport().call_count(), 0);
}

#[tokio::test]
async fn a_recorded_judgment_replays_under_a_new_threshold_without_inference() {
    let fixture = prepared_fixture(&[("upload.rs", UPLOAD_RS, &["format_size"])], "format_size");
    let (outcome, evaluator) =
        run_filter(&fixture.prepared, NoulTransport::new(0.80), options(0.70)).await;
    let filter = applied(outcome);
    assert_eq!(filter.judgments, judged(&[("d0000", 0.80)]));
    assert_eq!(filter.decision.retention, vec![Omitted]);
    assert_eq!(filter.versions(), [QUESTION_VERSION, POLICY_VERSION]);

    let stricter = apply_policy(&filter.evidence, &filter.judgments, 0.90).unwrap();
    assert_eq!(stricter.retention, vec![Retained(BelowThreshold)]);
    assert_eq!(
        apply_policy(&filter.evidence, &filter.judgments, 0.70).unwrap(),
        filter.decision
    );
    assert_eq!(evaluator.transport().call_count(), 1);
}

#[test]
fn omission_summary_stays_within_its_room() {
    let omitted: Vec<DisplayedDeclaration> = (0..30)
        .map(|index| {
            synthetic(
                &format!("helper_{index:02}"),
                "fn",
                Complete,
                (index * 10 + 1, index * 10 + 5),
            )
        })
        .collect();
    let omitted: Vec<&DisplayedDeclaration> = omitted.iter().collect();
    let unbounded = omission_summary(&omitted, 0.70, usize::MAX);
    assert!(
        unbounded.contains("helper_23 `src/lib.rs` L231-235, +6 more. Use `read` to view them._\n")
    );
    assert!(!unbounded.contains("helper_24 "));

    let count_only = omission_summary(&omitted, 0.70, 0).len();
    assert_eq!(count_only, 0);
    let opening_and_closing =
        unbounded.find(": helper_00").unwrap() + ". Use `read` to view them._\n".len();
    let summary = omission_summary(&omitted, 0.70, opening_and_closing);
    assert!(summary.starts_with("\n\n_Jev filter omitted 30 displayed bodies"));
    assert!(!summary.contains("helper_00"));
    for room in [0, 64, 180, opening_and_closing + 40, 400, 900] {
        assert!(
            omission_summary(&omitted, 0.70, room).len() <= room,
            "{room}"
        );
    }
    assert!(omission_summary(&[], 0.70, usize::MAX).is_empty());
}
