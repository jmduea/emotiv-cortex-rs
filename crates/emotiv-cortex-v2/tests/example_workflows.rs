mod support;

use emotiv_cortex_v2::protocol::constants::{Methods, Streams};
use emotiv_cortex_v2::protocol::headset::QueryHeadsetsOptions;
use emotiv_cortex_v2::protocol::profiles::ProfileAction;
use emotiv_cortex_v2::protocol::training::DetectionType;
use emotiv_cortex_v2::{CortexClient, CortexConfig, streams};
use futures_util::StreamExt;
use serde_json::{Value, json};

use support::mock_cortex::{MockConnection, MockCortexServer};

const CLIENT_ID: &str = "test-client-id";
const CLIENT_SECRET: &str = "test-client-secret";
const TOKEN: &str = "token-example";
const HEADSET_ID: &str = "INSIGHT-12345678";
const SESSION_ID: &str = "session-example";
const PROFILE_NAME: &str = "profile-alpha";

fn test_config(url: String) -> CortexConfig {
    let mut config = CortexConfig::new(CLIENT_ID, CLIENT_SECRET);
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

fn headset_response() -> Value {
    json!([{
        "id": HEADSET_ID,
        "status": "connected",
        "connectedBy": "dongle",
        "firmware": "1.0.0",
        "sensors": ["AF3", "T7", "Pz", "T8", "AF4"],
        "motionSensors": ["ACCX", "ACCY", "ACCZ", "MAGX", "MAGY", "MAGZ"]
    }])
}

fn session_response() -> Value {
    json!({
        "id": SESSION_ID,
        "status": "activated",
        "owner": "contract-user",
        "license": "license-001",
        "appId": "com.contract.example",
        "started": "2026-02-12T08:59:00Z",
        "streams": [],
        "recordIds": [],
        "recording": false,
        "headset": {
            "id": HEADSET_ID,
            "status": "connected"
        }
    })
}

async fn respond_authenticate(connection: &mut MockConnection) {
    let info = connection
        .recv_request_method(Methods::GET_CORTEX_INFO)
        .await;
    connection
        .send_result(rpc_id(&info), json!({"version": "mock"}))
        .await;

    let request_access = connection
        .recv_request_method(Methods::REQUEST_ACCESS)
        .await;
    assert_eq!(request_access["params"]["clientId"], CLIENT_ID);
    assert_eq!(request_access["params"]["clientSecret"], CLIENT_SECRET);
    connection
        .send_result(
            rpc_id(&request_access),
            json!({
                "accessGranted": true,
                "message": "Application was already approved"
            }),
        )
        .await;

    let authorize = connection.recv_request_method(Methods::AUTHORIZE).await;
    assert_eq!(authorize["params"]["clientId"], CLIENT_ID);
    assert_eq!(authorize["params"]["clientSecret"], CLIENT_SECRET);
    connection
        .send_result(rpc_id(&authorize), json!({"cortexToken": TOKEN}))
        .await;
}

#[tokio::test]
async fn auth_example_workflow_matches_mocked_rpc_sequence() {
    let mut server =
        match start_server_or_skip("auth_example_workflow_matches_mocked_rpc_sequence").await {
            Some(server) => server,
            None => return,
        };
    let config = test_config(server.ws_url());
    let mut client = CortexClient::connect(&config).await.unwrap();

    let mut connection = server.accept_connection().await;
    let responder = tokio::spawn(async move {
        let user_login = connection
            .recv_request_method(Methods::GET_USER_LOGIN)
            .await;
        connection
            .send_result(
                rpc_id(&user_login),
                json!([{"username": "contract-user", "currentOSUId": "launcher"}]),
            )
            .await;

        respond_authenticate(&mut connection).await;
    });

    let users = client.get_user_login().await.unwrap();
    let token = client.authenticate(CLIENT_ID, CLIENT_SECRET).await.unwrap();

    responder.await.unwrap();

    assert_eq!(users.len(), 1);
    assert_eq!(users[0].username, "contract-user");
    assert_eq!(token, TOKEN);

    client.disconnect().await.unwrap();
}

#[tokio::test]
async fn records_example_workflow_matches_mocked_rpc_sequence() {
    let mut server =
        match start_server_or_skip("records_example_workflow_matches_mocked_rpc_sequence").await {
            Some(server) => server,
            None => return,
        };
    let config = test_config(server.ws_url());
    let mut client = CortexClient::connect(&config).await.unwrap();

    let mut connection = server.accept_connection().await;
    let responder = tokio::spawn(async move {
        respond_authenticate(&mut connection).await;

        let query_headsets = connection
            .recv_request_method(Methods::QUERY_HEADSETS)
            .await;
        assert!(
            query_headsets.get("params").is_none() || query_headsets["params"] == json!({}),
            "expected queryHeadsets default request to omit params or send an empty object: {query_headsets}"
        );
        connection
            .send_result(rpc_id(&query_headsets), headset_response())
            .await;

        let create_session = connection
            .recv_request_method(Methods::CREATE_SESSION)
            .await;
        assert_eq!(create_session["params"]["headset"], HEADSET_ID);
        assert_eq!(create_session["params"]["status"], "active");
        connection
            .send_result(rpc_id(&create_session), session_response())
            .await;

        let create_record = connection.recv_request_method(Methods::CREATE_RECORD).await;
        assert_eq!(
            create_record["params"]["title"],
            "emotiv-cortex-v2 example recording"
        );
        connection
            .send_result(
                rpc_id(&create_record),
                json!({
                    "record": {
                        "uuid": "record-1",
                        "title": "emotiv-cortex-v2 example recording",
                        "startDatetime": "2026-02-12T09:00:00Z"
                    }
                }),
            )
            .await;

        for index in 1..=3 {
            let inject_marker = connection.recv_request_method(Methods::INJECT_MARKER).await;
            assert_eq!(inject_marker["params"]["label"], format!("event_{index}"));
            assert_eq!(inject_marker["params"]["value"], index);
            connection
                .send_result(
                    rpc_id(&inject_marker),
                    json!({
                        "marker": {
                            "uuid": format!("marker-{index}"),
                            "startDatetime": "2026-02-12T09:00:01Z"
                        }
                    }),
                )
                .await;
        }

        let stop_record = connection.recv_request_method(Methods::STOP_RECORD).await;
        connection
            .send_result(
                rpc_id(&stop_record),
                json!({
                    "record": {
                        "uuid": "record-1",
                        "title": "emotiv-cortex-v2 example recording",
                        "startDatetime": "2026-02-12T09:00:00Z",
                        "endDatetime": "2026-02-12T09:00:04Z"
                    }
                }),
            )
            .await;

        let query_records = connection.recv_request_method(Methods::QUERY_RECORDS).await;
        assert_eq!(query_records["params"]["limit"], 5);
        assert_eq!(query_records["params"]["query"], json!({}));
        assert_eq!(
            query_records["params"]["orderBy"],
            json!([{"startDatetime": "DESC"}])
        );
        assert!(query_records["params"].get("offset").is_none());
        connection
            .send_result(
                rpc_id(&query_records),
                json!({
                    "records": [{
                        "uuid": "record-1",
                        "title": "emotiv-cortex-v2 example recording",
                        "startDatetime": "2026-02-12T09:00:00Z",
                        "endDatetime": "2026-02-12T09:00:04Z"
                    }]
                }),
            )
            .await;

        let close_session = connection
            .recv_request_method(Methods::UPDATE_SESSION)
            .await;
        assert_eq!(close_session["params"]["status"], "close");
        connection
            .send_result(rpc_id(&close_session), json!({"message": "Session closed"}))
            .await;
    });

    let token = client.authenticate(CLIENT_ID, CLIENT_SECRET).await.unwrap();
    let headsets = client
        .query_headsets(QueryHeadsetsOptions::default())
        .await
        .unwrap();
    let headset = headsets.first().unwrap();
    let session = client.create_session(&token, &headset.id).await.unwrap();
    let record = client
        .create_record(&token, &session.id, "emotiv-cortex-v2 example recording")
        .await
        .unwrap();

    for index in 1..=3 {
        client
            .inject_marker(
                &token,
                &session.id,
                &format!("event_{index}"),
                index,
                "emotiv-cortex-v2-example",
                None,
            )
            .await
            .unwrap();
    }

    let stopped = client.stop_record(&token, &session.id).await.unwrap();
    let records = client.query_records(&token, Some(5), None).await.unwrap();
    client.close_session(&token, &session.id).await.unwrap();

    responder.await.unwrap();

    assert_eq!(record.uuid, "record-1");
    assert_eq!(stopped.uuid, "record-1");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].uuid, "record-1");

    client.disconnect().await.unwrap();
}

#[tokio::test]
async fn motion_example_workflow_matches_documented_subscribe_and_sample() {
    let mut server = match start_server_or_skip(
        "motion_example_workflow_matches_documented_subscribe_and_sample",
    )
    .await
    {
        Some(server) => server,
        None => return,
    };
    let config = test_config(server.ws_url());
    let mut client = CortexClient::connect(&config).await.unwrap();

    let mut connection = server.accept_connection().await;
    let responder = tokio::spawn(async move {
        respond_authenticate(&mut connection).await;

        let query_headsets = connection
            .recv_request_method(Methods::QUERY_HEADSETS)
            .await;
        connection
            .send_result(rpc_id(&query_headsets), headset_response())
            .await;

        let create_session = connection
            .recv_request_method(Methods::CREATE_SESSION)
            .await;
        connection
            .send_result(rpc_id(&create_session), session_response())
            .await;

        let subscribe = connection.recv_request_method(Methods::SUBSCRIBE).await;
        assert_eq!(subscribe["params"]["streams"], json!([Streams::MOT]));
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
                        "sid": SESSION_ID,
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
                    0.735341,
                    0.255615,
                    0.627441,
                    -0.015869,
                    0.948257,
                    -0.354986,
                    -0.083497,
                    -44.656766,
                    -86.970985,
                    23.221568
                ],
                "sid": SESSION_ID,
                "time": 1590402244.8242
            }))
            .await;

        let close_session = connection
            .recv_request_method(Methods::UPDATE_SESSION)
            .await;
        connection
            .send_result(rpc_id(&close_session), json!({"message": "Session closed"}))
            .await;
    });

    let token = client.authenticate(CLIENT_ID, CLIENT_SECRET).await.unwrap();
    let headsets = client
        .query_headsets(QueryHeadsetsOptions::default())
        .await
        .unwrap();
    let headset = headsets.first().unwrap();
    let session = client.create_session(&token, &headset.id).await.unwrap();
    let mut motion_stream = streams::subscribe_motion(&client, &token, &session.id)
        .await
        .unwrap();
    let motion = tokio::time::timeout(std::time::Duration::from_secs(2), motion_stream.next())
        .await
        .expect("timed out waiting for motion sample")
        .expect("motion stream ended unexpectedly");
    client.close_session(&token, &session.id).await.unwrap();

    responder.await.unwrap();

    assert!(motion.quaternion.is_some());
    assert!((motion.accelerometer[0] - 0.948257).abs() < f32::EPSILON);

    client.disconnect().await.unwrap();
}

#[tokio::test]
async fn performance_metrics_example_workflow_matches_documented_sample() {
    let mut server = match start_server_or_skip(
        "performance_metrics_example_workflow_matches_documented_sample",
    )
    .await
    {
        Some(server) => server,
        None => return,
    };
    let config = test_config(server.ws_url());
    let mut client = CortexClient::connect(&config).await.unwrap();

    let mut connection = server.accept_connection().await;
    let responder = tokio::spawn(async move {
        respond_authenticate(&mut connection).await;

        let query_headsets = connection
            .recv_request_method(Methods::QUERY_HEADSETS)
            .await;
        connection
            .send_result(rpc_id(&query_headsets), headset_response())
            .await;

        let create_session = connection
            .recv_request_method(Methods::CREATE_SESSION)
            .await;
        connection
            .send_result(rpc_id(&create_session), session_response())
            .await;

        let subscribe = connection.recv_request_method(Methods::SUBSCRIBE).await;
        assert_eq!(subscribe["params"]["streams"], json!([Streams::MET]));
        connection
            .send_result(
                rpc_id(&subscribe),
                json!({
                    "success": [{
                        "cols": [
                            "eng.isActive",
                            "eng",
                            "exc.isActive",
                            "exc",
                            "lex",
                            "str.isActive",
                            "str",
                            "rel.isActive",
                            "rel",
                            "int.isActive",
                            "int",
                            "attention.isActive",
                            "attention"
                        ],
                        "sid": SESSION_ID,
                        "streamName": Streams::MET
                    }],
                    "failure": []
                }),
            )
            .await;
        connection
            .push_event(json!({
                "met": [false, null, false, null, null, false, null, true, 0.266589, false, null, true, 0.098421],
                "sid": SESSION_ID,
                "time": 1559903137.1741
            }))
            .await;

        let close_session = connection
            .recv_request_method(Methods::UPDATE_SESSION)
            .await;
        connection
            .send_result(rpc_id(&close_session), json!({"message": "Session closed"}))
            .await;
    });

    let token = client.authenticate(CLIENT_ID, CLIENT_SECRET).await.unwrap();
    let headsets = client
        .query_headsets(QueryHeadsetsOptions::default())
        .await
        .unwrap();
    let headset = headsets.first().unwrap();
    let session = client.create_session(&token, &headset.id).await.unwrap();
    let mut metrics_stream = streams::subscribe_metrics(&client, &token, &session.id)
        .await
        .unwrap();
    let metrics = tokio::time::timeout(std::time::Duration::from_secs(2), metrics_stream.next())
        .await
        .expect("timed out waiting for metrics sample")
        .expect("metrics stream ended unexpectedly");
    client.close_session(&token, &session.id).await.unwrap();

    responder.await.unwrap();

    assert_eq!(metrics.engagement, None);
    assert_eq!(metrics.relaxation, Some(0.266589));
    assert_eq!(metrics.attention, Some(0.098421));

    client.disconnect().await.unwrap();
}

#[tokio::test]
async fn mental_commands_example_workflow_matches_documented_sample() {
    let mut server =
        match start_server_or_skip("mental_commands_example_workflow_matches_documented_sample")
            .await
        {
            Some(server) => server,
            None => return,
        };
    let config = test_config(server.ws_url());
    let mut client = CortexClient::connect(&config).await.unwrap();

    let mut connection = server.accept_connection().await;
    let responder = tokio::spawn(async move {
        respond_authenticate(&mut connection).await;

        let query_headsets = connection
            .recv_request_method(Methods::QUERY_HEADSETS)
            .await;
        connection
            .send_result(rpc_id(&query_headsets), headset_response())
            .await;

        let get_detection_info = connection
            .recv_request_method(Methods::GET_DETECTION_INFO)
            .await;
        assert_eq!(
            get_detection_info["params"]["detection"],
            DetectionType::MentalCommand.as_str()
        );
        connection
            .send_result(
                rpc_id(&get_detection_info),
                json!({
                    "actions": ["neutral", "push", "pull"],
                    "controls": ["start", "accept", "reject"],
                    "events": ["MC_Started", "MC_Succeeded"]
                }),
            )
            .await;

        let query_profiles = connection.recv_request_method(Methods::QUERY_PROFILE).await;
        connection
            .send_result(
                rpc_id(&query_profiles),
                json!([{
                    "uuid": "profile-1",
                    "name": PROFILE_NAME,
                    "readOnly": false,
                    "eegChannels": ["AF3", "T7", "Pz", "T8", "AF4"]
                }]),
            )
            .await;

        let setup_profile = connection.recv_request_method(Methods::SETUP_PROFILE).await;
        assert_eq!(setup_profile["params"]["profile"], PROFILE_NAME);
        assert_eq!(
            setup_profile["params"]["status"],
            ProfileAction::Load.as_str()
        );
        connection
            .send_result(rpc_id(&setup_profile), json!({"message": "Profile loaded"}))
            .await;

        let create_session = connection
            .recv_request_method(Methods::CREATE_SESSION)
            .await;
        connection
            .send_result(rpc_id(&create_session), session_response())
            .await;

        let subscribe = connection.recv_request_method(Methods::SUBSCRIBE).await;
        assert_eq!(subscribe["params"]["streams"], json!([Streams::COM]));
        connection
            .send_result(
                rpc_id(&subscribe),
                json!({
                    "success": [{
                        "cols": ["act", "pow"],
                        "sid": SESSION_ID,
                        "streamName": Streams::COM
                    }],
                    "failure": []
                }),
            )
            .await;
        connection
            .push_event(json!({
                "com": ["pull", 0.564],
                "sid": SESSION_ID,
                "time": 1559903099.348
            }))
            .await;

        let close_session = connection
            .recv_request_method(Methods::UPDATE_SESSION)
            .await;
        connection
            .send_result(rpc_id(&close_session), json!({"message": "Session closed"}))
            .await;
    });

    let token = client.authenticate(CLIENT_ID, CLIENT_SECRET).await.unwrap();
    let headsets = client
        .query_headsets(QueryHeadsetsOptions::default())
        .await
        .unwrap();
    let headset = headsets.first().unwrap();
    let detection_info = client
        .get_detection_info(DetectionType::MentalCommand)
        .await
        .unwrap();
    let profiles = client.query_profiles(&token).await.unwrap();
    client
        .setup_profile(&token, &headset.id, &profiles[0].name, ProfileAction::Load)
        .await
        .unwrap();
    let session = client.create_session(&token, &headset.id).await.unwrap();
    let mut commands = streams::subscribe_mental_commands(&client, &token, &session.id)
        .await
        .unwrap();
    let command = tokio::time::timeout(std::time::Duration::from_secs(2), commands.next())
        .await
        .expect("timed out waiting for mental-command sample")
        .expect("mental-command stream ended unexpectedly");
    client.close_session(&token, &session.id).await.unwrap();

    responder.await.unwrap();

    assert_eq!(detection_info.actions, vec!["neutral", "push", "pull"]);
    assert_eq!(profiles[0].name, PROFILE_NAME);
    assert_eq!(command.action, "pull");
    assert!((command.power - 0.564).abs() < f32::EPSILON);

    client.disconnect().await.unwrap();
}
