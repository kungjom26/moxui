//! Webhook delivery with exponential backoff retry.
//!
//! Handles HTTP POST delivery to a single endpoint, with HMAC-SHA256
//! payload signing when a secret is configured.

use std::time::Duration;

use hmac::{Hmac, Mac};
use sha2::Sha256;
use tracing::{error, info, warn};

use super::WebhookEndpoint;

/// Deliver a webhook payload with retry logic and exponential backoff.
///
/// Retries up to `max_retries` times with increasing delays:
/// - Delay 0: immediate
/// - Delay 1: 1s
/// - Delay 2: 2s
/// - Delay 3: 4s
/// - Delay n: 2^(n-1) seconds
pub async fn deliver_with_retry(
    endpoint: &WebhookEndpoint,
    payload: &serde_json::Value,
    timeout: Duration,
    max_retries: u32,
) {
    let body = serde_json::to_string(payload).unwrap_or_default();
    let url = endpoint.url.clone();
    let name = endpoint.name.clone();

    for attempt in 0..=max_retries {
        if attempt > 0 {
            let delay = Duration::from_secs(1 << (attempt - 1)); // 1, 2, 4, 8...
            info!(
                endpoint = %name,
                attempt = attempt,
                max_retries = max_retries,
                delay_ms = delay.as_millis(),
                "Retrying webhook delivery"
            );
            tokio::time::sleep(delay).await;
        }

        match deliver_once(endpoint, &body, timeout).await {
            Ok(()) => {
                info!(
                    endpoint = %name,
                    url = %url,
                    attempt = attempt,
                    "Webhook delivered successfully"
                );
                return;
            }
            Err(e) => {
                warn!(
                    endpoint = %name,
                    url = %url,
                    attempt = attempt,
                    error = %e,
                    "Webhook delivery failed"
                );
            }
        }
    }

    error!(
        endpoint = %name,
        url = %url,
        max_retries = max_retries,
        "Webhook delivery exhausted all retries"
    );
}

/// Deliver a single webhook POST to the endpoint.
async fn deliver_once(
    endpoint: &WebhookEndpoint,
    body: &str,
    timeout: Duration,
) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {e}"))?;

    let mut req = client.post(&endpoint.url).header("Content-Type", "application/json");

    // Add HMAC-SHA256 signature header if secret is configured
    if let Some(ref secret) = endpoint.secret {
        if let Ok(signature) = sign_payload(body, secret) {
            req = req.header("X-Webhook-Signature", &signature);
        }
    }

    let resp = req.body(body.to_string()).send().await.map_err(|e| format!("HTTP request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let resp_body = resp.text().await.unwrap_or_default();
        return Err(format!("HTTP {status}: {resp_body}"));
    }

    Ok(())
}

/// Compute HMAC-SHA256 signature of the payload.
fn sign_payload(body: &str, secret: &str) -> Result<String, String> {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
        .map_err(|e| format!("HMAC init failed: {e}"))?;
    mac.update(body.as_bytes());
    let result = mac.finalize();
    Ok(hex::encode(result.into_bytes()))
}
