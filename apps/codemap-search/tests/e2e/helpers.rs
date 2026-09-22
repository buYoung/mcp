use serde_json::Value;
use std::path::Path;
use std::process::Stdio;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::time::sleep;

pub const OPTIONAL_LANGUAGE_SUPPORT_CONFIG: &str = "\
[language_support]
is_shell_support_enabled = true
is_infrastructure_support_enabled = true
is_interface_support_enabled = true
is_build_support_enabled = true
";

pub const EVENT_NAVIGATION_CONFIG: &str = include_str!("../fixtures/event_navigation/config.toml");

pub fn event_navigation_repo() -> TempDir {
    create_mock_repo(&[
        (
            "src/known.ts",
            include_str!("../fixtures/event_navigation/known.ts"),
        ),
        (
            "src/events.ts",
            include_str!("../fixtures/event_navigation/events.ts"),
        ),
        (
            "src/barrel.ts",
            include_str!("../fixtures/event_navigation/barrel.ts"),
        ),
        (
            "src/users.ts",
            include_str!("../fixtures/event_navigation/users.ts"),
        ),
        (
            "src/other.ts",
            include_str!("../fixtures/event_navigation/other.ts"),
        ),
        (
            "src/controls.ts",
            include_str!("../fixtures/event_navigation/controls.ts"),
        ),
        (".codemap/config.toml", EVENT_NAVIGATION_CONFIG),
    ])
    .unwrap()
}

/// Helper to dynamically build a mock directory with specific files
pub fn create_mock_repo(files: &[(&str, &str)]) -> Result<TempDir, std::io::Error> {
    let temp_dir = tempfile::tempdir()?;
    for (rel_path, content) in files {
        let file_path = temp_dir.path().join(rel_path);
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(file_path, content)?;
    }
    Ok(temp_dir)
}

/// Keep native-library fixtures' cwd/config globals inside their own test process.
pub fn run_isolated_jev_test(name: &str, root: &Path) {
    let result = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture"])
        .current_dir(root)
        .env("CODEMAP_HOME", root.join("isolated-global"))
        .env("CODEMAP_JEV_ISOLATED_TEST", "1")
        .env_remove("CODEMAP_JEV_ABSENT_KEY")
        .output().unwrap();
    assert!(result.status.success(), "isolated test failed:\n{}\n{}", String::from_utf8_lossy(&result.stdout), String::from_utf8_lossy(&result.stderr));
}

/// Helper to invoke the codemap-search binary for CLI tests using assert_cmd
pub fn run_cli(args: &[&str], cwd: &Path) -> assert_cmd::assert::Assert {
    use assert_cmd::prelude::*;
    let mut cmd = std::process::Command::cargo_bin("codemap-search")
        .expect("Failed to find codemap-search binary");
    // Isolate the global config home to the test's own dir so the suite never reads the
    // developer's real ~/.codemap (Child 05 hermeticity); absent file → defaults.
    cmd.current_dir(cwd)
        .env("CODEMAP_HOME", cwd)
        .args(args)
        .assert()
}

/// Async MCP Client for interacting with the JSON-RPC server over stdio
pub struct McpClient {
    pub child: Child,
    pub stdin: ChildStdin,
    pub stdout_reader: BufReader<tokio::process::ChildStdout>,
    pub request_id: i64,
}

impl McpClient {
    /// Spawn the codemap-search binary in MCP server mode
    pub async fn spawn(cwd: &Path) -> Result<Self, std::io::Error> {
        // Obtains path to the cargo-built binary
        let binary_path = assert_cmd::cargo::cargo_bin("codemap-search");

        let mut child = Command::new(binary_path)
            .arg("mcp") // Launches MCP server mode
            .current_dir(cwd)
            // Hermetic global config home — never read the developer's real ~/.codemap.
            .env("CODEMAP_HOME", cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit()) // Keeps logging / errors visible in test logs
            .spawn()?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| std::io::Error::other("Failed to open child stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| std::io::Error::other("Failed to open child stdout"))?;
        let stdout_reader = BufReader::new(stdout);

        Ok(Self {
            child,
            stdin,
            stdout_reader,
            request_id: 1,
        })
    }

    /// Send a JSON-RPC request and wait for the response. For `tools/call`, transparently
    /// poll through the initial background-index warm-up: the server answers immediately
    /// while indexing, tagging search/overview output as "warming up", and tests want the
    /// post-index result. Other methods return on the first response.
    pub async fn send_request(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let start = Instant::now();
        loop {
            let response = self.send_request_once(method, params.clone()).await?;
            if method == "tools/call"
                && (response_is_warming(&response)
                    || overview_response_is_waiting_for_index(&params, &response))
                && start.elapsed() < Duration::from_secs(10)
            {
                sleep(Duration::from_millis(50)).await;
                continue;
            }
            return Ok(response);
        }
    }

    /// One JSON-RPC round trip over stdio.
    async fn send_request_once(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.request_id;
        self.request_id += 1;

        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        });

        // Serialize and ensure line framing
        let mut payload = serde_json::to_string(&request).map_err(|e| e.to_string())?;
        payload.push('\n');

        // Write to server's stdin
        self.stdin
            .write_all(payload.as_bytes())
            .await
            .map_err(|e| e.to_string())?;
        self.stdin.flush().await.map_err(|e| e.to_string())?;

        // Read single line response from server's stdout
        let mut line = String::new();
        self.stdout_reader
            .read_line(&mut line)
            .await
            .map_err(|e| e.to_string())?;

        // Parse and return JSON response
        let response: Value = serde_json::from_str(&line).map_err(|e| e.to_string())?;
        Ok(response)
    }

    /// Call a tool repeatedly until `predicate` passes on its result text, or 15s elapse;
    /// returns the final response. For tests that mutate the repo and then expect a
    /// background refresh to reflect the change — the trigger is debounced and indexing is
    /// async, so the updated result appears on a subsequent poll, not the first call.
    pub async fn send_tool_until<F>(
        &mut self,
        name: &str,
        arguments: Value,
        predicate: F,
    ) -> Result<Value, String>
    where
        F: Fn(&str) -> bool,
    {
        let params = serde_json::json!({ "name": name, "arguments": arguments });
        let start = Instant::now();
        loop {
            let response = self.send_request_once("tools/call", params.clone()).await?;
            let text = response["result"]["content"][0]["text"]
                .as_str()
                .unwrap_or("");
            if predicate(text) || start.elapsed() >= Duration::from_secs(15) {
                return Ok(response);
            }
            sleep(Duration::from_millis(50)).await;
        }
    }

    /// Abruptly kill the server to verify clean exit behavior
    pub async fn kill(mut self) -> Result<(), std::io::Error> {
        self.child.kill().await?;
        let _ = self.child.wait().await?;
        Ok(())
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        // A test's TempDir is removed as soon as its future completes. Without terminating
        // this child, its MCP watcher/indexer can keep using that removed workspace while
        // later tests run in parallel, eventually exhausting shared process and watcher
        // resources. Reap the process after signalling it: Tokio's orphan reaper is
        // best-effort and leaving every test child to it can accumulate zombies during the
        // default-parallel suite. The bounded wait avoids hanging a failed test forever;
        // explicit `kill` above remains the asynchronous path for lifecycle assertions.
        let _ = self.child.start_kill();
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            match self.child.try_wait() {
                Ok(Some(_)) | Err(_) => break,
                Ok(None) => std::thread::sleep(Duration::from_millis(1)),
            }
        }
    }
}

/// True when a `tools/call` response carries the search/overview warm-up notice, i.e. the
/// initial background index is still building.
fn response_is_warming(response: &Value) -> bool {
    response
        .get("result")
        .and_then(|r| r.get("content"))
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first())
        .and_then(|item| item.get("text"))
        .and_then(|t| t.as_str())
        .is_some_and(|text| text.contains("warming up"))
}

/// A contended initial pass can finish without publishing a file snapshot, while the request
/// itself queues the next refresh. Retry only this overview-specific transient; ordinary tool
/// errors and genuinely unsupported files still surface after the same bounded window.
fn overview_response_is_waiting_for_index(params: &Value, response: &Value) -> bool {
    params.get("name").and_then(Value::as_str) == Some("overview")
        && response
            .get("error")
            .and_then(|error| error.get("message"))
            .and_then(Value::as_str)
            .is_some_and(|message| message.contains("is not in the codemap"))
}
