use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    pub params: Option<serde_json::Value>,
    pub id: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    // A JSON-RPC response carries either `result` or `error`, never both — omit the
    // unused member so success frames don't ship `"error": null` (and vice versa).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<serde_json::Value>,
    pub id: Option<serde_json::Value>,
}

/// A version-dependent Codex code-mode hint, not a negotiated capability or an
/// acknowledgement that the client retained the result. Only log validated IDs.
pub(super) fn exec_call_id(params: Option<&serde_json::Value>) -> Option<&str> {
    let call_id = params?.get("_meta")?.get("callId")?.as_str()?;
    let uuid = call_id.strip_prefix("exec-")?;
    (uuid.len() == 36
        && uuid.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        }))
    .then_some(call_id)
}

pub(crate) struct LimitedLineReader<R> {
    reader: R,
    buffer: Vec<u8>,
    search_start: usize,
    max_line_length: usize,
}

impl<R: tokio::io::AsyncRead + Unpin> LimitedLineReader<R> {
    pub(crate) fn new(reader: R, max_line_length: usize) -> Self {
        Self {
            reader,
            buffer: Vec::new(),
            search_start: 0,
            max_line_length,
        }
    }

    pub(crate) async fn next_line(&mut self) -> Result<Option<String>, String> {
        let mut byte_buf = [0u8; 1024];
        loop {
            // The prefix was already checked before the preceding read. Rechecking it
            // for every 1 KiB chunk makes large frames quadratic in their byte length.
            if let Some(offset) = self.buffer[self.search_start..]
                .iter()
                .position(|&b| b == b'\n')
            {
                let pos = self.search_start + offset;
                let mut line_bytes = self.buffer.drain(..=pos).collect::<Vec<u8>>();
                self.search_start = 0;
                let mut len = line_bytes.len();
                if len > 0 && line_bytes[len - 1] == b'\n' {
                    len -= 1;
                }
                if len > 0 && line_bytes[len - 1] == b'\r' {
                    len -= 1;
                }
                line_bytes.truncate(len);
                let line_str =
                    String::from_utf8(line_bytes).map_err(|e| format!("Invalid UTF-8: {}", e))?;
                return Ok(Some(line_str));
            }
            self.search_start = self.buffer.len();

            use tokio::io::AsyncReadExt;
            let n = self
                .reader
                .read(&mut byte_buf)
                .await
                .map_err(|e| format!("Read error: {}", e))?;
            if n == 0 {
                if self.buffer.is_empty() {
                    return Ok(None);
                } else {
                    let mut line_bytes = std::mem::take(&mut self.buffer);
                    self.search_start = 0;
                    let mut len = line_bytes.len();
                    if len > 0 && line_bytes[len - 1] == b'\n' {
                        len -= 1;
                    }
                    if len > 0 && line_bytes[len - 1] == b'\r' {
                        len -= 1;
                    }
                    line_bytes.truncate(len);
                    let line_str = String::from_utf8(line_bytes)
                        .map_err(|e| format!("Invalid UTF-8: {}", e))?;
                    return Ok(Some(line_str));
                }
            }
            self.buffer.extend_from_slice(&byte_buf[..n]);
            if self.buffer.len() > self.max_line_length {
                return Err("Max line length exceeded".to_string());
            }
        }
    }
}
