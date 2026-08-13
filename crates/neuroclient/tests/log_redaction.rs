//! Regression tests asserting that credential material and biosignal
//! payloads never reach tracing output, even at TRACE verbosity.

mod support;

use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use neuroclient::retry::{RetryPolicy, with_retry};
use neuroclient::{CortexClient, CortexConfig, CortexError};
use serde_json::{Value, json};
use tracing_subscriber::fmt::MakeWriter;

use support::mock_cortex::MockCortexServer;

const SENTINEL_SECRET: &str = "SENTINEL-CLIENT-SECRET-9f8e7d6c";
const SENTINEL_TOKEN: &str = "SENTINEL-CORTEX-TOKEN-1a2b3c4d5e6f";
const SENTINEL_ERROR: &str = "SENTINEL-SERVER-ERROR-MESSAGE-42";
/// Distinctive EEG sample value that must never appear in logs.
const SENTINEL_EEG: &str = "31337.4242";

/// A `MakeWriter` that captures all tracing output into a shared buffer.
#[derive(Clone, Default)]
struct CaptureWriter(Arc<Mutex<Vec<u8>>>);

impl CaptureWriter {
    fn contents(&self) -> String {
        let buf = self.0.lock().unwrap();
        String::from_utf8_lossy(&buf).into_owned()
    }
}

impl Write for CaptureWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for CaptureWriter {
    type Writer = CaptureWriter;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

fn test_config(url: String) -> CortexConfig {
    let mut config = CortexConfig::new("test-client-id", SENTINEL_SECRET);
    config.cortex_url = url;
    config.timeouts.rpc_timeout_secs = 2;
    config
}

fn rpc_id(request: &Value) -> u64 {
    request
        .get("id")
        .and_then(Value::as_u64)
        .expect("request missing numeric id")
}

/// Full mock round trip (authenticate, error response, raw EEG frame)
/// under a TRACE-level subscriber; asserts no sensitive material leaks.
#[tokio::test]
async fn tracing_output_never_contains_secrets_tokens_or_samples() {
    let capture = CaptureWriter::default();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_writer(capture.clone())
        .with_ansi(false)
        .finish();
    // Thread-local default: tokio::test runs a current-thread runtime, so
    // the client's reader task logs are captured too.
    let _guard = tracing::subscriber::set_default(subscriber);

    let mut server = match MockCortexServer::start().await {
        Ok(server) => server,
        Err(err) => {
            eprintln!("Skipping log redaction test: mock server unavailable: {err}");
            return;
        }
    };
    let config = test_config(server.ws_url());
    let mut client = CortexClient::connect(&config).await.unwrap();

    let mut connection = server.accept_connection().await;
    let responder = tokio::spawn(async move {
        // getCortexInfo
        let request = connection.recv_request().await;
        connection
            .send_result(rpc_id(&request), json!({"version": "mock-1.0.0"}))
            .await;

        // requestAccess
        let request = connection.recv_request().await;
        connection
            .send_result(rpc_id(&request), json!({"accessGranted": true}))
            .await;

        // authorize — returns the sentinel token
        let request = connection.recv_request().await;
        let sent_secret = request["params"]["clientSecret"]
            .as_str()
            .unwrap()
            .to_string();
        connection
            .send_result(rpc_id(&request), json!({"cortexToken": SENTINEL_TOKEN}))
            .await;

        // Push a raw EEG-style stream frame containing the sentinel sample,
        // then answer one more RPC so the test can synchronize on ordering.
        connection
            .push_event(json!({
                "sid": "session-1",
                "time": 1_234.5,
                "eeg": [12, 0, SENTINEL_EEG.parse::<f64>().unwrap(), 4.2, 4.3, 4.4, 4.5, 0.0, 0, []],
            }))
            .await;

        let request = connection.recv_request().await;
        connection
            .send_error(rpc_id(&request), -32999, SENTINEL_ERROR)
            .await;

        sent_secret
    });

    let token = client
        .authenticate(&config.client_id, &config.client_secret)
        .await
        .unwrap();
    assert_eq!(token, SENTINEL_TOKEN);

    // This RPC is answered *after* the EEG frame was sent on the same
    // socket, so once it resolves the frame has been processed and logged.
    let err = client.get_cortex_info().await.unwrap_err();
    let sent_secret = responder.await.unwrap();

    // Sanity: the secret really went over the wire, and the error text is
    // still available to the caller.
    assert_eq!(sent_secret, SENTINEL_SECRET);
    assert!(
        err.to_string().contains(SENTINEL_ERROR),
        "error text must still reach the caller: {err}"
    );

    let retry_policy = RetryPolicy::Backoff {
        max_retries: 1,
        base_delay: Duration::ZERO,
        max_delay: Duration::ZERO,
    };
    let retry_err = with_retry(&retry_policy, || async {
        Err::<(), _>(CortexError::ConnectionLost {
            reason: SENTINEL_ERROR.into(),
        })
    })
    .await
    .unwrap_err();
    assert!(retry_err.to_string().contains(SENTINEL_ERROR));

    client.disconnect().await.unwrap();

    let logs = capture.contents();
    assert!(
        logs.contains("Sending Cortex request"),
        "expected debug logging to be active; captured:\n{logs}"
    );
    for sentinel in [
        SENTINEL_SECRET,
        SENTINEL_TOKEN,
        SENTINEL_EEG,
        SENTINEL_ERROR,
    ] {
        assert!(
            !logs.contains(sentinel),
            "sensitive value {sentinel:?} leaked into tracing output:\n{logs}"
        );
    }
}

#[tokio::test]
async fn authentication_preflight_error_text_is_redacted() {
    let capture = CaptureWriter::default();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_writer(capture.clone())
        .with_ansi(false)
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);

    let mut server = match MockCortexServer::start().await {
        Ok(server) => server,
        Err(err) => {
            eprintln!("Skipping auth redaction test: mock server unavailable: {err}");
            return;
        }
    };
    let config = test_config(server.ws_url());
    let client = CortexClient::connect(&config).await.unwrap();
    let mut connection = server.accept_connection().await;

    let responder = tokio::spawn(async move {
        let info = connection.recv_request().await;
        connection
            .send_error(rpc_id(&info), -32_999, SENTINEL_ERROR)
            .await;

        let access = connection.recv_request().await;
        connection
            .send_result(rpc_id(&access), json!({"accessGranted": true}))
            .await;

        let authorize = connection.recv_request().await;
        connection
            .send_result(rpc_id(&authorize), json!({"cortexToken": SENTINEL_TOKEN}))
            .await;
    });

    let token = client
        .authenticate(&config.client_id, &config.client_secret)
        .await
        .unwrap();
    assert_eq!(token, SENTINEL_TOKEN);
    responder.await.unwrap();
    client.shutdown().await;

    let logs = capture.contents();
    assert!(logs.contains("getCortexInfo failed"));
    assert!(logs.contains("api_error"));
    assert!(logs.contains("-32999"));
    assert!(!logs.contains(SENTINEL_ERROR));
}

#[tokio::test]
async fn malformed_warning_diagnostics_do_not_echo_payloads() {
    let capture = CaptureWriter::default();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_writer(capture.clone())
        .with_ansi(false)
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);

    let mut server = match MockCortexServer::start().await {
        Ok(server) => server,
        Err(err) => {
            eprintln!("Skipping warning redaction test: mock server unavailable: {err}");
            return;
        }
    };
    let config = test_config(server.ws_url());
    let client = CortexClient::connect(&config).await.unwrap();
    let mut connection = server.accept_connection().await;

    connection
        .push_event(json!({
            "warning": {
                "code": SENTINEL_ERROR,
                "message": SENTINEL_ERROR,
            }
        }))
        .await;
    let responder = tokio::spawn(async move {
        let request = connection.recv_request().await;
        connection
            .send_result(rpc_id(&request), json!({"version": "mock"}))
            .await;
    });

    client.get_cortex_info().await.unwrap();
    responder.await.unwrap();
    client.shutdown().await;

    let logs = capture.contents();
    assert!(logs.contains("Failed to parse Cortex warning object"));
    assert!(!logs.contains(SENTINEL_ERROR));
}
