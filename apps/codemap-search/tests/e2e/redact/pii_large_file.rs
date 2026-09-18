use super::{call, text};
use crate::e2e::helpers::{create_mock_repo, McpClient};
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Instant;

struct Fixture {
    path: &'static str,
    source: &'static str,
    manifest: &'static str,
    factory: &'static str,
    symbol: &'static str,
    overview: &'static str,
}

const TYPESCRIPT: Fixture = Fixture {
    path: "src/merchant_service.ts",
    source: include_str!("../../fixtures/redact_app/merchant_service.ts"),
    manifest: include_str!("../../fixtures/redact_app/merchant_service.pii.json"),
    factory: "export function buildAtlasMarketScenario",
    symbol: "buildAtlasMarketScenario",
    overview: "MerchantService",
};
const GO: Fixture = Fixture {
    path: "src/merchant_service.go",
    source: include_str!("../../fixtures/redact_app/merchant_service.go"),
    manifest: include_str!("../../fixtures/redact_app/merchant_service.go.pii.json"),
    factory: "func BuildAtlasMarketScenario",
    symbol: "BuildAtlasMarketScenario",
    overview: "RunScenario",
};
const RUST: Fixture = Fixture {
    path: "src/merchant_service.rs",
    source: include_str!("../../fixtures/redact_app/merchant_service.rs"),
    manifest: include_str!("../../fixtures/redact_app/merchant_service.rs.pii.json"),
    factory: "fn build_atlas_market_scenario",
    symbol: "build_atlas_market_scenario",
    overview: "MerchantService",
};
const JAVA: Fixture = Fixture {
    path: "src/MerchantService.java",
    source: include_str!("../../fixtures/redact_app/MerchantService.java"),
    manifest: include_str!("../../fixtures/redact_app/MerchantService.java.pii.json"),
    factory: "    static Scenario buildAtlasMarketScenario",
    symbol: "buildAtlasMarketScenario",
    overview: "MerchantService",
};
const HTML: Fixture = Fixture {
    path: "src/merchant_dashboard.html",
    source: include_str!("../../fixtures/redact_app/merchant_dashboard.html"),
    manifest: include_str!("../../fixtures/redact_app/merchant_dashboard.html.pii.json"),
    factory: "    <section class=\"tenant\" data-tenant=\"atlas-market\"",
    symbol: "section",
    overview: "tenant-filter",
};

#[derive(Deserialize)]
struct SensitiveValue {
    entity: String,
    field: String,
    line: usize,
    column_bytes: usize,
    value: String,
}

#[derive(Deserialize)]
struct FixtureManifest {
    line_count: usize,
    pii_occurrences: usize,
    entities: Vec<String>,
    spans: Vec<SensitiveValue>,
}

fn expected_lines(source: &str, manifest: &FixtureManifest) -> Vec<String> {
    let mut lines: Vec<_> = source.lines().map(str::to_owned).collect();
    assert_eq!(lines.len(), manifest.line_count);
    assert_eq!(manifest.line_count, 10_000);
    assert_eq!(manifest.pii_occurrences, 900);
    assert_eq!(manifest.spans.len(), 900);
    assert_eq!(manifest.entities.len(), 90);
    for span in &manifest.spans {
        let line = &mut lines[span.line - 1];
        let range = span.column_bytes..span.column_bytes + span.value.len();
        assert_eq!(
            line.get(range.clone()),
            Some(span.value.as_str()),
            "{} at line {}",
            span.entity,
            span.line
        );
        let mask = if span.value.len() >= "[REDACTED]".len() {
            "[REDACTED]".into()
        } else {
            "*".repeat(span.value.len())
        };
        line.replace_range(range, &mask);
    }
    assert_eq!(
        source
            .lines()
            .zip(&lines)
            .filter(|(before, after)| *before != *after)
            .count(),
        900
    );
    lines
}

fn assert_source_window(response: &Value, expected: &[String], first_line: usize, count: usize) {
    let rendered: Vec<_> = text(response)
        .lines()
        .filter_map(|line| line.split_once('→'))
        .collect();
    assert_eq!(rendered.len(), count, "missing source lines");
    for (index, (number, actual)) in rendered.iter().enumerate() {
        let line = first_line + index;
        assert_eq!(number.trim().parse::<usize>().unwrap(), line);
        assert_eq!(
            *actual,
            expected[line - 1],
            "unexpected masking at source line {line}"
        );
    }
}

#[tokio::test]
async fn test_all_ninety_pii_types_in_ten_thousand_lines() {
    check_fixture(&TYPESCRIPT).await;
}

#[tokio::test]
async fn test_go_ten_thousand_lines() {
    check_fixture(&GO).await;
}

#[tokio::test]
async fn test_rust_ten_thousand_lines() {
    check_fixture(&RUST).await;
}

#[tokio::test]
async fn test_java_ten_thousand_lines() {
    check_fixture(&JAVA).await;
}

#[tokio::test]
async fn test_html_ten_thousand_lines() {
    check_fixture(&HTML).await;
}

async fn check_fixture(fixture: &Fixture) {
    let source = fixture.source;
    let file_path = fixture.path;
    let manifest: FixtureManifest = serde_json::from_str(fixture.manifest).unwrap();
    let expected = expected_lines(source, &manifest);
    let config = format!(
        "[update]\nconfig_auto_update=false\n[redact]\npii_entities={}\n",
        serde_json::to_string(&manifest.entities).unwrap()
    );
    let repo = create_mock_repo(&[(file_path, source), (".codemap/config.toml", &config)]).unwrap();
    let mut client = McpClient::spawn(repo.path()).await.unwrap();
    let started = Instant::now();
    let response = call(
        &mut client,
        "read",
        json!({"file_path":file_path,"offset":1,"limit":manifest.line_count,"view":"source"}),
    )
    .await;
    eprintln!(
        "{file_path}: 10000 lines, 90 PII types, 900 values, {} bytes, full read={}ms",
        source.len(),
        started.elapsed().as_millis()
    );
    assert_source_window(&response, &expected, 1, manifest.line_count);

    // Windows begin directly on values, without the enclosing factory or request DTO.
    for span in manifest.spans.iter().rev().step_by(89).take(10) {
        let response = call(
            &mut client,
            "read",
            json!({"file_path":file_path,"offset":span.line,"limit":1,"view":"source"}),
        )
        .await;
        assert_source_window(&response, &expected, span.line, 1);
    }

    // Match each real source binding, including map keys and HTML form attributes.
    // The oracle is the fixture manifest; it does not use detector results.
    let source_lines: Vec<_> = source.lines().collect();
    let prefixes: std::collections::BTreeSet<_> = manifest
        .spans
        .iter()
        .map(|span| regex::escape(&source_lines[span.line - 1][..span.column_bytes]))
        .collect();
    let pattern = format!(
        r"^(?:{})",
        prefixes.into_iter().collect::<Vec<_>>().join("|")
    );
    let response = call(
        &mut client,
        "grep",
        json!({"path":file_path,"pattern":pattern,"output_mode":"count"}),
    )
    .await;
    assert!(
        text(&response).contains(&format!("{file_path}:900")),
        "{response}"
    );
    for expand in ["none", "callable"] {
        let started = Instant::now();
        let response = call(&mut client, "grep", json!({"path":file_path,"pattern":pattern,"head_limit":1000,"expand":expand,"view":"source"})).await;
        eprintln!(
            "{file_path}: grep {expand}={}ms",
            started.elapsed().as_millis()
        );
        let output = text(&response);
        for span in &manifest.spans {
            assert!(
                output.contains(&format!("{file_path}:{}:", span.line)),
                "missing {} ({}) at line {}",
                span.entity,
                span.field,
                span.line
            );
            assert!(
                output.contains(&format!(
                    "{file_path}:{}:{}",
                    span.line,
                    expected[span.line - 1]
                )),
                "wrong {} output at line {}",
                span.entity,
                span.line
            );
        }
    }

    let factory_line = source
        .lines()
        .position(|line| line.starts_with(fixture.factory))
        .unwrap()
        + 1;
    for view in ["full", "definitions", "relations"] {
        let response = call(
            &mut client,
            "read",
            json!({"file_path":file_path,"offset":factory_line,"limit":1,"view":view}),
        )
        .await;
        // HTML supports source/tag definitions but has no callable relationships.
        let expected_context = if fixture.path.ends_with(".html") && view == "relations" {
            "No declaration context"
        } else {
            fixture.symbol
        };
        assert!(text(&response).contains(expected_context), "{response}");
    }
    for entity in [
        "CREDIT_CARD",
        "EMAIL_ADDRESS",
        "IBAN_CODE",
        "KR_RRN",
        "US_HEALTH_INSURANCE_MEMBER_ID",
    ] {
        let span = manifest
            .spans
            .iter()
            .find(|span| span.entity == entity)
            .unwrap();
        let response = call(
            &mut client,
            "search",
            json!({"query":span.value,"caller_context":false}),
        )
        .await;
        assert!(text(&response).contains(file_path), "{entity}: {response}");
        assert!(
            !response.to_string().contains(&span.value),
            "indexed {entity} value leaked"
        );
        if fixture.path.ends_with(".html") {
            // HTML indexes format text, but its query extracts tags/attributes rather
            // than string literals. Search finds the file and renders definitions;
            // read/grep above verify actual value replacements byte for byte.
            assert!(
                text(&response).contains("match: path/docstring"),
                "{response}"
            );
        } else {
            assert!(
                text(&response).contains("[REDACTED]") || text(&response).contains("*****"),
                "{entity}: {response}"
            );
        }
    }
    let response = call(&mut client, "overview", json!({"path":file_path})).await;
    assert!(text(&response).contains(fixture.overview), "{response}");
    assert_eq!(
        std::fs::read_to_string(repo.path().join(file_path)).unwrap(),
        source
    );
}
