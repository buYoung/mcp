use super::{DecisionFuture, FailureKind, Policy, Transport};
use reqwest::header::{HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use std::time::Duration;

const ENDPOINT: &str = "https://api.typesafe.ai/v1/systemone";
const MAX_RESPONSE_BYTES: usize = 2_000_000;

/// Construction is inert. Credentials are supplied by the host and never included in Debug.
pub struct HttpsTransport {
    client: reqwest::Client,
    authorization: HeaderValue,
}

impl HttpsTransport {
    pub fn new(api_key: &str, policy: &Policy) -> Result<Self, FailureKind> {
        policy.validate()?;
        if api_key.trim().is_empty() {
            return Err(FailureKind::InvalidInput);
        }
        let mut authorization = HeaderValue::from_str(&format!("Bearer {api_key}"))
            .map_err(|_| FailureKind::InvalidInput)?;
        authorization.set_sensitive(true);
        let client = client_builder(policy)
            .build()
            .map_err(|_| FailureKind::Transport)?;
        Ok(Self {
            client,
            authorization,
        })
    }

    async fn send_to(&self, endpoint: &str, body: Vec<u8>) -> Result<Vec<u8>, FailureKind> {
        let mut response = self
            .client
            .post(endpoint)
            .header(AUTHORIZATION, self.authorization.clone())
            .header(CONTENT_TYPE, "application/json")
            .body(body)
            .send()
            .await
            .map_err(|_| FailureKind::Transport)?;
        if !response.status().is_success() {
            return Err(FailureKind::Http(response.status().as_u16()));
        }
        if response
            .content_length()
            .is_some_and(|size| size > MAX_RESPONSE_BYTES as u64)
        {
            return Err(FailureKind::ResponseTooLarge);
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| FailureKind::Transport)? {
            if bytes.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
                return Err(FailureKind::ResponseTooLarge);
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(bytes)
    }
}

fn client_builder(policy: &Policy) -> reqwest::ClientBuilder {
    reqwest::Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .timeout(Duration::from_millis(policy.timeout_ms))
        .connect_timeout(Duration::from_millis(policy.timeout_ms.min(10_000)))
        .pool_idle_timeout(Duration::from_millis(policy.pool_idle_timeout_ms))
        .pool_max_idle_per_host(policy.max_in_flight_requests)
}

impl Transport for HttpsTransport {
    fn send(&self, body: Vec<u8>) -> DecisionFuture<'_, Result<Vec<u8>, FailureKind>> {
        Box::pin(self.send_to(ENDPOINT, body))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[tokio::test]
    async fn test_idle_connection_reuse_and_expiry_on_loopback() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}/", listener.local_addr().unwrap());
        let connections = Arc::new(AtomicUsize::new(0));
        let count = connections.clone();
        let server = tokio::spawn(async move {
            let mut handlers = tokio::task::JoinSet::new();
            loop {
                let (mut socket, _) = listener.accept().await.unwrap();
                count.fetch_add(1, Ordering::SeqCst);
                handlers.spawn(async move {
                    let mut buffer = [0; 4096];
                    while let Ok(size) = socket.read(&mut buffer).await {
                        if size == 0 {
                            break;
                        }
                        if socket
                            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}")
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                });
            }
        });
        let policy = Policy {
            pool_idle_timeout_ms: 40,
            ..Default::default()
        };
        let transport = HttpsTransport {
            client: client_builder(&policy).https_only(false).build().unwrap(),
            authorization: HeaderValue::from_static("Bearer fixture"),
        };
        for _ in 0..2 {
            assert_eq!(
                transport.send_to(&endpoint, Vec::new()).await.unwrap(),
                b"{}"
            );
        }
        assert_eq!(connections.load(Ordering::SeqCst), 1);
        tokio::time::sleep(Duration::from_millis(100)).await;
        transport.send_to(&endpoint, Vec::new()).await.unwrap();
        assert_eq!(connections.load(Ordering::SeqCst), 2);
        server.abort();
    }
}
