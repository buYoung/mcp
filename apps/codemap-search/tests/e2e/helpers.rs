use std::path::{Path, PathBuf};
use std::process::Stdio;
use tempfile::TempDir;
use tokio::process::{Command, Child, ChildStdin};
use tokio::io::{BufReader, AsyncBufReadExt, AsyncWriteExt};
use serde_json::Value;

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

/// Helper to invoke the codemap-search binary for CLI tests using assert_cmd
pub fn run_cli(args: &[&str], cwd: &Path) -> assert_cmd::assert::Assert {
    use assert_cmd::prelude::*;
    let mut cmd = std::process::Command::cargo_bin("codemap-search")
        .expect("Failed to find codemap-search binary");
    cmd.current_dir(cwd).args(args).assert()
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
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit()) // Keeps logging / errors visible in test logs
            .spawn()?;

        let stdin = child.stdin.take().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::Other, "Failed to open child stdin")
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::Other, "Failed to open child stdout")
        })?;
        let stdout_reader = BufReader::new(stdout);

        Ok(Self {
            child,
            stdin,
            stdout_reader,
            request_id: 1,
        })
    }

    /// Send a JSON-RPC request to the MCP server and wait for the response
    pub async fn send_request(&mut self, method: &str, params: Value) -> Result<Value, String> {
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
        self.stdin.write_all(payload.as_bytes()).await.map_err(|e| e.to_string())?;
        self.stdin.flush().await.map_err(|e| e.to_string())?;

        // Read single line response from server's stdout
        let mut line = String::new();
        self.stdout_reader.read_line(&mut line).await.map_err(|e| e.to_string())?;
        
        // Parse and return JSON response
        let response: Value = serde_json::from_str(&line).map_err(|e| e.to_string())?;
        Ok(response)
    }

    /// Abruptly kill the server to verify clean exit behavior
    pub async fn kill(mut self) -> Result<(), std::io::Error> {
        self.child.kill().await?;
        let _ = self.child.wait().await?;
        Ok(())
    }
}
