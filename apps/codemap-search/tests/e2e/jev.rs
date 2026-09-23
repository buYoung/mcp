//! Cross-mode Jev regressions through the stdio MCP server. Disabled and keyless servers must
//! answer like a server without Jev settings; in debug builds, scripted offline answers then
//! check source identity, observations, masking, whole-call fallback and output caps. No case
//! reaches the provider: the harness strips every key variable, and evaluated cases use the
//! debug-only scripted transport.

use std::path::Path;

use serde_json::{json, Value};

use super::helpers::{create_mock_repo, McpClient};

const POLICY_PATH: &str = "src/upload/policy.rs";
const POLICY_RS: &str = r#"/// Maximum attempts before an upload gives up.
pub const MAX_ATTEMPTS: u32 = 3;

pub struct RetryPolicy {
    max_attempts: u32,
}

impl RetryPolicy {
    pub fn new() -> Self {
        Self { max_attempts: MAX_ATTEMPTS }
    }

    pub fn allows(&self, attempt: u32) -> bool {
        attempt < self.max_attempts
    }
}

pub fn retry_upload(policy: &RetryPolicy, chunk: &[u8]) -> bool {
    let mut attempt = 0;
    while policy.allows(attempt) {
        if send_chunk(chunk) {
            return true;
        }
        attempt += 1;
    }
    false
}

fn send_chunk(chunk: &[u8]) -> bool {
    !chunk.is_empty()
}

pub fn format_upload_banner(name: &str) -> String {
    format!("uploading {name}")
}
"#;
const SESSION_PATH: &str = "web/uploadSession.ts";
const SESSION_TS: &str = r#"export const RETRY_LIMIT = 3;

export class UploadSession {
  private attempts = 0;

  canRetry(): boolean {
    return this.attempts < RETRY_LIMIT;
  }

  retryUpload(send: () => boolean): boolean {
    while (this.canRetry()) {
      if (send()) {
        return true;
      }
      this.attempts += 1;
    }
    return false;
  }

  describeBanner(name: string): string {
    const banner = () => `uploading ${name}`;
    return banner();
  }
}
"#;
const BANNER_PATH: &str = "src/banner.rs";
const BANNER_RS: &str = r#"pub fn render_banner(title: &str) -> String {
    format!("== {title} ==")
}
"#;
const FIXTURE: [(&str, &str); 3] = [
    (POLICY_PATH, POLICY_RS),
    (SESSION_PATH, SESSION_TS),
    (BANNER_PATH, BANNER_RS),
];
const SEARCH_QUERY: &str = "retry upload banner";
const TASK_QUERY: &str = "How does upload retry decide to stop?";

fn bypass_note(tool: &str, reason: &str) -> String {
    format!("\n\n[jev {tool}: bypassed ({reason}) · original output unchanged]")
}

/// Calls whose responses must not depend on disabled or keyless Jev settings, in call order,
/// with the bypass reason a keyless server with both modes enabled appends.
fn compatibility_calls() -> [(&'static str, Value, Option<&'static str>); 8] {
    [
        (
            "search",
            json!({ "query": SEARCH_QUERY }),
            Some("no_task_query"),
        ),
        (
            "search",
            json!({ "query": SEARCH_QUERY, "task_query": TASK_QUERY }),
            Some("missing_credential"),
        ),
        ("overview", json!({}), Some("no_task_query")),
        (
            "overview",
            json!({ "task_query": TASK_QUERY }),
            Some("missing_credential"),
        ),
        (
            "overview",
            json!({ "path": "src", "task_query": TASK_QUERY }),
            Some("not_repository_root"),
        ),
        (
            "read",
            json!({ "file_path": POLICY_PATH, "view": "source" }),
            None,
        ),
        (
            "grep",
            json!({ "pattern": "attempt", "view": "source" }),
            None,
        ),
        ("find", json!({ "pattern": "*.ts" }), None),
    ]
}

/// The `tools/list` result and each compatibility call's result from a fresh server over
/// `repo`, reading global settings from `global` when given.
async fn record_session(repo: &Path, global: Option<&Path>) -> (Value, Vec<Value>) {
    let mut client = match global {
        Some(global) => {
            McpClient::spawn_with_env(repo, &[("CODEMAP_HOME", global.to_str().unwrap())]).await
        }
        None => McpClient::spawn(repo).await,
    }
    .unwrap();
    let tools = client.send_request("tools/list", json!({})).await.unwrap();
    let mut results = Vec::new();
    for (name, arguments, _) in compatibility_calls() {
        let response = client
            .send_request(
                "tools/call",
                json!({ "name": name, "arguments": arguments }),
            )
            .await
            .unwrap();
        assert!(response["error"].is_null(), "{name}: {response}");
        results.push(response["result"].clone());
    }
    (tools["result"].clone(), results)
}

fn global_config(settings: &str) -> tempfile::TempDir {
    let global = tempfile::tempdir().unwrap();
    std::fs::write(
        global.path().join("config.toml"),
        format!("[analysis.jev]\n{settings}\n"),
    )
    .unwrap();
    global
}

#[tokio::test]
async fn test_jev_disabled_and_keyless_servers_answer_like_the_baseline() {
    let repo = create_mock_repo(&FIXTURE).unwrap();
    let (baseline_tools, baseline) = record_session(repo.path(), None).await;

    let disabled = global_config("is_overview_enabled = false\nis_search_filter_enabled = false");
    let (tools, results) = record_session(repo.path(), Some(disabled.path())).await;
    assert_eq!(tools, baseline_tools);
    assert_eq!(results, baseline);

    // Both modes on without a key: the production transport path, which never gets a key.
    let keyless = global_config("is_overview_enabled = true\nis_search_filter_enabled = true");
    let (tools, results) = record_session(repo.path(), Some(keyless.path())).await;
    for ((name, _, reason), (result, baseline_result)) in compatibility_calls()
        .into_iter()
        .zip(results.iter().zip(&baseline))
    {
        let baseline_text = baseline_result["content"][0]["text"].as_str().unwrap();
        let expected = match reason {
            Some(reason) => format!("{baseline_text}{}", bypass_note(name, reason)),
            None => baseline_text.to_string(),
        };
        assert_eq!(result["content"][0]["text"], expected, "{name}");
        assert_eq!(result["content"].as_array().unwrap().len(), 1, "{name}");
    }
    let baseline_tools = baseline_tools["tools"].as_array().unwrap();
    let tools = tools["tools"].as_array().unwrap();
    assert_eq!(tools.len(), baseline_tools.len());
    for (tool, baseline_tool) in tools.iter().zip(baseline_tools) {
        let name = baseline_tool["name"].as_str().unwrap();
        if name != "overview" && name != "search" {
            assert_eq!(tool, baseline_tool, "{name}");
            continue;
        }
        assert_eq!(tool["annotations"]["openWorldHint"], true, "{name}");
        assert_eq!(tool["annotations"]["readOnlyHint"], true, "{name}");
        let description = tool["description"].as_str().unwrap();
        assert!(
            description.starts_with(baseline_tool["description"].as_str().unwrap())
                && description.contains("Jev"),
            "{name}: {description}"
        );
        let mut schema = tool["inputSchema"].clone();
        let mut baseline_schema = baseline_tool["inputSchema"].clone();
        for schema in [&mut schema, &mut baseline_schema] {
            schema["properties"]["task_query"]["description"] = Value::Null;
        }
        assert_eq!(schema, baseline_schema, "{name}");
    }
}

/// Evaluated cases: debug builds answer Jev requests from a script file.
#[cfg(debug_assertions)]
mod scripted {
    use std::collections::{BTreeMap, BTreeSet};

    use super::*;
    use crate::e2e::helpers::{JevScript, JEV_TEST_KEY_ENV};

    /// Credentials from the existing redaction fixtures.
    const CREDENTIAL_RS: &str = "pub const PASSWORD: &str = \"correct-horse-credential\";\npub const API_KEY: &str = \"sk-proj-abcdefghijklmnopqrstuv0123456789\";\npub const SAFE: &str = \"visible-message\";\npub fn authenticate() -> &'static str { PASSWORD }\n";
    const CREDENTIAL_TS: &str = "export const password = 'fixture::auth::credential';\n\nexport function checkPassword(candidate: string): boolean {\n  return candidate === password;\n}\n";
    const SECRET_VALUES: [&str; 3] = [
        "correct-horse-credential",
        "sk-proj-abcdefghijklmnopqrstuv0123456789",
        "fixture::auth::credential",
    ];
    /// Carries a key format the masking rules recognize in free text.
    const CREDENTIAL_TASK_QUERY: &str =
        "Why does authenticate reject the key sk-proj-abcdefghijklmnopqrstuv0123456789?";

    /// A server over `repo` whose global config enables `settings` and names the test key
    /// variable, answering Jev requests with the ordered `rules`.
    struct ScriptedServer {
        client: McpClient,
        script: JevScript,
        _global: tempfile::TempDir,
        _scripts: tempfile::TempDir,
    }

    impl ScriptedServer {
        async fn start(repo: &Path, settings: &str, rules: Value) -> Self {
            let global = global_config(&format!(
                "api_key_env = \"{JEV_TEST_KEY_ENV}\"\nrequest_spacing_ms = 0\n{settings}"
            ));
            let scripts = tempfile::tempdir().unwrap();
            let script = JevScript::new(scripts.path(), json!({ "rules": rules }));
            let client = McpClient::spawn_with_jev(repo, global.path(), &script, true)
                .await
                .unwrap();
            Self {
                client,
                script,
                _global: global,
                _scripts: scripts,
            }
        }

        async fn call(&mut self, name: &str, arguments: Value) -> String {
            let response = self
                .client
                .send_request(
                    "tools/call",
                    json!({ "name": name, "arguments": arguments }),
                )
                .await
                .unwrap();
            text(&response).to_string()
        }

        /// Files recorded as returned source by `search` calls so far.
        async fn search_read_paths(&mut self) -> BTreeSet<String> {
            let report = self
                .call("analyze", json!({ "target": "reads", "tool": "search" }))
                .await;
            let report: Value = serde_json::from_str(&report).unwrap();
            let columns = report["files"]["columns"].as_array().unwrap();
            let path_column = columns.iter().position(|column| column == "path").unwrap();
            report["files"]["rows"]
                .as_array()
                .unwrap()
                .iter()
                .map(|row| row[path_column].as_str().unwrap().to_string())
                .collect()
        }
    }

    fn text(response: &Value) -> &str {
        response["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_else(|| panic!("tool text expected: {response}"))
    }

    /// The output before an appended Jev note.
    fn without_note<'a>(output: &'a str, tool: &str) -> &'a str {
        output.split(&format!("\n\n[jev {tool}:")).next().unwrap()
    }

    fn original_line(path: &str, number: usize) -> &'static str {
        let (_, contents) = FIXTURE
            .iter()
            .find(|(fixture_path, _)| *fixture_path == path)
            .unwrap_or_else(|| panic!("unexpected file {path}"));
        contents.lines().nth(number - 1).unwrap()
    }

    /// `NNN→text` source rows as line number to text.
    fn numbered_rows(block: &str) -> BTreeMap<usize, String> {
        block
            .lines()
            .map(|row| {
                let (number, text) = row
                    .split_once('→')
                    .unwrap_or_else(|| panic!("numbered row expected: {row}"));
                (number.trim().parse().unwrap(), text.to_string())
            })
            .collect()
    }

    /// Displayed source rows per file, from the code fences under each `## N. path` heading.
    /// Fails when a fence is left open or a heading appears inside one.
    fn displayed_rows(output: &str) -> BTreeMap<String, BTreeMap<usize, String>> {
        let mut files: BTreeMap<String, BTreeMap<usize, String>> = BTreeMap::new();
        let mut path = None;
        let mut fenced_block: Option<String> = None;
        for line in output.lines() {
            if line.starts_with("```") {
                match fenced_block.take() {
                    Some(block) => {
                        let path: &String = path.as_ref().expect("source under a file heading");
                        files
                            .entry(path.clone())
                            .or_default()
                            .extend(numbered_rows(&block));
                    }
                    None => fenced_block = Some(String::new()),
                }
            } else if let Some(block) = fenced_block.as_mut() {
                block.push_str(line);
                block.push('\n');
            } else if let Some(heading) = line.strip_prefix("## ") {
                path = heading.split_once(". ").map(|(_, path)| path.to_string());
            }
        }
        assert!(fenced_block.is_none(), "unclosed code fence: {output}");
        files
    }

    /// File headings, declaration headings and declaration rows, in order.
    fn structure(output: &str) -> Vec<&str> {
        output
            .lines()
            .filter(|line| {
                line.starts_with("## ")
                    || line.starts_with("### ")
                    || line.trim_start().starts_with("- Symbol:")
            })
            .collect()
    }

    /// `(name, path, start, end)` for each body the omission summary lists.
    fn omitted_bodies(output: &str) -> BTreeSet<(String, String, usize, usize)> {
        let (_, listed) = output
            .split_once("stay listed above: ")
            .expect("omission summary");
        let (listed, _) = listed.split_once(". Use `read`").unwrap();
        listed
            .split(", ")
            .map(|entry| {
                let mut parts = entry.split('`');
                let name = parts.next().unwrap().trim().to_string();
                let path = parts.next().unwrap().to_string();
                let range = parts.next().unwrap().trim().trim_start_matches('L');
                let (start, end) = range.split_once('-').unwrap();
                (name, path, start.parse().unwrap(), end.parse().unwrap())
            })
            .collect()
    }

    fn body_ranges(bodies: &BTreeSet<(String, String, usize, usize)>) -> BTreeSet<(String, usize)> {
        bodies
            .iter()
            .flat_map(|(_, path, start, end)| (*start..=*end).map(move |line| (path.clone(), line)))
            .collect()
    }

    fn row_keys(rows: &BTreeMap<String, BTreeMap<usize, String>>) -> BTreeSet<(String, usize)> {
        rows.iter()
            .flat_map(|(path, lines)| lines.keys().map(move |line| (path.clone(), *line)))
            .collect()
    }

    #[tokio::test]
    async fn test_jev_filter_keeps_source_identity_boundaries_and_observations() {
        let repo = create_mock_repo(&FIXTURE).unwrap();
        let rules = json!([
            {"type": "noul", "contains": "anner", "noul": 0.95},
            {"type": "noul", "noul": 0.05}
        ]);
        let mut server =
            ScriptedServer::start(repo.path(), "is_search_filter_enabled = true", rules).await;
        // Wait for the index through overview, which records no returned source.
        server.call("overview", json!({})).await;
        let filtered = server
            .call(
                "search",
                json!({ "query": SEARCH_QUERY, "task_query": TASK_QUERY }),
            )
            .await;
        let filtered_reads = server.search_read_paths().await;
        let regular = server
            .call("search", json!({ "query": SEARCH_QUERY }))
            .await;
        let all_reads = server.search_read_paths().await;

        assert!(
            filtered.contains(
                "[jev search: applied · omitted=3/6 bodies · judged=6 · threshold=0.70 · requests=1"
            ),
            "{filtered}"
        );
        assert!(regular.ends_with(&bypass_note("search", "no_task_query")));
        let regular = without_note(&regular, "search");
        let omitted = omitted_bodies(&filtered);
        let expected: BTreeSet<_> = [
            ("describeBanner", SESSION_PATH, 20, 23),
            ("format_upload_banner", POLICY_PATH, 33, 35),
            ("render_banner", BANNER_PATH, 1, 3),
        ]
        .into_iter()
        .map(|(name, path, start, end)| (name.to_string(), path.to_string(), start, end))
        .collect();
        assert_eq!(omitted, expected);

        // Headings, declaration rows and file order stay; every shown row is original source.
        let filtered = without_note(&filtered, "search");
        assert_eq!(structure(filtered), structure(regular));
        let regular_rows = displayed_rows(regular);
        let filtered_rows = displayed_rows(filtered);
        assert_eq!(regular_rows.len(), 3, "{regular}");
        for rows in [&regular_rows, &filtered_rows] {
            for (path, lines) in rows {
                for (number, line) in lines {
                    assert_eq!(line, original_line(path, *number), "{path}:{number}");
                }
            }
        }
        // Exactly the listed bodies disappear.
        let regular_keys = row_keys(&regular_rows);
        let filtered_keys = row_keys(&filtered_rows);
        assert!(filtered_keys.is_subset(&regular_keys));
        let removed: BTreeSet<_> = regular_keys.difference(&filtered_keys).cloned().collect();
        assert_eq!(removed, body_ranges(&omitted));

        // The evaluator received every displayed body exactly as shown.
        let requests = server.script.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0]["state"]["task_query"], TASK_QUERY);
        let questions = requests[0]["questions"].as_object().unwrap();
        assert_eq!(questions.len(), 6);
        for question in questions.values() {
            let instructions = &question["instructions"];
            let declaration = &instructions["declaration"];
            let path = declaration["file"].as_str().unwrap();
            let start = declaration["start_line"].as_u64().unwrap() as usize;
            let end = declaration["end_line"].as_u64().unwrap() as usize;
            let body = numbered_rows(instructions["displayed_body"].as_str().unwrap());
            assert_eq!(
                body.keys().copied().collect::<Vec<_>>(),
                (start..=end).collect::<Vec<_>>()
            );
            for (number, line) in &body {
                assert_eq!(line, original_line(path, *number), "{path}:{number}");
            }
        }

        // A file whose every shown body was omitted is not recorded as returned source.
        let shown_files: BTreeSet<String> = [POLICY_PATH, SESSION_PATH]
            .map(String::from)
            .into_iter()
            .collect();
        assert_eq!(filtered_reads, shown_files);
        let mut every_file = shown_files;
        every_file.insert(BANNER_PATH.to_string());
        assert_eq!(all_reads, every_file);
    }

    #[tokio::test]
    async fn test_jev_model_bound_evidence_follows_the_redaction_setting() {
        let rules = json!([
            {"type": "noul", "noul": 0.05},
            {"type": "score", "probabilities": [0.0, 0.0, 0.2, 0.8]},
            {"type": "choice", "choice": "entry"}
        ]);
        let settings = "is_overview_enabled = true\nis_search_filter_enabled = true";
        for is_redact_enabled in [true, false] {
            let repo_config = format!(
                "# codemap-config-version: 24\n[output]\nis_redact_enabled = {is_redact_enabled}\n"
            );
            let repo = create_mock_repo(&[
                ("src/lib.rs", CREDENTIAL_RS),
                ("web/auth.ts", CREDENTIAL_TS),
                (".codemap/config.toml", repo_config.as_str()),
            ])
            .unwrap();
            let mut server = ScriptedServer::start(repo.path(), settings, rules.clone()).await;
            let overview = server
                .call("overview", json!({ "task_query": CREDENTIAL_TASK_QUERY }))
                .await;
            let search = server
                .call(
                    "search",
                    json!({ "query": "authenticate password", "task_query": CREDENTIAL_TASK_QUERY }),
                )
                .await;
            assert!(
                overview.contains("[jev overview: applied · status=matched · requests=2"),
                "{overview}"
            );
            assert!(
                search.contains("[jev search: applied · omitted=0/"),
                "{search}"
            );
            let requests = server.script.requests();
            assert_eq!(requests.len(), 3, "file scores, roles, search bodies");
            let bodies: Vec<String> = requests[2]["questions"]
                .as_object()
                .unwrap()
                .values()
                .map(|question| {
                    question["instructions"]["displayed_body"]
                        .as_str()
                        .unwrap()
                        .to_string()
                })
                .collect();
            let sent: Vec<String> = requests.iter().map(Value::to_string).collect();
            if is_redact_enabled {
                for delivered in sent.iter().chain([&overview, &search]) {
                    for secret in SECRET_VALUES {
                        assert!(!delivered.contains(secret), "{secret} leaked: {delivered}");
                    }
                }
                for request in &requests {
                    assert_eq!(
                        request["state"]["task_query"],
                        "Why does authenticate reject the key [REDACTED]?"
                    );
                }
                assert!(
                    bodies
                        .iter()
                        .any(|body| body.contains("PASSWORD: &str = \"[REDACTED]\";")),
                    "{bodies:?}"
                );
            } else {
                // Unmasked output is sent as the client would receive it.
                assert!(search.contains("correct-horse-credential"), "{search}");
                assert!(
                    bodies
                        .iter()
                        .any(|body| body.contains("PASSWORD: &str = \"correct-horse-credential\";")),
                    "{bodies:?}"
                );
                for request in &requests {
                    assert_eq!(request["state"]["task_query"], CREDENTIAL_TASK_QUERY);
                }
            }
            assert_eq!(
                std::fs::read_to_string(repo.path().join("src/lib.rs")).unwrap(),
                CREDENTIAL_RS
            );
        }
    }

    #[tokio::test]
    async fn test_jev_invalid_answers_fall_back_to_the_whole_regular_call() {
        let repo = create_mock_repo(&FIXTURE).unwrap();
        let settings = "is_overview_enabled = true\nis_search_filter_enabled = true";
        let mut server = ScriptedServer::start(repo.path(), settings, json!([])).await;
        let search = server
            .call("search", json!({ "query": SEARCH_QUERY }))
            .await;
        let base_search = without_note(&search, "search").to_string();
        let overview = server.call("overview", json!({})).await;
        let base_overview = without_note(&overview, "overview").to_string();
        let task_search = json!({ "query": SEARCH_QUERY, "task_query": TASK_QUERY });
        let task_overview = json!({ "task_query": TASK_QUERY });

        // A Noul probability outside [0, 1] rejects the whole answer set.
        server.script.update(json!({"rules": [
            {"type": "noul", "contains": "anner", "noul": 1.5},
            {"type": "noul", "noul": 0.05}
        ]}));
        let search = server.call("search", task_search.clone()).await;
        assert!(
            search.contains(
                "[jev search: fallback (invalid_answer) · original output preserved · requests=1"
            ),
            "{search}"
        );
        assert_eq!(without_note(&search, "search"), base_search);

        // Score probabilities that do not sum to one fail the file stage.
        server.script.update(json!({"rules": [
            {"type": "score", "probabilities": [0.1, 0.1, 0.1, 0.2]}
        ]}));
        let overview = server.call("overview", task_overview.clone()).await;
        assert!(
            overview.contains(
                "[jev overview: fallback (invalid_answer) · original output preserved · requests=1"
            ),
            "{overview}"
        );
        assert_eq!(without_note(&overview, "overview"), base_overview);

        // Missing role answers discard the completed file ranking too.
        server.script.update(json!({"rules": [
            {"type": "score", "contains": "policy.rs", "probabilities": [0.0, 0.0, 0.2, 0.8]},
            {"type": "score", "probabilities": [1.0, 0.0, 0.0, 0.0]}
        ]}));
        let overview = server.call("overview", task_overview).await;
        assert!(
            overview.contains(
                "[jev overview: fallback (answer_set_mismatch) · original output preserved · requests=2"
            ),
            "{overview}"
        );
        assert!(!overview.contains("## Indexed file recommendations"));
        assert_eq!(without_note(&overview, "overview"), base_overview);
        assert_eq!(server.script.requests().len(), 4);
    }

    #[tokio::test]
    async fn test_jev_additions_stay_within_the_output_caps() {
        // A current repo config without active output limits, so global limits apply.
        let mut files = FIXTURE.to_vec();
        files.push((".codemap/config.toml", "# codemap-config-version: 24\n"));
        let repo = create_mock_repo(&files).unwrap();
        let rules = json!([
            {"type": "noul", "noul": 0.05},
            {"type": "score", "contains": "policy.rs", "probabilities": [0.0, 0.0, 0.2, 0.8]},
            {"type": "score", "probabilities": [1.0, 0.0, 0.0, 0.0]},
            {"type": "choice", "choice": "entry"}
        ]);
        let modes = "is_overview_enabled = true\nis_search_filter_enabled = true";
        let task_search = json!({ "query": SEARCH_QUERY, "task_query": TASK_QUERY });
        let task_overview = json!({ "task_query": TASK_QUERY });

        let mut server = ScriptedServer::start(repo.path(), modes, rules.clone()).await;
        let overview = server.call("overview", json!({})).await;
        let base_overview = without_note(&overview, "overview").to_string();
        let search = server
            .call("search", json!({ "query": SEARCH_QUERY }))
            .await;
        let base_search_bytes = without_note(&search, "search").len();
        drop(server);

        // Too little room: overview sends nothing and drops its note; the filtered search is
        // evaluated, but its note does not fit, so the regular output is returned.
        let overview_cap = base_overview.len() + 300;
        let search_cap = base_search_bytes + 40;
        let tight = format!(
            "{modes}\n[output.overview]\nmax_bytes = {overview_cap}\n[output.search]\nmax_bytes = {search_cap}"
        );
        let mut server = ScriptedServer::start(repo.path(), &tight, rules.clone()).await;
        let overview = server.call("overview", task_overview.clone()).await;
        assert_eq!(overview, base_overview);
        assert!(server.script.requests().is_empty());
        let plain = server
            .call("search", json!({ "query": SEARCH_QUERY }))
            .await;
        let filtered = server.call("search", task_search.clone()).await;
        assert!(
            plain.len() <= search_cap && !plain.contains("[jev"),
            "{plain}"
        );
        assert_eq!(filtered, plain);
        assert_eq!(server.script.requests().len(), 1);
        drop(server);

        // Enough room: both additions fit within their caps.
        let overview_cap = base_overview.len() + 6_000;
        let search_cap = base_search_bytes + 1_000;
        let roomy = format!(
            "{modes}\n[output.overview]\nmax_bytes = {overview_cap}\n[output.search]\nmax_bytes = {search_cap}"
        );
        let mut server = ScriptedServer::start(repo.path(), &roomy, rules).await;
        let overview = server.call("overview", task_overview).await;
        assert!(overview.len() <= overview_cap, "{overview}");
        assert!(overview.starts_with(&base_overview), "{overview}");
        assert!(
            overview.contains("### 1. src/upload/policy.rs"),
            "{overview}"
        );
        assert!(
            overview.contains("[jev overview: applied · status=matched · requests=2"),
            "{overview}"
        );
        let filtered = server.call("search", task_search).await;
        assert!(filtered.len() <= search_cap, "{filtered}");
        assert!(
            filtered.contains("[jev search: applied · omitted=0/6 bodies"),
            "{filtered}"
        );
        assert_eq!(without_note(&filtered, "search").len(), base_search_bytes);
    }
}
