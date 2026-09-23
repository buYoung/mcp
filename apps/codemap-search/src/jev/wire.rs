use serde::Deserialize;
use serde_json::{Map, Value};

use super::error::HttpFailureClass;
use super::evaluator::Usage;
use super::JevError;

/// The documented response envelope. Unknown fields are ignored.
#[derive(Deserialize)]
pub(crate) struct WireResponse {
    pub(crate) model: String,
    pub(crate) answers: Map<String, Value>,
    usage: WireUsage,
}

#[derive(Deserialize)]
struct WireUsage {
    input_tokens: u64,
    output_tokens: u64,
}

impl WireResponse {
    pub(crate) fn usage(&self) -> Usage {
        Usage {
            input_tokens: self.usage.input_tokens,
            output_tokens: self.usage.output_tokens,
        }
    }
}

pub(crate) fn parse_response(body: &[u8]) -> Result<WireResponse, JevError> {
    // serde messages can quote response values; report only the failure position.
    serde_json::from_slice(body).map_err(|error| JevError::MalformedResponse {
        reason: format!(
            "response does not match the documented envelope ({:?} at line {} column {})",
            error.classify(),
            error.line(),
            error.column()
        ),
    })
}

/// Maps a non-200 status to the shared failure contract. The body is inspected in memory
/// only to recognise context-size rejections and is never retained.
pub(crate) fn classify_http_failure(status: u16, body: &[u8]) -> JevError {
    if status == 413 || (matches!(status, 400 | 422) && mentions_context_limit(body)) {
        return JevError::ProviderContextLimit { status };
    }
    let class = match status {
        401 | 403 => HttpFailureClass::Unauthorized,
        400 | 404 | 422 => HttpFailureClass::InvalidRequest,
        429 => HttpFailureClass::RateLimited,
        503 | 529 => HttpFailureClass::Overloaded,
        500..=599 => HttpFailureClass::Server,
        _ => HttpFailureClass::Other,
    };
    JevError::HttpStatus { status, class }
}

fn mentions_context_limit(body: &[u8]) -> bool {
    let text = String::from_utf8_lossy(body).to_ascii_lowercase();
    let mentions_size = ["token", "context"].iter().any(|word| text.contains(word));
    let mentions_limit = ["limit", "exceed", "too long", "too large", "maximum"]
        .iter()
        .any(|word| text.contains(word));
    mentions_size && mentions_limit
}
