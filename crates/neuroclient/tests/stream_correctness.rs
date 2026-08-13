//! Stream correctness tests: typed subscription outcomes (H2), warning
//! handling (H3), and session-aware routing (H4).

mod support;

use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use neuroclient::protocol::constants::{Methods, Streams};
use neuroclient::protocol::warnings::WarningCodes;
use neuroclient::{CortexClient, CortexConfig, CortexError, streams};
use serde_json::{Value, json};
use tokio::time::timeout;

use support::mock_cortex::MockCortexServer;

const STEP: Duration = Duration::from_secs(3);

fn test_config(url: String) -> CortexConfig {
    let mut config = CortexConfig::new("test-client-id", "test-client-secret");
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

async fn start_server_or_skip(test_name: &str) -> Option<MockCortexServer> {
    match MockCortexServer::start().await {
        Ok(server) => Some(server),
        Err(err) => {
            eprintln!("Skipping {test_name}: unable to start mock server: {err}");
            None
        }
    }
}

fn eeg_event(sid: &str, marker: f64) -> Value {
    json!({
        "sid": sid,
        "time": 1_609_459_200.0,
        "eeg": [1, 0, marker, 2.0, 3.0, 4.0, 5.0, 0.0, 0, []],
    })
}

fn mot_event(sid: &str, marker: f64) -> Value {
    // 12-element layout: [counter, interpolated, q0..q3, ax, ay, az, mx, my, mz]
    json!({
        "sid": sid,
        "time": 1_609_459_200.0,
        "mot": [33, 0, 0.7, 0.5, 0.5, 0.25, marker, 2.0, 3.0, 10.0, 20.0, 30.0],
    })
}

// ─── H2: typed subscription outcomes ─────────────────────────────────────

#[tokio::test]
async fn subscription_failure_returns_error_and_rolls_back_route() {
    let Some(mut server) =
        start_server_or_skip("subscription_failure_returns_error_and_rolls_back_route").await
    else {
        return;
    };
    let config = test_config(server.ws_url());
    let client = CortexClient::connect(&config).await.unwrap();
    let mut connection = server.accept_connection().await;

    let responder = tokio::spawn(async move {
        // Total failure: the requested stream is only in `failure`.
        let request = connection.recv_request_method(Methods::SUBSCRIBE).await;
        connection
            .send_result(
                rpc_id(&request),
                json!({
                    "success": [],
                    "failure": [{
                        "streamName": "eeg",
                        "code": -32602,
                        "message": "The stream eeg is not available",
                    }],
                }),
            )
            .await;

        // Second subscribe succeeds — proves the failed route was rolled
        // back (otherwise the duplicate-route check would reject it).
        let request = connection.recv_request_method(Methods::SUBSCRIBE).await;
        connection
            .send_result(
                rpc_id(&request),
                json!({
                    "success": [{"streamName": "eeg", "cols": [], "sid": "session-1"}],
                    "failure": [],
                }),
            )
            .await;
        connection.push_event(eeg_event("session-1", 42.0)).await;
        connection
    });

    let Err(err) = streams::subscribe_eeg(&client, "token", "session-1", 5).await else {
        panic!("subscribe must fail when the stream is in the failure array");
    };
    assert!(
        matches!(err, CortexError::StreamError { .. }),
        "expected StreamError, got {err:?}"
    );
    assert!(err.to_string().contains("eeg"), "unexpected error: {err}");

    // Route was rolled back: dispatch stats hold no entry for it.
    assert!(client.stream_dispatch_stats().is_empty());

    // Retry succeeds and receives data.
    let mut eeg = streams::subscribe_eeg(&client, "token", "session-1", 5)
        .await
        .unwrap();
    let sample = timeout(STEP, eeg.next())
        .await
        .expect("timed out waiting for EEG sample")
        .expect("stream ended unexpectedly");
    assert!((f64::from(sample.channels[0]) - 42.0).abs() < 1e-6);

    let _ = responder.await.unwrap();
}

#[tokio::test]
async fn partial_subscription_failure_is_reported_per_stream() {
    let Some(mut server) =
        start_server_or_skip("partial_subscription_failure_is_reported_per_stream").await
    else {
        return;
    };
    let config = test_config(server.ws_url());
    let client = CortexClient::connect(&config).await.unwrap();
    let mut connection = server.accept_connection().await;

    let responder = tokio::spawn(async move {
        let request = connection.recv_request_method(Methods::SUBSCRIBE).await;
        connection
            .send_result(
                rpc_id(&request),
                json!({
                    "success": [{"streamName": "mot", "cols": [], "sid": "session-1"}],
                    "failure": [{
                        "streamName": "eeg",
                        "code": -32602,
                        "message": "denied",
                    }],
                }),
            )
            .await;
    });

    // Raw multi-stream call returns the typed partial result.
    let result = client
        .subscribe_streams("token", "session-1", &[Streams::MOT, Streams::EEG])
        .await
        .unwrap();
    assert!(result.confirms(Streams::MOT));
    assert!(!result.confirms(Streams::EEG));
    let failure = result.failure_for(Streams::EEG).expect("missing failure");
    assert_eq!(failure.code, -32602);

    responder.await.unwrap();
}

#[tokio::test]
async fn partial_unsubscribe_removes_only_confirmed_routes() {
    let Some(mut server) =
        start_server_or_skip("partial_unsubscribe_removes_only_confirmed_routes").await
    else {
        return;
    };
    let config = test_config(server.ws_url());
    let client = CortexClient::connect(&config).await.unwrap();
    let mut connection = server.accept_connection().await;

    let mut eeg_rx = client
        .add_stream_channel("session-1", Streams::EEG)
        .unwrap();
    let mut mot_rx = client
        .add_stream_channel("session-1", Streams::MOT)
        .unwrap();

    let responder = tokio::spawn(async move {
        let request = connection.recv_request_method(Methods::UNSUBSCRIBE).await;
        connection
            .send_result(
                rpc_id(&request),
                json!({
                    "success": [{
                        "streamName": "mot",
                        "cols": [],
                        "sid": "session-1",
                    }],
                    "failure": [{
                        "streamName": "eeg",
                        "code": -32602,
                        "message": "still subscribed",
                    }],
                }),
            )
            .await;
        connection
    });

    let err = streams::unsubscribe(&client, "token", "session-1", &[Streams::EEG, Streams::MOT])
        .await
        .unwrap_err();
    assert!(matches!(err, CortexError::StreamError { .. }));

    let connection = responder.await.unwrap();
    let closed = timeout(STEP, mot_rx.recv())
        .await
        .expect("confirmed motion unsubscribe must close its local route");
    assert!(closed.is_none());

    connection.push_event(eeg_event("session-1", 8.0)).await;
    let eeg = timeout(STEP, eeg_rx.recv())
        .await
        .expect("failed EEG unsubscribe must preserve its local route")
        .expect("EEG route closed after failed unsubscribe");
    assert_eq!(eeg["sid"], "session-1");
}

#[tokio::test]
async fn contradictory_unsubscribe_failure_preserves_route() {
    let Some(mut server) =
        start_server_or_skip("contradictory_unsubscribe_failure_preserves_route").await
    else {
        return;
    };
    let config = test_config(server.ws_url());
    let client = CortexClient::connect(&config).await.unwrap();
    let mut connection = server.accept_connection().await;
    let mut eeg_rx = client
        .add_stream_channel("session-1", Streams::EEG)
        .unwrap();

    let responder = tokio::spawn(async move {
        let request = connection.recv_request_method(Methods::UNSUBSCRIBE).await;
        connection
            .send_result(
                rpc_id(&request),
                json!({
                    "success": [{
                        "streamName": "eeg",
                        "cols": [],
                        "sid": "session-1",
                    }],
                    "failure": [{
                        "streamName": "eeg",
                        "code": -32602,
                        "message": "failure takes precedence",
                    }],
                }),
            )
            .await;
        connection
    });

    let err = streams::unsubscribe(&client, "token", "session-1", &[Streams::EEG])
        .await
        .unwrap_err();
    assert!(matches!(err, CortexError::StreamError { .. }));

    let connection = responder.await.unwrap();
    connection.push_event(eeg_event("session-1", 9.0)).await;
    let event = timeout(STEP, eeg_rx.recv())
        .await
        .expect("contradictory failure must retain route")
        .expect("route closed despite failure");
    assert_eq!(event["sid"], "session-1");
}

#[tokio::test]
async fn stale_unsubscribe_response_cannot_remove_newer_route() {
    let Some(mut server) =
        start_server_or_skip("stale_unsubscribe_response_cannot_remove_newer_route").await
    else {
        return;
    };
    let config = test_config(server.ws_url());
    let client = Arc::new(CortexClient::connect(&config).await.unwrap());
    let mut connection = server.accept_connection().await;
    let mut old_rx = client
        .add_stream_channel("session-1", Streams::EEG)
        .unwrap();

    let unsubscribe = tokio::spawn({
        let client = Arc::clone(&client);
        async move { streams::unsubscribe(&client, "token", "session-1", &[Streams::EEG]).await }
    });
    let request = connection.recv_request_method(Methods::UNSUBSCRIBE).await;

    client.remove_stream_channel("session-1", Streams::EEG);
    let mut new_rx = client
        .add_stream_channel("session-1", Streams::EEG)
        .unwrap();

    connection
        .send_result(
            rpc_id(&request),
            json!({
                "success": [{
                    "streamName": "eeg",
                    "cols": [],
                    "sid": "session-1",
                }],
                "failure": [],
            }),
        )
        .await;
    unsubscribe.await.unwrap().unwrap();

    assert!(old_rx.recv().await.is_none());
    connection.push_event(eeg_event("session-1", 10.0)).await;
    let event = timeout(STEP, new_rx.recv())
        .await
        .expect("newer route did not receive sample")
        .expect("stale unsubscribe removed newer route");
    assert_eq!(event["sid"], "session-1");
}

#[tokio::test]
async fn rpc_error_during_subscribe_rolls_back_route() {
    let Some(mut server) =
        start_server_or_skip("rpc_error_during_subscribe_rolls_back_route").await
    else {
        return;
    };
    let config = test_config(server.ws_url());
    let client = CortexClient::connect(&config).await.unwrap();
    let mut connection = server.accept_connection().await;

    let responder = tokio::spawn(async move {
        let request = connection.recv_request_method(Methods::SUBSCRIBE).await;
        connection
            .send_error(rpc_id(&request), -32014, "Session does not exist")
            .await;
    });

    let Err(err) = streams::subscribe_motion(&client, "token", "session-x").await else {
        panic!("subscribe must fail when the RPC returns an error");
    };
    assert!(!matches!(err, CortexError::StreamError { .. }));
    assert!(client.stream_dispatch_stats().is_empty());

    responder.await.unwrap();
}

#[tokio::test]
async fn samples_arriving_before_subscribe_response_are_not_lost() {
    let Some(mut server) =
        start_server_or_skip("samples_arriving_before_subscribe_response_are_not_lost").await
    else {
        return;
    };
    let config = test_config(server.ws_url());
    let client = CortexClient::connect(&config).await.unwrap();
    let mut connection = server.accept_connection().await;

    let responder = tokio::spawn(async move {
        let request = connection.recv_request_method(Methods::SUBSCRIBE).await;
        // Sample hits the wire *before* the subscribe response: the route
        // must already be reserved so this first sample is captured.
        connection.push_event(mot_event("session-1", 7.0)).await;
        connection
            .send_result(
                rpc_id(&request),
                json!({
                    "success": [{"streamName": "mot", "cols": [], "sid": "session-1"}],
                    "failure": [],
                }),
            )
            .await;
    });

    let mut motion = streams::subscribe_motion(&client, "token", "session-1")
        .await
        .unwrap();
    let sample = timeout(STEP, motion.next())
        .await
        .expect("timed out waiting for early motion sample")
        .expect("stream ended unexpectedly");
    assert!((sample.accelerometer[0] - 7.0).abs() < 1e-6);

    responder.await.unwrap();
}

#[tokio::test]
async fn duplicate_local_route_is_rejected() {
    let Some(server) = start_server_or_skip("duplicate_local_route_is_rejected").await else {
        return;
    };
    let config = test_config(server.ws_url());
    let client = CortexClient::connect(&config).await.unwrap();

    let _rx = client
        .add_stream_channel("session-1", Streams::EEG)
        .unwrap();
    let err = client
        .add_stream_channel("session-1", Streams::EEG)
        .unwrap_err();
    assert!(
        matches!(err, CortexError::StreamError { .. }),
        "expected StreamError, got {err:?}"
    );

    // A different session may register the same stream type.
    assert!(client.add_stream_channel("session-2", Streams::EEG).is_ok());
}

#[tokio::test]
async fn canceled_subscription_releases_its_reserved_route() {
    let Some(mut server) =
        start_server_or_skip("canceled_subscription_releases_its_reserved_route").await
    else {
        return;
    };
    let config = test_config(server.ws_url());
    let client = Arc::new(CortexClient::connect(&config).await.unwrap());
    let mut connection = server.accept_connection().await;

    let canceled = tokio::spawn({
        let client = Arc::clone(&client);
        async move { streams::subscribe_motion(&client, "token", "session-1").await }
    });
    let first_request = connection.recv_request_method(Methods::SUBSCRIBE).await;

    canceled.abort();
    let Err(join_error) = canceled.await else {
        panic!("aborted subscription task completed successfully");
    };
    assert!(join_error.is_cancelled());
    tokio::task::yield_now().await;
    assert!(client.stream_dispatch_stats().is_empty());

    // Clear the canceled RPC response, then prove that the same route can be
    // reserved and committed by a later subscription.
    connection
        .send_error(rpc_id(&first_request), -32014, "canceled")
        .await;
    let retry = tokio::spawn({
        let client = Arc::clone(&client);
        async move { streams::subscribe_motion(&client, "token", "session-1").await }
    });
    let retry_request = connection.recv_request_method(Methods::SUBSCRIBE).await;
    connection
        .send_result(
            rpc_id(&retry_request),
            json!({
                "success": [{"streamName": "mot", "cols": [], "sid": "session-1"}],
                "failure": [],
            }),
        )
        .await;

    let _motion = retry.await.unwrap().unwrap();
    assert_eq!(client.stream_dispatch_stats().len(), 1);
}

#[tokio::test]
async fn stale_subscription_rollback_cannot_remove_newer_route() {
    let Some(mut server) =
        start_server_or_skip("stale_subscription_rollback_cannot_remove_newer_route").await
    else {
        return;
    };
    let config = test_config(server.ws_url());
    let client = Arc::new(CortexClient::connect(&config).await.unwrap());
    let mut connection = server.accept_connection().await;
    let mut warnings = client.warning_receiver();

    let stale = tokio::spawn({
        let client = Arc::clone(&client);
        async move { streams::subscribe_motion(&client, "token-a", "session-1").await }
    });
    let stale_request = connection.recv_request_method(Methods::SUBSCRIBE).await;

    connection
        .push_event(json!({
            "warning": {
                "code": WarningCodes::SESSION_AUTO_CLOSED,
                "message": {
                    "behavior": "The session was closed",
                    "sessionId": "session-1",
                },
            }
        }))
        .await;
    timeout(STEP, warnings.recv())
        .await
        .expect("warning was not dispatched")
        .expect("warning channel closed");

    let current = tokio::spawn({
        let client = Arc::clone(&client);
        async move { streams::subscribe_motion(&client, "token-b", "session-1").await }
    });
    let current_request = connection.recv_request_method(Methods::SUBSCRIBE).await;

    // The stale future resumes after the new route exists. Its reservation
    // guard must compare sender identity before rolling back.
    connection
        .send_error(rpc_id(&stale_request), -32014, "stale request failed")
        .await;
    assert!(stale.await.unwrap().is_err());

    connection
        .send_result(
            rpc_id(&current_request),
            json!({
                "success": [{"streamName": "mot", "cols": [], "sid": "session-1"}],
                "failure": [],
            }),
        )
        .await;
    let mut motion = current.await.unwrap().unwrap();

    connection.push_event(mot_event("session-1", 6.0)).await;
    let sample = timeout(STEP, motion.next())
        .await
        .expect("timed out waiting on newer route")
        .expect("newer route was removed by stale rollback");
    assert!((sample.accelerometer[0] - 6.0).abs() < 1e-6);
}

// ─── H4: session-aware routing ───────────────────────────────────────────

#[tokio::test]
async fn bulk_channel_creation_preserves_other_sessions_and_is_atomic() {
    let Some(mut server) =
        start_server_or_skip("bulk_channel_creation_preserves_other_sessions_and_is_atomic").await
    else {
        return;
    };
    let config = test_config(server.ws_url());
    let client = CortexClient::connect(&config).await.unwrap();
    let connection = server.accept_connection().await;

    let mut session_1 = client
        .create_stream_channels("session-1", &[Streams::EEG])
        .unwrap()
        .remove(Streams::EEG)
        .unwrap();
    let mut session_2 = client
        .create_stream_channels("session-2", &[Streams::EEG])
        .unwrap()
        .remove(Streams::EEG)
        .unwrap();

    let duplicate = client.create_stream_channels("session-1", &[Streams::EEG, Streams::MOT]);
    assert!(matches!(duplicate, Err(CortexError::StreamError { .. })));
    assert_eq!(
        client.stream_dispatch_stats().len(),
        2,
        "failed bulk insert must not add any routes"
    );

    connection.push_event(eeg_event("session-1", 1.0)).await;
    connection.push_event(eeg_event("session-2", 2.0)).await;
    assert_eq!(
        timeout(STEP, session_1.recv()).await.unwrap().unwrap()["sid"],
        "session-1"
    );
    assert_eq!(
        timeout(STEP, session_2.recv()).await.unwrap().unwrap()["sid"],
        "session-2"
    );
}

#[tokio::test]
async fn concurrent_sessions_with_same_stream_stay_isolated() {
    let Some(mut server) =
        start_server_or_skip("concurrent_sessions_with_same_stream_stay_isolated").await
    else {
        return;
    };
    let config = test_config(server.ws_url());
    let client = CortexClient::connect(&config).await.unwrap();
    let connection = server.accept_connection().await;

    let mut rx1 = client
        .add_stream_channel("session-1", Streams::MOT)
        .unwrap();
    let mut rx2 = client
        .add_stream_channel("session-2", Streams::MOT)
        .unwrap();

    // Interleave samples from both sessions.
    connection.push_event(mot_event("session-1", 1.0)).await;
    connection.push_event(mot_event("session-2", 2.0)).await;
    connection.push_event(mot_event("session-1", 3.0)).await;
    connection.push_event(mot_event("session-2", 4.0)).await;

    for expected in [1.0, 3.0] {
        let event = timeout(STEP, rx1.recv())
            .await
            .expect("timed out on session-1 event")
            .expect("session-1 channel closed unexpectedly");
        assert_eq!(event["sid"], "session-1");
        assert!((event["mot"][6].as_f64().unwrap() - expected).abs() < 1e-9);
    }
    for expected in [2.0, 4.0] {
        let event = timeout(STEP, rx2.recv())
            .await
            .expect("timed out on session-2 event")
            .expect("session-2 channel closed unexpectedly");
        assert_eq!(event["sid"], "session-2");
        assert!((event["mot"][6].as_f64().unwrap() - expected).abs() < 1e-9);
    }

    client.shutdown().await;
}

// ─── H3: warning objects ─────────────────────────────────────────────────

#[tokio::test]
async fn session_closing_warning_cancels_only_target_session() {
    let Some(mut server) =
        start_server_or_skip("session_closing_warning_cancels_only_target_session").await
    else {
        return;
    };
    let config = test_config(server.ws_url());
    let client = CortexClient::connect(&config).await.unwrap();
    let connection = server.accept_connection().await;

    let mut rx1 = client
        .add_stream_channel("session-1", Streams::EEG)
        .unwrap();
    let mut rx2 = client
        .add_stream_channel("session-2", Streams::EEG)
        .unwrap();
    let mut warnings = client.warning_receiver();

    // Session 1's session is auto-closed by Cortex.
    connection
        .push_event(json!({
            "warning": {
                "code": WarningCodes::SESSION_AUTO_CLOSED,
                "message": {
                    "behavior": "The session was closed",
                    "sessionId": "session-1",
                },
            }
        }))
        .await;

    // The warning is observable...
    let warning = timeout(STEP, warnings.recv())
        .await
        .expect("timed out waiting for warning")
        .expect("warning channel closed");
    assert_eq!(warning.code, WarningCodes::SESSION_AUTO_CLOSED);
    assert_eq!(warning.session_id(), Some("session-1"));

    // ...session-1's stream terminates...
    let closed = timeout(STEP, rx1.recv())
        .await
        .expect("session-1 receiver must resolve after warning");
    assert!(closed.is_none(), "session-1 channel must be closed");

    // ...and session-2 keeps flowing.
    connection.push_event(eeg_event("session-2", 5.0)).await;
    let event = timeout(STEP, rx2.recv())
        .await
        .expect("timed out on session-2 event")
        .expect("session-2 channel must stay open");
    assert_eq!(event["sid"], "session-2");

    client.shutdown().await;
}

#[tokio::test]
async fn unknown_warning_codes_are_broadcast_without_side_effects() {
    let Some(mut server) =
        start_server_or_skip("unknown_warning_codes_are_broadcast_without_side_effects").await
    else {
        return;
    };
    let config = test_config(server.ws_url());
    let client = CortexClient::connect(&config).await.unwrap();
    let connection = server.accept_connection().await;

    let mut rx = client
        .add_stream_channel("session-1", Streams::EEG)
        .unwrap();
    let mut warnings = client.warning_receiver();

    // A string-message warning and an unknown future code.
    connection
        .push_event(json!({
            "warning": {"code": WarningCodes::USER_LOGIN, "message": "User logged in"}
        }))
        .await;
    connection
        .push_event(json!({
            "warning": {"code": 424_242, "message": {"future": "payload"}}
        }))
        .await;

    let first = timeout(STEP, warnings.recv()).await.unwrap().unwrap();
    assert_eq!(first.code, WarningCodes::USER_LOGIN);
    let second = timeout(STEP, warnings.recv()).await.unwrap().unwrap();
    assert_eq!(second.code, 424_242);

    // Streams unaffected.
    connection.push_event(eeg_event("session-1", 9.0)).await;
    let event = timeout(STEP, rx.recv())
        .await
        .expect("timed out on stream event")
        .expect("channel must stay open after non-canceling warnings");
    assert_eq!(event["sid"], "session-1");

    client.shutdown().await;
}
