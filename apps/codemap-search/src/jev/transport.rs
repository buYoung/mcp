//! HTTP boundary. Authentication lives here and only here: the API key is never part of
//! the model state, the request body, `Debug` output or error text.
//!
//! Client policy (all fixed, none configurable through MCP):
//! - no redirects are followed (`reqwest::redirect::Policy::none()`), so neither the
//!   credential nor the evidence can be re-sent to another location;
//! - no client-side retries (`reqwest::retry::never()`), including the library default of
//!   re-sending requests after a protocol NACK or a stale pooled connection: every POST is
//!   one billed attempt with one request identity;
//! - HTTP/1.1 only, one JSON body per batch, `Content-Type: application/json`,
//!   `Authorization: Bearer`, rustls with the `ring` provider and the OS trust store;
//! - response bodies are read up to [`MAX_RESPONSE_BODY_BYTES`] and abandoned beyond it.

use super::{BoxFuture, JevError, ENDPOINT, MAX_RESPONSE_BODY_BYTES};
use std::time::{Duration, Instant};

/// A credential that never prints. `expose` is the only way to read it.
#[derive(Clone)]
pub struct SecretString(String);

impl SecretString {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for SecretString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SecretString([REDACTED])")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransportError {
    /// The HTTP client could not be built (TLS or configuration).
    Client(String),
    Connect(String),
    /// The per-request timeout (the remaining whole-call deadline) elapsed.
    Timeout,
    /// The response body exceeded [`MAX_RESPONSE_BODY_BYTES`]; the read was abandoned.
    ResponseTooLarge {
        limit: usize,
    },
    Io(String),
}

impl std::fmt::Display for TransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Client(reason) => write!(f, "client setup failed: {reason}"),
            Self::Connect(reason) => write!(f, "connection failed: {reason}"),
            Self::Timeout => write!(f, "request timed out"),
            Self::ResponseTooLarge { limit } => {
                write!(f, "response body exceeds the {limit}-byte limit")
            }
            Self::Io(reason) => write!(f, "request failed: {reason}"),
        }
    }
}

impl std::error::Error for TransportError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransportResponse {
    pub status: u16,
    pub body: Vec<u8>,
    pub elapsed: Duration,
}

/// One POST of an already-encoded batch body. Implementations must not retry: the
/// evaluator owns request identity and treats every attempt as a billed request.
pub trait Transport: Send + Sync {
    fn post(
        &self,
        body: Vec<u8>,
        timeout: Duration,
    ) -> BoxFuture<'_, Result<TransportResponse, TransportError>>;
}

/// Connection-level settings for [`HttpsTransport`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpsSettings {
    /// Idle lifetime after which a pooled connection is dropped instead of reused. The
    /// provider closed connections idle for about 30 s in the PoC; reqwest's pool applies
    /// this bound and opens a fresh connection for the next request.
    pub pool_idle_timeout: Duration,
    /// Idle connections kept per host; matches the in-flight request ceiling.
    pub max_idle_connections: usize,
    pub connect_timeout: Duration,
}

impl Default for HttpsSettings {
    fn default() -> Self {
        Self {
            pool_idle_timeout: Duration::from_millis(30_000),
            max_idle_connections: super::MAX_IN_FLIGHT_REQUESTS,
            connect_timeout: Duration::from_millis(10_000),
        }
    }
}

impl HttpsSettings {
    pub fn validate(&self) -> Result<(), JevError> {
        for (name, duration) in [
            ("pool_idle_timeout", self.pool_idle_timeout),
            ("connect_timeout", self.connect_timeout),
        ] {
            if !super::evaluator::is_safe_duration(duration) {
                return Err(JevError::InvalidConfig(format!(
                    "{name} must be positive and within the monotonic clock range"
                )));
            }
        }
        if self.max_idle_connections == 0 {
            return Err(JevError::InvalidConfig(
                "max_idle_connections must be at least 1".into(),
            ));
        }
        Ok(())
    }
}

/// Asynchronous HTTPS transport to [`ENDPOINT`] over rustls (ring provider, OS trust
/// store). Connections are pooled and reused within `pool_idle_timeout`; POSTs are never
/// retried and redirects are never followed.
pub struct HttpsTransport {
    client: reqwest::Client,
    api_key: SecretString,
    endpoint: String,
}

impl std::fmt::Debug for HttpsTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpsTransport")
            .field("endpoint", &self.endpoint)
            .field("api_key", &self.api_key)
            .finish()
    }
}

impl HttpsTransport {
    pub fn new(api_key: SecretString, settings: HttpsSettings) -> Result<Self, JevError> {
        Self::build(api_key, settings, ENDPOINT.to_string())
    }

    /// Test-only: the same client policy against a local listener, so redirect, retry,
    /// pooling and body-limit behavior can be observed on real sockets without any external
    /// endpoint. Not compiled into the library or binary.
    #[cfg(test)]
    pub(super) fn with_endpoint_for_tests(
        api_key: SecretString,
        settings: HttpsSettings,
        endpoint: String,
    ) -> Result<Self, JevError> {
        Self::build(api_key, settings, endpoint)
    }

    fn build(
        api_key: SecretString,
        settings: HttpsSettings,
        endpoint: String,
    ) -> Result<Self, JevError> {
        if api_key.expose().trim().is_empty() {
            return Err(JevError::InvalidConfig("API key is empty".into()));
        }
        settings.validate()?;
        // `rustls` is built with only the ring provider; installing it explicitly keeps the
        // process default independent of any other rustls user linked later.
        let _ = rustls::crypto::ring::default_provider().install_default();
        let client = reqwest::Client::builder()
            .pool_idle_timeout(settings.pool_idle_timeout)
            .pool_max_idle_per_host(settings.max_idle_connections)
            .connect_timeout(settings.connect_timeout)
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .user_agent(concat!("codemap-search/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|error| JevError::Transport(TransportError::Client(error.to_string())))?;
        Ok(Self {
            client,
            api_key,
            endpoint,
        })
    }

    #[cfg(test)]
    pub(super) fn endpoint(&self) -> &str {
        &self.endpoint
    }
}

fn map_reqwest_error(error: reqwest::Error) -> TransportError {
    if error.is_timeout() {
        TransportError::Timeout
    } else if error.is_connect() {
        TransportError::Connect(error.to_string())
    } else {
        TransportError::Io(error.to_string())
    }
}

impl Transport for HttpsTransport {
    fn post(
        &self,
        body: Vec<u8>,
        timeout: Duration,
    ) -> BoxFuture<'_, Result<TransportResponse, TransportError>> {
        Box::pin(async move {
            let started = Instant::now();
            let mut response = self
                .client
                .post(&self.endpoint)
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .bearer_auth(self.api_key.expose())
                .timeout(timeout)
                .body(body)
                .send()
                .await
                .map_err(map_reqwest_error)?;
            let status = response.status().as_u16();
            if response
                .content_length()
                .is_some_and(|length| length > MAX_RESPONSE_BODY_BYTES as u64)
            {
                return Err(TransportError::ResponseTooLarge {
                    limit: MAX_RESPONSE_BODY_BYTES,
                });
            }
            // Read chunk by chunk so an unbounded or lying body never grows past the limit.
            let mut collected = Vec::new();
            while let Some(chunk) = response.chunk().await.map_err(map_reqwest_error)? {
                if collected.len() + chunk.len() > MAX_RESPONSE_BODY_BYTES {
                    return Err(TransportError::ResponseTooLarge {
                        limit: MAX_RESPONSE_BODY_BYTES,
                    });
                }
                collected.extend_from_slice(&chunk);
            }
            Ok(TransportResponse {
                status,
                body: collected,
                elapsed: started.elapsed(),
            })
        })
    }
}
