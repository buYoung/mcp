use std::fmt;
use std::sync::Arc;

use reqwest::header::{HeaderValue, AUTHORIZATION, CONTENT_TYPE};

use super::error::TransportErrorKind;
use super::evaluator::TransportPolicy;
use super::{BoxFuture, JevError};

/// The only production endpoint. There is no runtime override.
pub const ENDPOINT: &str = "https://api.typesafe.ai/v1/systemone";
/// Largest accepted success body; a full 80 KB batch answers in a few tens of KB.
pub const MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
/// Error bodies are read only far enough to classify them.
const MAX_ERROR_BODY_BYTES: usize = 64 * 1024;
const MAX_API_KEY_BYTES: usize = 4096;

/// Raw provider reply before validation.
pub struct TransportResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TransportError {
    pub kind: TransportErrorKind,
    /// Error chain text without the request URL, headers, or body.
    pub detail: String,
}

impl From<TransportError> for JevError {
    fn from(error: TransportError) -> Self {
        JevError::Transport {
            kind: error.kind,
            detail: error.detail,
        }
    }
}

/// Posts one encoded Jev request body. Implementations own authentication; the evaluator
/// never sees credentials. Test doubles implement this to keep suites offline.
pub trait JevTransport: Send + Sync {
    fn post(&self, body: Vec<u8>) -> BoxFuture<'_, Result<TransportResponse, TransportError>>;
}

impl<T: JevTransport + ?Sized> JevTransport for Arc<T> {
    fn post(&self, body: Vec<u8>) -> BoxFuture<'_, Result<TransportResponse, TransportError>> {
        (**self).post(body)
    }
}

/// An operator-supplied TypeSafe API key. Debug output never shows the value.
#[derive(Clone, PartialEq, Eq)]
pub struct ApiKey(String);

impl ApiKey {
    pub fn new(value: impl Into<String>) -> Result<Self, JevError> {
        let value = value.into();
        let value = value.trim();
        if value.is_empty() {
            return Err(JevError::InvalidCredential {
                reason: "the API key is empty",
            });
        }
        if value.len() > MAX_API_KEY_BYTES || !value.bytes().all(|byte| byte.is_ascii_graphic()) {
            return Err(JevError::InvalidCredential {
                reason: "the API key must be printable ASCII without spaces",
            });
        }
        Ok(Self(value.to_string()))
    }
}

impl fmt::Debug for ApiKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ApiKey([redacted])")
    }
}

/// HTTPS transport for `ENDPOINT`: rustls with the ring provider and the OS trust store,
/// HTTP/1.1 only, no redirects, and no automatic retries. Idle pooled connections are
/// discarded after `TransportPolicy::pool_idle_timeout`, so a POST never starts on a
/// connection idle for longer. hyper may transparently re-send a request whose bytes were
/// never written because a reused idle connection had already closed; that replaces the
/// connection and is not a retry of a delivered POST.
pub struct HttpsTransport {
    client: reqwest::Client,
    endpoint: reqwest::Url,
    authorization: HeaderValue,
}

impl HttpsTransport {
    pub fn new(api_key: &ApiKey, policy: &TransportPolicy) -> Result<Self, JevError> {
        let endpoint = reqwest::Url::parse(ENDPOINT).expect("the endpoint constant is a valid URL");
        Self::build(endpoint, api_key, policy, true)
    }

    /// Plain-HTTP loopback transport for connection-reuse tests; never compiled into
    /// production builds.
    #[cfg(test)]
    pub(crate) fn loopback_for_test(
        endpoint: &str,
        api_key: &ApiKey,
        policy: &TransportPolicy,
    ) -> Result<Self, JevError> {
        let endpoint = reqwest::Url::parse(endpoint).expect("test endpoint is a valid URL");
        assert_eq!(endpoint.host_str(), Some("127.0.0.1"));
        Self::build(endpoint, api_key, policy, false)
    }

    fn build(
        endpoint: reqwest::Url,
        api_key: &ApiKey,
        policy: &TransportPolicy,
        is_https_only: bool,
    ) -> Result<Self, JevError> {
        let mut authorization =
            HeaderValue::from_str(&format!("Bearer {}", api_key.0)).map_err(|_| {
                JevError::InvalidCredential {
                    reason: "the API key is not a valid header value",
                }
            })?;
        authorization.set_sensitive(true);
        let client = reqwest::Client::builder()
            .tls_backend_preconfigured(tls_config()?)
            .https_only(is_https_only)
            .http1_only()
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .pool_idle_timeout(policy.pool_idle_timeout)
            .pool_max_idle_per_host(policy.max_in_flight_requests)
            .user_agent(concat!("codemap-search/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|error| JevError::Transport {
                kind: TransportErrorKind::Other,
                detail: error_chain(&error.without_url()),
            })?;
        Ok(Self {
            client,
            endpoint,
            authorization,
        })
    }

    async fn send(&self, body: Vec<u8>) -> Result<TransportResponse, TransportError> {
        let mut response = self
            .client
            .post(self.endpoint.clone())
            .header(CONTENT_TYPE, "application/json")
            .header(AUTHORIZATION, self.authorization.clone())
            .body(body)
            .send()
            .await
            .map_err(transport_error)?;
        let status = response.status().as_u16();
        let limit_bytes = if status == 200 {
            MAX_RESPONSE_BYTES
        } else {
            MAX_ERROR_BODY_BYTES
        };
        let mut collected = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(transport_error)? {
            let room = limit_bytes - collected.len();
            if chunk.len() > room {
                if status == 200 {
                    return Err(TransportError {
                        kind: TransportErrorKind::ResponseTooLarge,
                        detail: format!("response body exceeded {limit_bytes} bytes"),
                    });
                }
                collected.extend_from_slice(&chunk[..room]);
                break;
            }
            collected.extend_from_slice(&chunk);
        }
        Ok(TransportResponse {
            status,
            body: collected,
        })
    }
}

impl JevTransport for HttpsTransport {
    fn post(&self, body: Vec<u8>) -> BoxFuture<'_, Result<TransportResponse, TransportError>> {
        Box::pin(self.send(body))
    }
}

impl fmt::Debug for HttpsTransport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HttpsTransport")
            .field("endpoint", &self.endpoint.as_str())
            .finish_non_exhaustive()
    }
}

/// An explicit provider keeps process-wide rustls defaults untouched.
fn tls_config() -> Result<rustls::ClientConfig, JevError> {
    use rustls_platform_verifier::BuilderVerifierExt;

    let tls_failure = |error: rustls::Error| JevError::Transport {
        kind: TransportErrorKind::Other,
        detail: format!("TLS configuration failed: {error}"),
    };
    let mut config = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .map_err(tls_failure)?
    .with_platform_verifier()
    .map_err(tls_failure)?
    .with_no_client_auth();
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    Ok(config)
}

fn transport_error(error: reqwest::Error) -> TransportError {
    let kind = if error.is_timeout() {
        TransportErrorKind::Timeout
    } else if error.is_connect() {
        TransportErrorKind::Connect
    } else {
        TransportErrorKind::Other
    };
    TransportError {
        kind,
        detail: error_chain(&error.without_url()),
    }
}

fn error_chain(error: &dyn std::error::Error) -> String {
    let mut detail = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        detail.push_str(": ");
        detail.push_str(&cause.to_string());
        source = cause.source();
    }
    detail
}
