//! Lifecycle tests: shutdown/drop must terminate the reader task, close
//! the socket, fail pending RPCs, and close stream channels — never hang
//! or leak connections.

mod support;

use std::sync::Arc;
use std::time::Duration;

use neuroclient::protocol::constants::Streams;
use neuroclient::{CortexClient, CortexConfig};
use tokio::time::timeout;

use support::mock_cortex::MockCortexServer;

const STEP: Duration = Duration::from_secs(3);

fn test_config(url: String) -> CortexConfig {
    let mut config = CortexConfig::new("test-client-id", "test-client-secret");
    config.cortex_url = url;
    config.timeouts.rpc_timeout_secs = 30; // longer than the test: proves shutdown unblocks RPCs
    config
}

async fn start_server_or_skip(test_name: &str) -> Option<MockCortexServer> {
    match MockCortexServer::start().await {
        Ok(server) => Some(server),
        Err(err) => {
            eprintln!("Skipping {test_name}: unable to start mock server: {err}");
            None
        }
    }
}

#[tokio::test]
async fn shutdown_closes_socket_streams_and_pending_rpcs() {
    let Some(mut server) =
        start_server_or_skip("shutdown_closes_socket_streams_and_pending_rpcs").await
    else {
        return;
    };
    let config = test_config(server.ws_url());
    let client = Arc::new(CortexClient::connect(&config).await.unwrap());
    let mut connection = server.accept_connection().await;

    // A stream channel that must be closed by shutdown.
    let mut eeg_rx = client
        .add_stream_channel("session-1", Streams::EEG)
        .unwrap();

    // An in-flight RPC that the server never answers.
    let pending_client = Arc::clone(&client);
    let pending_call = tokio::spawn(async move { pending_client.get_cortex_info().await });
    // Ensure the request reached the server before shutting down.
    let _ = timeout(STEP, connection.recv_request()).await.unwrap();

    assert!(client.is_connected());
    timeout(STEP, client.shutdown())
        .await
        .expect("shutdown must be bounded");
    assert!(!client.is_connected());

    // Pending RPC fails promptly instead of waiting out its 30s timeout.
    let result = timeout(STEP, pending_call)
        .await
        .expect("pending RPC must resolve on shutdown")
        .unwrap();
    assert!(result.is_err(), "pending RPC must fail on shutdown");

    // Stream channel is closed.
    let next = timeout(STEP, eeg_rx.recv())
        .await
        .expect("stream receiver must resolve on shutdown");
    assert!(next.is_none(), "stream channel must close on shutdown");

    // Server observes the close.
    connection.wait_for_close().await;

    // Idempotent: a second shutdown returns immediately.
    timeout(STEP, client.shutdown())
        .await
        .expect("repeat shutdown must be a no-op");
}

#[tokio::test]
async fn drop_aborts_reader_and_closes_socket() {
    let Some(mut server) = start_server_or_skip("drop_aborts_reader_and_closes_socket").await
    else {
        return;
    };
    let config = test_config(server.ws_url());

    let mut eeg_rx = {
        let client = CortexClient::connect(&config).await.unwrap();
        client
            .add_stream_channel("session-1", Streams::EEG)
            .unwrap()
        // client dropped here without disconnect()
    };
    let mut connection = server.accept_connection().await;

    // Dropping the client must close the socket and the stream channel.
    connection.wait_for_close().await;
    let next = timeout(STEP, eeg_rx.recv())
        .await
        .expect("stream receiver must resolve after drop");
    assert!(next.is_none(), "stream channel must close after drop");
}
