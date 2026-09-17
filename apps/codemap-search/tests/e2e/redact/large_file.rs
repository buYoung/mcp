use super::{call, text};
use crate::e2e::helpers::{create_mock_repo, McpClient};
use serde_json::{json, Value};
use std::time::Instant;

const FILE_PATH: &str = "src/large_fixture.ts";
const LINE_COUNT: usize = 10_000;

struct SecretCase {
    line: usize,
    value: &'static str,
    source: &'static str,
}

fn secret_cases() -> [SecretCase; 10] {
    // Deliberately synthetic credentials. Each unique value occurs on one source line.
    [
        SecretCase {
            line: 3,
            value: "fixture-password-first-0123456789",
            source: "  const password = 'fixture-password-first-0123456789';",
        },
        SecretCase {
            line: 731,
            value: "fixture-context-token-02-0123456789",
            source: "  const options = { accessToken: 'fixture-context-token-02-0123456789', message: 'public-fixture-neighbor' };",
        },
        SecretCase {
            line: 1919,
            value: "ghp_FIXTUREONLY03abcdefghijklmnop0123456789",
            source: "  const githubExample = 'ghp_FIXTUREONLY03abcdefghijklmnop0123456789';",
        },
        SecretCase {
            line: 3077,
            value: "sk-proj-FIXTUREONLY04abcdefghijklmnop0123456789",
            source: "  const serviceExample = 'sk-proj-FIXTUREONLY04abcdefghijklmnop0123456789';",
        },
        SecretCase {
            line: 4096,
            value: "sk_test_FIXTUREONLY05abcdefghijklmnop0123456789",
            source: "  const billingExample = 'sk_test_FIXTUREONLY05abcdefghijklmnop0123456789';",
        },
        SecretCase {
            line: 5303,
            value: "glpat-FIXTUREONLY06abcdefghijklmnop0123456789",
            source: "  const gitlabExample = 'glpat-FIXTUREONLY06abcdefghijklmnop0123456789';",
        },
        SecretCase {
            line: 6781,
            value: "npm_FIXTUREONLY07abcdefghijklmnop0123456789",
            source: "  const packageExample = 'npm_FIXTUREONLY07abcdefghijklmnop0123456789';",
        },
        SecretCase {
            line: 7999,
            value: "fixture-url-password-08-0123456789",
            source: "  const databaseExample = 'postgres://reader:fixture-url-password-08-0123456789@localhost/example';",
        },
        SecretCase {
            line: 9123,
            value: "fixture-multiline-secret-09-0123456789",
            source: "  const clientSecret = `\nfixture-multiline-secret-09-0123456789\n`;",
        },
        SecretCase {
            line: 9995,
            value: "FIXTUREONLY10PEMabcdefghijklmnopqrstuvwxyz0123456789",
            source: "  const certificateExample = `-----BEGIN PRIVATE KEY-----\nFIXTUREONLY10PEMabcdefghijklmnopqrstuvwxyz0123456789\n-----END PRIVATE KEY-----`;",
        },
    ]
}

fn large_source(cases: &[SecretCase]) -> String {
    let mut lines: Vec<_> = (1..=LINE_COUNT)
        .map(|line| format!("  checksum += {line} % 7; // public-row-{line:05}"))
        .collect();
    lines[0] = "export function loadLargeFixture(externalToken: string) {".into();
    lines[1] = "  let checksum = 0;".into();
    lines[99] = "  const referencePassword: string = externalToken;".into();
    lines[LINE_COUNT - 2] = "  return checksum;".into();
    lines[LINE_COUNT - 1] = "}".into();
    for case in cases {
        for (offset, source_line) in case.source.lines().enumerate() {
            lines[case.line - 1 + offset] = source_line.into();
        }
    }
    lines.join("\n") + "\n"
}

fn assert_no_secrets(response: &Value, cases: &[SecretCase]) {
    let serialized = response.to_string();
    for (index, case) in cases.iter().enumerate() {
        let leaked_line = response["result"]["content"][0]["text"]
            .as_str()
            .into_iter()
            .flat_map(str::lines)
            .find(|line| line.contains(case.value))
            .map(|line| line.replace(case.value, "<secret>"));
        assert!(
            !serialized.contains(case.value),
            "secret case {} leaked in JSON-RPC output: {leaked_line:?}",
            index + 1,
        );
    }
}

#[tokio::test]
async fn test_redact_ten_secrets_in_ten_thousand_line_file() {
    let cases = secret_cases();
    let source = large_source(&cases);
    assert_eq!(source.lines().count(), LINE_COUNT);
    for case in &cases {
        assert_eq!(source.matches(case.value).count(), 1);
    }
    let repo = create_mock_repo(&[(FILE_PATH, &source)]).unwrap();
    assert_eq!(
        std::fs::read_to_string(repo.path().join(FILE_PATH)).unwrap(),
        source
    );
    eprintln!(
        "large redaction fixture: lines={LINE_COUNT}, bytes={}, secrets={}",
        source.len(),
        cases.len()
    );

    let mut client = McpClient::spawn(repo.path()).await.unwrap();
    let started = Instant::now();
    let ready = call(&mut client, "overview", json!({"path":FILE_PATH})).await;
    assert!(text(&ready).contains("loadLargeFixture"));
    eprintln!(
        "large redaction fixture: index warm-up={}ms",
        started.elapsed().as_millis()
    );

    let started = Instant::now();
    // Explicit range exercises the original file beyond the default unbounded-read ceiling.
    let response = call(
        &mut client,
        "read",
        json!({"file_path":FILE_PATH, "offset":1, "limit":LINE_COUNT, "view":"source"}),
    )
    .await;
    eprintln!(
        "large redaction fixture: full read={}ms",
        started.elapsed().as_millis()
    );
    assert_no_secrets(&response, &cases);
    let output = text(&response);
    assert_eq!(
        output.lines().filter(|line| line.contains('→')).count(),
        LINE_COUNT
    );
    assert!(output.contains("10000→}"));
    assert!(output.contains("referencePassword: string = externalToken"));
    assert!(output.contains("public-fixture-neighbor"));
    assert!(output.contains("postgres://reader:[REDACTED]@localhost/example"));
    assert!(output.contains("public-row-09998"));

    // Slice inside each secret, including multiline values whose opening is not returned.
    let started = Instant::now();
    for case in &cases {
        let line = source
            .lines()
            .position(|line| line.contains(case.value))
            .unwrap()
            + 1;
        let response = call(
            &mut client,
            "read",
            json!({"file_path":FILE_PATH, "offset":line, "limit":1, "view":"source"}),
        )
        .await;
        assert_no_secrets(&response, &cases);
        assert!(
            text(&response).contains("[REDACTED]"),
            "case at line {line}"
        );
        assert!(text(&response).contains(&format!("{line}→")));
    }
    eprintln!(
        "large redaction fixture: ten read windows={}ms",
        started.elapsed().as_millis()
    );

    let pattern = cases
        .iter()
        .map(|case| regex::escape(case.value))
        .collect::<Vec<_>>()
        .join("|");
    let response = call(
        &mut client,
        "grep",
        json!({"path":FILE_PATH, "pattern":pattern, "output_mode":"count"}),
    )
    .await;
    assert!(text(&response).contains(&format!("{FILE_PATH}:10")));
    for expand in ["none", "callable"] {
        let started = Instant::now();
        let response = call(
            &mut client,
            "grep",
            json!({"path":FILE_PATH, "pattern":pattern, "expand":expand, "view":"source"}),
        )
        .await;
        eprintln!(
            "large redaction fixture: grep {expand}={}ms",
            started.elapsed().as_millis()
        );
        assert_no_secrets(&response, &cases);
        let output = text(&response);
        for case in &cases {
            let line = source
                .lines()
                .position(|line| line.contains(case.value))
                .unwrap()
                + 1;
            assert!(
                output.contains(&format!("{FILE_PATH}:{line}:")),
                "missing match at line {line}"
            );
        }
        assert!(output.matches("[REDACTED]").count() >= cases.len());
    }

    let started = Instant::now();
    for case in &cases {
        // The PEM body alone is not an indexed match in this fixture; use its
        // indexed label to exercise the actual multiline literal presentation.
        let query = if case.source.contains("-----BEGIN PRIVATE KEY-----") {
            "PRIVATE KEY"
        } else {
            case.value
        };
        let response = call(
            &mut client,
            "search",
            json!({"query":query, "caller_context":false}),
        )
        .await;
        assert_no_secrets(&response, &cases);
        assert!(text(&response).contains(FILE_PATH));
        assert!(text(&response).contains("[REDACTED]"));
    }
    eprintln!(
        "large redaction fixture: ten searches={}ms",
        started.elapsed().as_millis()
    );
    let response = call(
        &mut client,
        "search",
        json!({"query":cases[9].value, "caller_context":false}),
    )
    .await;
    assert_no_secrets(&response, &cases);
    assert!(text(&response).contains("No indexed matches"));

    let response = call(
        &mut client,
        "search",
        json!({"query":"public-fixture-neighbor", "caller_context":false}),
    )
    .await;
    assert_no_secrets(&response, &cases);
    assert!(text(&response).contains("public-fixture-neighbor"));
    assert_eq!(
        std::fs::read_to_string(repo.path().join(FILE_PATH)).unwrap(),
        source
    );
}
