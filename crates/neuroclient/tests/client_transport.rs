mod support;

use futures_util::StreamExt;
use neuroclient::protocol::constants::{Methods, Streams};
use neuroclient::protocol::headset::QueryHeadsetsOptions;
use neuroclient::{CortexClient, CortexConfig, CortexError, streams};
use serde_json::{Value, json};

use support::mock_cortex::MockCortexServer;

fn test_config(url: String) -> CortexConfig {
    let mut config = CortexConfig::new("test-client-id", "test-client-secret");
    config.cortex_url = url;
    config.timeouts.rpc_timeout_secs = 1;
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

#[tokio::test]
async fn connect_and_get_cortex_info_round_trip() {
    let Some(mut server) = start_server_or_skip("connect_and_get_cortex_info_round_trip").await
    else {
        return;
    };
    let config = test_config(server.ws_url());
    let mut client = CortexClient::connect(&config).await.unwrap();

    let mut connection = server.accept_connection().await;
    let responder = tokio::spawn(async move {
        let request = connection
            .recv_request_method(Methods::GET_CORTEX_INFO)
            .await;
        connection
            .send_result(rpc_id(&request), json!({"version": "mock-1.0.0"}))
            .await;
        request
    });

    let info = client.get_cortex_info().await.unwrap();
    let request = responder.await.unwrap();

    assert_eq!(info["version"], "mock-1.0.0");
    assert_eq!(request["jsonrpc"], "2.0");
    assert!(request.get("params").is_none());

    client.disconnect().await.unwrap();
}

#[tokio::test]
async fn authenticate_fallback_request_access_method_not_found() {
    let Some(mut server) =
        start_server_or_skip("authenticate_fallback_request_access_method_not_found").await
    else {
        return;
    };
    let config = test_config(server.ws_url());
    let mut client = CortexClient::connect(&config).await.unwrap();

    let mut connection = server.accept_connection().await;
    let responder = tokio::spawn(async move {
        let mut methods = Vec::new();

        let request = connection.recv_request().await;
        methods.push(request["method"].as_str().unwrap().to_string());
        connection
            .send_result(rpc_id(&request), json!({"version": "ok"}))
            .await;

        let request = connection.recv_request().await;
        methods.push(request["method"].as_str().unwrap().to_string());
        connection
            .send_error(rpc_id(&request), -32601, "requestAccess")
            .await;

        let request = connection.recv_request().await;
        methods.push(request["method"].as_str().unwrap().to_string());
        connection
            .send_result(rpc_id(&request), json!({"cortexToken": "token-fallback"}))
            .await;

        methods
    });

    let token = client
        .authenticate("test-client-id", "test-client-secret")
        .await
        .unwrap();
    let methods = responder.await.unwrap();

    assert_eq!(token, "token-fallback");
    assert_eq!(
        methods,
        vec![
            Methods::GET_CORTEX_INFO,
            Methods::REQUEST_ACCESS,
            Methods::AUTHORIZE,
        ]
    );

    client.disconnect().await.unwrap();
}

#[tokio::test]
async fn authenticate_fails_when_authorize_method_not_found() {
    let Some(mut server) =
        start_server_or_skip("authenticate_fails_when_authorize_method_not_found").await
    else {
        return;
    };
    let config = test_config(server.ws_url());
    let mut client = CortexClient::connect(&config).await.unwrap();

    let mut connection = server.accept_connection().await;
    let responder = tokio::spawn(async move {
        let request = connection
            .recv_request_method(Methods::GET_CORTEX_INFO)
            .await;
        connection
            .send_result(rpc_id(&request), json!({"version": "ok"}))
            .await;

        let request = connection
            .recv_request_method(Methods::REQUEST_ACCESS)
            .await;
        connection.send_result(rpc_id(&request), json!({})).await;

        let request = connection.recv_request_method(Methods::AUTHORIZE).await;
        connection
            .send_error(rpc_id(&request), -32601, "authorize")
            .await;
    });

    let err = client
        .authenticate("test-client-id", "test-client-secret")
        .await
        .unwrap_err();
    responder.await.unwrap();

    assert!(matches!(err, CortexError::AuthenticationFailed { .. }));
    assert!(
        err.to_string().contains("authorize"),
        "expected authorize detail in error: {err}"
    );

    client.disconnect().await.unwrap();
}

#[tokio::test]
async fn rpc_timeout_is_reported_and_next_call_still_works() {
    let Some(mut server) =
        start_server_or_skip("rpc_timeout_is_reported_and_next_call_still_works").await
    else {
        return;
    };
    let mut config = test_config(server.ws_url());
    config.timeouts.rpc_timeout_secs = 1;
    let mut client = CortexClient::connect(&config).await.unwrap();

    let mut connection = server.accept_connection().await;
    let responder = tokio::spawn(async move {
        let first_request = connection
            .recv_request_method(Methods::GET_CORTEX_INFO)
            .await;
        let second_request = connection
            .recv_request_method(Methods::GET_CORTEX_INFO)
            .await;
        connection
            .send_result(rpc_id(&second_request), json!({"ok": true}))
            .await;
        (first_request, second_request)
    });

    let timeout_err = client.get_cortex_info().await.unwrap_err();
    assert_eq!(client.pending_response_count().await, 0);
    let second = client.get_cortex_info().await.unwrap();
    let _ = responder.await.unwrap();

    assert!(matches!(timeout_err, CortexError::Timeout { seconds: 1 }));
    assert_eq!(second["ok"], true);

    client.disconnect().await.unwrap();
}

#[tokio::test]
async fn send_failure_cleans_pending_response_entry() {
    let Some(mut server) = start_server_or_skip("send_failure_cleans_pending_response_entry").await
    else {
        return;
    };
    let config = test_config(server.ws_url());
    let mut client = CortexClient::connect(&config).await.unwrap();

    let _connection = server.accept_connection().await;
    client.disconnect().await.unwrap();

    let err = client.get_cortex_info().await.unwrap_err();
    assert!(matches!(err, CortexError::WebSocket(_)));
    assert_eq!(client.pending_response_count().await, 0);
}

#[tokio::test]
async fn stop_reader_finishes_without_polling_delay() {
    let Some(mut server) = start_server_or_skip("stop_reader_finishes_without_polling_delay").await
    else {
        return;
    };
    let config = test_config(server.ws_url());
    let mut client = CortexClient::connect(&config).await.unwrap();
    let _connection = server.accept_connection().await;

    let start = std::time::Instant::now();
    client.stop_reader().await;
    let elapsed = start.elapsed();

    assert!(
        elapsed < std::time::Duration::from_millis(150),
        "reader stop took {elapsed:?}"
    );
    assert!(!client.is_connected());

    client.disconnect().await.unwrap();
}

#[tokio::test]
async fn stream_dispatch_stats_track_overflow_drops() {
    let Some(mut server) = start_server_or_skip("stream_dispatch_stats_track_overflow_drops").await
    else {
        return;
    };
    let config = test_config(server.ws_url());
    let mut client = CortexClient::connect(&config).await.unwrap();

    let connection = server.accept_connection().await;
    let _receivers = client
        .create_stream_channels("session-1", &[Streams::EEG])
        .unwrap();

    let pusher = tokio::spawn(async move {
        let event = json!({
            "sid": "session-1",
            "time": 1_609_459_200.0,
            "eeg": [1, 0, 1.0, 2.0, 3.0, 4.0, 5.0, 0.0, 0, []]
        });
        for _ in 0..(1024 + 256) {
            connection.push_event(event.clone()).await;
        }
    });
    pusher.await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let stats = client.stream_dispatch_stats();
    let eeg = stats
        .get(&("session-1".to_string(), "eeg"))
        .copied()
        .unwrap_or_default();
    assert!(eeg.delivered > 0);
    assert!(eeg.dropped_full > 0);
    assert_eq!(eeg.dropped_closed, 0);

    client.disconnect().await.unwrap();
}

#[tokio::test]
async fn subscribe_eeg_routes_stream_event_to_typed_stream() {
    let Some(mut server) =
        start_server_or_skip("subscribe_eeg_routes_stream_event_to_typed_stream").await
    else {
        return;
    };
    let config = test_config(server.ws_url());
    let mut client = CortexClient::connect(&config).await.unwrap();

    let mut connection = server.accept_connection().await;
    let responder = tokio::spawn(async move {
        let request = connection.recv_request_method(Methods::SUBSCRIBE).await;
        connection
            .send_result(rpc_id(&request), json!({"success": [Streams::EEG]}))
            .await;
        connection
            .push_event(json!({
                "sid": "session-1",
                "time": 1_609_459_200.0,
                "eeg": [29, 0, 4262.564, 4264.615, 4265.128, 4267.179, 4263.59, 0.0, 0, []]
            }))
            .await;
    });

    let mut eeg_stream = streams::subscribe_eeg(&client, "token", "session-1", 5)
        .await
        .unwrap();
    let sample = tokio::time::timeout(std::time::Duration::from_secs(2), eeg_stream.next())
        .await
        .expect("timed out waiting for eeg sample")
        .expect("typed stream ended unexpectedly");

    responder.await.unwrap();

    assert_eq!(sample.counter, 29);
    assert_eq!(sample.channels.len(), 5);
    assert!(!sample.interpolated);

    client.disconnect().await.unwrap();
}

#[tokio::test]
async fn subscribe_motion_accepts_documented_legacy_motion_payload() {
    let Some(mut server) =
        start_server_or_skip("subscribe_motion_accepts_documented_legacy_motion_payload").await
    else {
        return;
    };
    let config = test_config(server.ws_url());
    let mut client = CortexClient::connect(&config).await.unwrap();

    let mut connection = server.accept_connection().await;
    let responder = tokio::spawn(async move {
        let request = connection.recv_request_method(Methods::SUBSCRIBE).await;
        connection
            .send_result(
                rpc_id(&request),
                json!({
                    "success": [{
                        "cols": [
                            "COUNTER_MEMS",
                            "INTERPOLATED_MEMS",
                            "GYROX",
                            "GYROY",
                            "GYROZ",
                            "ACCX",
                            "ACCY",
                            "ACCZ",
                            "MAGX",
                            "MAGY",
                            "MAGZ"
                        ],
                        "sid": "session-1",
                        "streamName": Streams::MOT
                    }],
                    "failure": []
                }),
            )
            .await;
        connection
            .push_event(json!({
                "mot": [
                    14,
                    0,
                    8206,
                    8187,
                    8181,
                    4235,
                    8668,
                    8128,
                    8294,
                    8237,
                    7938
                ],
                "sid": "session-1",
                "time": 1_559_902_927.742_8
            }))
            .await;
    });

    let mut motion_stream = streams::subscribe_motion(&client, "token", "session-1")
        .await
        .unwrap();
    let motion = tokio::time::timeout(std::time::Duration::from_secs(2), motion_stream.next())
        .await
        .expect("timed out waiting for motion sample")
        .expect("typed stream ended unexpectedly");

    responder.await.unwrap();

    assert!(motion.quaternion.is_none());
    for (actual, expected) in motion
        .accelerometer
        .iter()
        .zip([4235.0_f32, 8668.0, 8128.0])
        .chain(motion.magnetometer.iter().zip([8294.0, 8237.0, 7938.0]))
    {
        assert!((actual - expected).abs() < f32::EPSILON);
    }

    client.disconnect().await.unwrap();
}

#[tokio::test]
async fn subscribe_metrics_maps_documented_mn8_labels() {
    let Some(mut server) =
        start_server_or_skip("subscribe_metrics_maps_documented_mn8_labels").await
    else {
        return;
    };
    let config = test_config(server.ws_url());
    let mut client = CortexClient::connect(&config).await.unwrap();

    let mut connection = server.accept_connection().await;
    let responder = tokio::spawn(async move {
        let request = connection.recv_request_method(Methods::SUBSCRIBE).await;
        connection
            .send_result(
                rpc_id(&request),
                json!({
                    "success": [{
                        "cols": [
                            "attention.isActive",
                            "attention",
                            "cognitiveStress.isActive",
                            "cognitiveStress"
                        ],
                        "sid": "session-1",
                        "streamName": Streams::MET
                    }],
                    "failure": []
                }),
            )
            .await;
        connection
            .push_event(json!({
                "met": [true, 0.8, true, 0.4],
                "sid": "session-1",
                "time": 1_759_225_262.505_2
            }))
            .await;
    });

    let mut metrics_stream = streams::subscribe_metrics(&client, "token", "session-1")
        .await
        .unwrap();
    let metrics = tokio::time::timeout(std::time::Duration::from_secs(2), metrics_stream.next())
        .await
        .expect("timed out waiting for metrics sample")
        .expect("typed stream ended unexpectedly");

    responder.await.unwrap();

    assert_eq!(metrics.attention, Some(0.8));
    assert_eq!(metrics.stress, Some(0.4));

    client.disconnect().await.unwrap();
}

#[tokio::test]
async fn subscribe_mental_commands_routes_documented_payload_to_typed_stream() {
    let Some(mut server) =
        start_server_or_skip("subscribe_mental_commands_routes_documented_payload_to_typed_stream")
            .await
    else {
        return;
    };
    let config = test_config(server.ws_url());
    let mut client = CortexClient::connect(&config).await.unwrap();

    let mut connection = server.accept_connection().await;
    let responder = tokio::spawn(async move {
        let request = connection.recv_request_method(Methods::SUBSCRIBE).await;
        connection
            .send_result(
                rpc_id(&request),
                json!({
                    "success": [{
                        "cols": ["act", "pow"],
                        "sid": "session-1",
                        "streamName": Streams::COM
                    }],
                    "failure": []
                }),
            )
            .await;
        connection
            .push_event(json!({
                "com": ["pull", 0.564],
                "sid": "session-1",
                "time": 1_559_903_099.348
            }))
            .await;
    });

    let mut commands = streams::subscribe_mental_commands(&client, "token", "session-1")
        .await
        .unwrap();
    let command = tokio::time::timeout(std::time::Duration::from_secs(2), commands.next())
        .await
        .expect("timed out waiting for mental-command sample")
        .expect("typed stream ended unexpectedly");

    responder.await.unwrap();

    assert_eq!(command.action, "pull");
    assert!((command.power - 0.564).abs() < f32::EPSILON);

    client.disconnect().await.unwrap();
}

#[tokio::test]
async fn subscribe_dev_routes_documented_payload_to_typed_stream() {
    let Some(mut server) =
        start_server_or_skip("subscribe_dev_routes_documented_payload_to_typed_stream").await
    else {
        return;
    };
    let config = test_config(server.ws_url());
    let mut client = CortexClient::connect(&config).await.unwrap();

    let mut connection = server.accept_connection().await;
    let responder = tokio::spawn(async move {
        let request = connection.recv_request_method(Methods::SUBSCRIBE).await;
        connection
            .send_result(
                rpc_id(&request),
                json!({
                    "success": [{
                        "cols": [
                            "Battery",
                            "Signal",
                            ["AF3", "T7", "Pz", "T8", "AF4", "OVERALL"],
                            "BatteryPercent"
                        ],
                        "sid": "session-1",
                        "streamName": Streams::DEV
                    }],
                    "failure": []
                }),
            )
            .await;
        connection
            .push_event(json!({
                "dev": [3, 1, [4, 1, 1, 2, 4, 25], 74],
                "sid": "session-1",
                "time": 1_590_403_053.500_2
            }))
            .await;
    });

    let mut device_quality = streams::subscribe_dev(&client, "token", "session-1", 5)
        .await
        .unwrap();
    let sample = tokio::time::timeout(std::time::Duration::from_secs(2), device_quality.next())
        .await
        .expect("timed out waiting for device-quality sample")
        .expect("typed stream ended unexpectedly");

    responder.await.unwrap();

    assert_eq!(sample.battery_level, 3);
    assert!((sample.signal_strength - 1.0).abs() < f32::EPSILON);
    assert_eq!(sample.channel_quality.len(), 5);
    assert!((sample.channel_quality[0] - 1.0).abs() < f32::EPSILON);
    assert!((sample.channel_quality[1] - 0.25).abs() < f32::EPSILON);
    assert!((sample.overall_quality - 0.25).abs() < f32::EPSILON);
    assert_eq!(sample.battery_percent, 74);

    client.disconnect().await.unwrap();
}

#[tokio::test]
async fn subscribe_eq_routes_documented_payload_to_typed_stream() {
    let Some(mut server) =
        start_server_or_skip("subscribe_eq_routes_documented_payload_to_typed_stream").await
    else {
        return;
    };
    let config = test_config(server.ws_url());
    let mut client = CortexClient::connect(&config).await.unwrap();

    let mut connection = server.accept_connection().await;
    let responder = tokio::spawn(async move {
        let request = connection.recv_request_method(Methods::SUBSCRIBE).await;
        connection
            .send_result(
                rpc_id(&request),
                json!({
                    "success": [{
                        "cols": [
                            "batteryPercent",
                            "overall",
                            "sampleRateQuality",
                            "AF3",
                            "T7",
                            "Pz",
                            "T8",
                            "AF4"
                        ],
                        "sid": "session-1",
                        "streamName": Streams::EQ
                    }],
                    "failure": []
                }),
            )
            .await;
        connection
            .push_event(json!({
                "eq": [78, 25, 1.0, 4, 1, 1, 2, 4],
                "sid": "session-1",
                "time": 1_590_403_053.500_2
            }))
            .await;
    });

    let mut eeg_quality = streams::subscribe_eq(&client, "token", "session-1", 5)
        .await
        .unwrap();
    let sample = tokio::time::timeout(std::time::Duration::from_secs(2), eeg_quality.next())
        .await
        .expect("timed out waiting for eeg-quality sample")
        .expect("typed stream ended unexpectedly");

    responder.await.unwrap();

    assert_eq!(sample.battery_percent, 78);
    assert!((sample.overall - 0.25).abs() < f32::EPSILON);
    assert!((sample.sample_rate_quality - 1.0).abs() < f32::EPSILON);
    assert_eq!(sample.sensor_quality.len(), 5);
    assert!((sample.sensor_quality[0] - 1.0).abs() < f32::EPSILON);
    assert!((sample.sensor_quality[3] - 0.5).abs() < f32::EPSILON);

    client.disconnect().await.unwrap();
}

#[tokio::test]
async fn subscribe_band_power_routes_documented_payload_to_typed_stream() {
    let Some(mut server) =
        start_server_or_skip("subscribe_band_power_routes_documented_payload_to_typed_stream")
            .await
    else {
        return;
    };
    let config = test_config(server.ws_url());
    let mut client = CortexClient::connect(&config).await.unwrap();

    let mut connection = server.accept_connection().await;
    let responder = tokio::spawn(async move {
        let request = connection.recv_request_method(Methods::SUBSCRIBE).await;
        connection
            .send_result(
                rpc_id(&request),
                json!({
                    "success": [{
                        "cols": [
                            "AF3/theta", "AF3/alpha", "AF3/betaL", "AF3/betaH", "AF3/gamma",
                            "T7/theta", "T7/alpha", "T7/betaL", "T7/betaH", "T7/gamma",
                            "Pz/theta", "Pz/alpha", "Pz/betaL", "Pz/betaH", "Pz/gamma",
                            "T8/theta", "T8/alpha", "T8/betaL", "T8/betaH", "T8/gamma",
                            "AF4/theta", "AF4/alpha", "AF4/betaL", "AF4/betaH", "AF4/gamma"
                        ],
                        "sid": "session-1",
                        "streamName": Streams::POW
                    }],
                    "failure": []
                }),
            )
            .await;
        connection
            .push_event(json!({
                "pow": [
                    1.246, 0.706, 0.566, 1.065, 0.602,
                    10.293, 4.374, 11.638, 351.767, 40.273,
                    50.159, 4.585, 0.467, 1.481, 3.764,
                    9.861, 3.139, 2.094, 3.342, 4.452,
                    75.652, 1.972, 2.932, 2.555, 7.005
                ],
                "sid": "session-1",
                "time": 1_590_403_491.030_7
            }))
            .await;
    });

    let mut band_power = streams::subscribe_band_power(&client, "token", "session-1", 5)
        .await
        .unwrap();
    let sample = tokio::time::timeout(std::time::Duration::from_secs(2), band_power.next())
        .await
        .expect("timed out waiting for band-power sample")
        .expect("typed stream ended unexpectedly");

    responder.await.unwrap();

    assert_eq!(sample.channel_powers.len(), 5);
    assert!((sample.channel_powers[0][0] - 1.246).abs() < f32::EPSILON);
    assert!((sample.channel_powers[1][3] - 351.767).abs() < f32::EPSILON);
    assert!((sample.channel_powers[4][4] - 7.005).abs() < f32::EPSILON);

    client.disconnect().await.unwrap();
}

#[tokio::test]
async fn subscribe_facial_expressions_routes_documented_payload_to_typed_stream() {
    let Some(mut server) = start_server_or_skip(
        "subscribe_facial_expressions_routes_documented_payload_to_typed_stream",
    )
    .await
    else {
        return;
    };
    let config = test_config(server.ws_url());
    let mut client = CortexClient::connect(&config).await.unwrap();

    let mut connection = server.accept_connection().await;
    let responder = tokio::spawn(async move {
        let request = connection.recv_request_method(Methods::SUBSCRIBE).await;
        connection
            .send_result(
                rpc_id(&request),
                json!({
                    "success": [{
                        "cols": ["eyeAct", "uAct", "uPow", "lAct", "lPow"],
                        "sid": "session-1",
                        "streamName": Streams::FAC
                    }],
                    "failure": []
                }),
            )
            .await;
        connection
            .push_event(json!({
                "fac": ["neutral", "neutral", 0, "clench", 0.0576],
                "sid": "session-1",
                "time": 1_559_903_035.296_1
            }))
            .await;
    });

    let mut facial = streams::subscribe_facial_expressions(&client, "token", "session-1")
        .await
        .unwrap();
    let sample = tokio::time::timeout(std::time::Duration::from_secs(2), facial.next())
        .await
        .expect("timed out waiting for facial-expression sample")
        .expect("typed stream ended unexpectedly");

    responder.await.unwrap();

    assert_eq!(sample.eye_action, "neutral");
    assert_eq!(sample.upper_face_action, "neutral");
    assert!((sample.upper_face_power - 0.0).abs() < f32::EPSILON);
    assert_eq!(sample.lower_face_action, "clench");
    assert!((sample.lower_face_power - 0.0576).abs() < f32::EPSILON);

    client.disconnect().await.unwrap();
}

#[tokio::test]
async fn subscribe_sys_routes_documented_payload_to_typed_stream() {
    let Some(mut server) =
        start_server_or_skip("subscribe_sys_routes_documented_payload_to_typed_stream").await
    else {
        return;
    };
    let config = test_config(server.ws_url());
    let mut client = CortexClient::connect(&config).await.unwrap();

    let mut connection = server.accept_connection().await;
    let responder = tokio::spawn(async move {
        let request = connection.recv_request_method(Methods::SUBSCRIBE).await;
        connection
            .send_result(
                rpc_id(&request),
                json!({
                    "success": [{
                        "cols": ["event", "msg"],
                        "sid": "session-1",
                        "streamName": Streams::SYS
                    }],
                    "failure": []
                }),
            )
            .await;
        connection
            .push_event(json!({
                "sid": "session-1",
                "sys": ["mentalCommand", "MC_Started"],
                "time": 1_559_903_035.296_1
            }))
            .await;
    });

    let mut sys_events = streams::subscribe_sys(&client, "token", "session-1")
        .await
        .unwrap();
    let sample = tokio::time::timeout(std::time::Duration::from_secs(2), sys_events.next())
        .await
        .expect("timed out waiting for system-event sample")
        .expect("typed stream ended unexpectedly");

    responder.await.unwrap();

    assert_eq!(sample.sid, "session-1");
    assert_eq!(sample.sys.len(), 2);
    assert_eq!(sample.sys[0].as_str(), Some("mentalCommand"));
    assert_eq!(sample.sys[1].as_str(), Some("MC_Started"));

    client.disconnect().await.unwrap();
}

#[tokio::test]
// Full subscribe/dispatch/unsubscribe scenario; splitting would hide ordering.
#[allow(clippy::too_many_lines)]
async fn unsubscribe_closes_motion_stream_after_channel_cleanup() {
    let Some(mut server) =
        start_server_or_skip("unsubscribe_closes_motion_stream_after_channel_cleanup").await
    else {
        return;
    };
    let config = test_config(server.ws_url());
    let mut client = CortexClient::connect(&config).await.unwrap();
    let (post_unsubscribe_tx, post_unsubscribe_rx) = tokio::sync::oneshot::channel();

    let mut connection = server.accept_connection().await;
    let responder = tokio::spawn(async move {
        let subscribe = connection.recv_request_method(Methods::SUBSCRIBE).await;
        connection
            .send_result(
                rpc_id(&subscribe),
                json!({
                    "success": [{
                        "cols": [
                            "COUNTER_MEMS",
                            "INTERPOLATED_MEMS",
                            "Q0",
                            "Q1",
                            "Q2",
                            "Q3",
                            "ACCX",
                            "ACCY",
                            "ACCZ",
                            "MAGX",
                            "MAGY",
                            "MAGZ"
                        ],
                        "sid": "session-1",
                        "streamName": Streams::MOT
                    }],
                    "failure": []
                }),
            )
            .await;
        connection
            .push_event(json!({
                "mot": [
                    48,
                    0,
                    0.735_341,
                    0.255_615,
                    0.627_441,
                    -0.015_869,
                    0.948_257,
                    -0.354_986,
                    -0.083_497,
                    -44.656_766,
                    -86.970_985,
                    23.221_568
                ],
                "sid": "session-1",
                "time": 1_590_402_244.824_2
            }))
            .await;

        let unsubscribe = connection.recv_request_method(Methods::UNSUBSCRIBE).await;
        connection
            .send_result(
                rpc_id(&unsubscribe),
                json!({
                    "success": [Streams::MOT],
                    "failure": []
                }),
            )
            .await;

        post_unsubscribe_rx
            .await
            .expect("post-unsubscribe signal dropped unexpectedly");

        connection
            .push_event(json!({
                "mot": [
                    49,
                    0,
                    0.735_341,
                    0.255_615,
                    0.627_441,
                    -0.015_869,
                    0.948_257,
                    -0.354_986,
                    -0.083_497,
                    -44.656_766,
                    -86.970_985,
                    23.221_568
                ],
                "sid": "session-1",
                "time": 1_590_402_245.824_2
            }))
            .await;
    });

    let mut motion_stream = streams::subscribe_motion(&client, "token", "session-1")
        .await
        .unwrap();
    let first = tokio::time::timeout(std::time::Duration::from_secs(2), motion_stream.next())
        .await
        .expect("timed out waiting for first motion sample")
        .expect("typed stream ended unexpectedly before unsubscribe");
    assert!(first.quaternion.is_some());

    streams::unsubscribe(&client, "token", "session-1", &[Streams::MOT])
        .await
        .unwrap();
    let _ = post_unsubscribe_tx.send(());

    let next = tokio::time::timeout(std::time::Duration::from_secs(2), motion_stream.next())
        .await
        .expect("timed out waiting for stream shutdown");

    responder.await.unwrap();

    assert!(
        next.is_none(),
        "motion stream should close after unsubscribe"
    );

    client.disconnect().await.unwrap();
}

#[tokio::test]
async fn api_error_code_maps_to_domain_error() {
    let Some(mut server) = start_server_or_skip("api_error_code_maps_to_domain_error").await else {
        return;
    };
    let config = test_config(server.ws_url());
    let mut client = CortexClient::connect(&config).await.unwrap();

    let mut connection = server.accept_connection().await;
    let responder = tokio::spawn(async move {
        let request = connection
            .recv_request_method(Methods::CONTROL_DEVICE)
            .await;
        connection
            .send_error(rpc_id(&request), -32001, "no headset connected")
            .await;
    });

    let err = client.connect_headset("HS-1").await.unwrap_err();
    responder.await.unwrap();

    assert!(matches!(err, CortexError::NoHeadsetFound));

    client.disconnect().await.unwrap();
}

#[tokio::test]
async fn query_headsets_options_round_trip_over_transport() {
    let Some(mut server) =
        start_server_or_skip("query_headsets_options_round_trip_over_transport").await
    else {
        return;
    };
    let config = test_config(server.ws_url());
    let mut client = CortexClient::connect(&config).await.unwrap();

    let mut connection = server.accept_connection().await;
    let responder = tokio::spawn(async move {
        let request = connection
            .recv_request_method(Methods::QUERY_HEADSETS)
            .await;
        connection.send_result(rpc_id(&request), json!([])).await;
        request
    });

    let _ = client
        .query_headsets(QueryHeadsetsOptions {
            id: Some("HS-123".to_string()),
            include_flex_mappings: true,
        })
        .await
        .unwrap();

    let request = responder.await.unwrap();
    assert_eq!(request["params"]["id"], "HS-123");
    assert_eq!(request["params"]["includeFlexMappings"], true);

    client.disconnect().await.unwrap();
}

// ─── Error-path tests: protocol and API error mapping ───────────────────────

#[tokio::test]
async fn rpc_response_null_result_no_error_yields_protocol_error() {
    let Some(mut server) =
        start_server_or_skip("rpc_response_null_result_no_error_yields_protocol_error").await
    else {
        return;
    };
    let config = test_config(server.ws_url());
    let mut client = CortexClient::connect(&config).await.unwrap();

    let mut connection = server.accept_connection().await;
    let responder = tokio::spawn(async move {
        let request = connection
            .recv_request_method(Methods::GET_CORTEX_INFO)
            .await;
        connection
            .send_json(json!({"jsonrpc": "2.0", "id": rpc_id(&request)}))
            .await;
        request
    });

    let err = client.get_cortex_info().await.unwrap_err();
    responder.await.unwrap();

    assert!(matches!(err, CortexError::ProtocolError { .. }));
    assert!(
        err.to_string().contains("no result or error"),
        "expected protocol error about missing result/error, got: {err}"
    );

    client.disconnect().await.unwrap();
}

#[tokio::test]
async fn rpc_response_malformed_json_yields_protocol_error() {
    let Some(mut server) =
        start_server_or_skip("rpc_response_malformed_json_yields_protocol_error").await
    else {
        return;
    };
    let config = test_config(server.ws_url());
    let mut client = CortexClient::connect(&config).await.unwrap();

    let mut connection = server.accept_connection().await;
    let responder = tokio::spawn(async move {
        let request = connection
            .recv_request_method(Methods::GET_CORTEX_INFO)
            .await;
        let id = rpc_id(&request);
        // Valid numeric id so the client routes to the pending request; "error" as string
        // so CortexResponse deserialization fails (expects { code, message }).
        connection
            .send_json(json!({"jsonrpc": "2.0", "id": id, "error": "not an object"}))
            .await;
        request
    });

    let err = client.get_cortex_info().await.unwrap_err();
    responder.await.unwrap();

    assert!(matches!(err, CortexError::ProtocolError { .. }));
    assert!(
        err.to_string().contains("parse") || err.to_string().contains("Protocol"),
        "expected protocol/parse error, got: {err}"
    );

    client.disconnect().await.unwrap();
}

#[tokio::test]
async fn get_user_login_result_wrong_type_yields_protocol_error() {
    let Some(mut server) =
        start_server_or_skip("get_user_login_result_wrong_type_yields_protocol_error").await
    else {
        return;
    };
    let config = test_config(server.ws_url());
    let mut client = CortexClient::connect(&config).await.unwrap();

    let mut connection = server.accept_connection().await;
    let responder = tokio::spawn(async move {
        let request = connection
            .recv_request_method(Methods::GET_USER_LOGIN)
            .await;
        // Result must be an array of UserLoginInfo; a number is invalid.
        connection.send_result(rpc_id(&request), json!(123)).await;
        request
    });

    let err = client.get_user_login().await.unwrap_err();
    responder.await.unwrap();

    assert!(matches!(err, CortexError::ProtocolError { .. }));
    assert!(
        err.to_string().contains("parse")
            || err.to_string().contains("Protocol")
            || err.to_string().contains("user login"),
        "expected protocol/parse error for wrong result type, got: {err}"
    );

    client.disconnect().await.unwrap();
}

#[tokio::test]
async fn api_error_token_expired_maps_to_token_expired() {
    let Some(mut server) =
        start_server_or_skip("api_error_token_expired_maps_to_token_expired").await
    else {
        return;
    };
    let config = test_config(server.ws_url());
    let mut client = CortexClient::connect(&config).await.unwrap();

    let mut connection = server.accept_connection().await;
    let responder = tokio::spawn(async move {
        let request = connection
            .recv_request_method(Methods::GET_CORTEX_INFO)
            .await;
        connection
            .send_error(rpc_id(&request), -32015, "cortex token expired")
            .await;
    });

    let err = client.get_cortex_info().await.unwrap_err();
    responder.await.unwrap();

    assert!(matches!(err, CortexError::TokenExpired));

    client.disconnect().await.unwrap();
}

#[tokio::test]
async fn api_error_session_error_maps_and_preserves_message() {
    let Some(mut server) =
        start_server_or_skip("api_error_session_error_maps_and_preserves_message").await
    else {
        return;
    };
    let config = test_config(server.ws_url());
    let mut client = CortexClient::connect(&config).await.unwrap();

    let mut connection = server.accept_connection().await;
    let responder = tokio::spawn(async move {
        let request = connection
            .recv_request_method(Methods::GET_CORTEX_INFO)
            .await;
        connection
            .send_error(
                rpc_id(&request),
                -32005,
                "session already exists for this headset",
            )
            .await;
    });

    let err = client.get_cortex_info().await.unwrap_err();
    responder.await.unwrap();

    match &err {
        CortexError::SessionError { reason } => assert!(reason.contains("session already exists")),
        _ => panic!("expected SessionError with message, got {err:?}"),
    }

    client.disconnect().await.unwrap();
}

#[tokio::test]
async fn api_error_stream_error_maps_and_preserves_message() {
    let Some(mut server) =
        start_server_or_skip("api_error_stream_error_maps_and_preserves_message").await
    else {
        return;
    };
    let config = test_config(server.ws_url());
    let mut client = CortexClient::connect(&config).await.unwrap();

    let mut connection = server.accept_connection().await;
    let responder = tokio::spawn(async move {
        let request = connection
            .recv_request_method(Methods::GET_CORTEX_INFO)
            .await;
        connection
            .send_error(rpc_id(&request), -32016, "invalid stream name")
            .await;
    });

    let err = client.get_cortex_info().await.unwrap_err();
    responder.await.unwrap();

    match &err {
        CortexError::StreamError { reason } => assert!(reason.contains("invalid stream")),
        _ => panic!("expected StreamError with message, got {err:?}"),
    }

    client.disconnect().await.unwrap();
}

#[tokio::test]
async fn api_error_user_not_logged_in_maps_correctly() {
    let Some(mut server) =
        start_server_or_skip("api_error_user_not_logged_in_maps_correctly").await
    else {
        return;
    };
    let config = test_config(server.ws_url());
    let mut client = CortexClient::connect(&config).await.unwrap();

    let mut connection = server.accept_connection().await;
    let responder = tokio::spawn(async move {
        let request = connection
            .recv_request_method(Methods::GET_CORTEX_INFO)
            .await;
        connection
            .send_error(rpc_id(&request), -32033, "user not logged in to emotiv id")
            .await;
    });

    let err = client.get_cortex_info().await.unwrap_err();
    responder.await.unwrap();

    assert!(matches!(err, CortexError::UserNotLoggedIn));

    client.disconnect().await.unwrap();
}

#[tokio::test]
async fn api_error_not_approved_maps_correctly() {
    let Some(mut server) = start_server_or_skip("api_error_not_approved_maps_correctly").await
    else {
        return;
    };
    let config = test_config(server.ws_url());
    let mut client = CortexClient::connect(&config).await.unwrap();

    let mut connection = server.accept_connection().await;
    let responder = tokio::spawn(async move {
        let request = connection
            .recv_request_method(Methods::GET_CORTEX_INFO)
            .await;
        connection
            .send_error(rpc_id(&request), -32142, "application not approved")
            .await;
    });

    let err = client.get_cortex_info().await.unwrap_err();
    responder.await.unwrap();

    assert!(matches!(err, CortexError::NotApproved));

    client.disconnect().await.unwrap();
}

#[tokio::test]
async fn api_error_license_error_maps_and_preserves_message() {
    let Some(mut server) =
        start_server_or_skip("api_error_license_error_maps_and_preserves_message").await
    else {
        return;
    };
    let config = test_config(server.ws_url());
    let mut client = CortexClient::connect(&config).await.unwrap();

    let mut connection = server.accept_connection().await;
    let responder = tokio::spawn(async move {
        let request = connection
            .recv_request_method(Methods::GET_CORTEX_INFO)
            .await;
        connection
            .send_error(rpc_id(&request), -32024, "license expired")
            .await;
    });

    let err = client.get_cortex_info().await.unwrap_err();
    responder.await.unwrap();

    match &err {
        CortexError::LicenseError { reason } => assert!(reason.contains("license expired")),
        _ => panic!("expected LicenseError with message, got {err:?}"),
    }

    client.disconnect().await.unwrap();
}

#[tokio::test]
async fn api_error_headset_error_maps_and_preserves_message() {
    let Some(mut server) =
        start_server_or_skip("api_error_headset_error_maps_and_preserves_message").await
    else {
        return;
    };
    let config = test_config(server.ws_url());
    let mut client = CortexClient::connect(&config).await.unwrap();

    let mut connection = server.accept_connection().await;
    let responder = tokio::spawn(async move {
        let request = connection
            .recv_request_method(Methods::GET_CORTEX_INFO)
            .await;
        connection
            .send_error(rpc_id(&request), -32152, "headset not ready")
            .await;
    });

    let err = client.get_cortex_info().await.unwrap_err();
    responder.await.unwrap();

    match &err {
        CortexError::HeadsetError { reason } => assert!(reason.contains("headset not ready")),
        _ => panic!("expected HeadsetError with message, got {err:?}"),
    }

    client.disconnect().await.unwrap();
}

#[tokio::test]
async fn api_error_method_not_found_includes_method_name() {
    let Some(mut server) =
        start_server_or_skip("api_error_method_not_found_includes_method_name").await
    else {
        return;
    };
    let config = test_config(server.ws_url());
    let mut client = CortexClient::connect(&config).await.unwrap();

    let mut connection = server.accept_connection().await;
    let responder = tokio::spawn(async move {
        let request = connection
            .recv_request_method(Methods::GET_CORTEX_INFO)
            .await;
        connection
            .send_error(rpc_id(&request), -32601, "getCortexInfo")
            .await;
    });

    let err = client.get_cortex_info().await.unwrap_err();
    responder.await.unwrap();

    match &err {
        CortexError::MethodNotFound { method } => assert_eq!(method, "getCortexInfo"),
        _ => panic!("expected MethodNotFound with method name, got {err:?}"),
    }

    client.disconnect().await.unwrap();
}
