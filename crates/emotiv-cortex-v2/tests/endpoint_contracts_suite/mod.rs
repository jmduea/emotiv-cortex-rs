use emotiv_cortex_v2::protocol::constants::Methods;
use emotiv_cortex_v2::{CortexClient, CortexConfig, ResilientClient};
use serde_json::json;

use crate::support::mock_cortex::{MockConnection, MockCortexServer};

mod assertions;
mod executors;
mod fixtures;

use assertions::{assert_request_matches_step, rpc_id};
use executors::{execute_cortex_step, execute_resilient_step};
use fixtures::{TOKEN_CORTEX, TOKEN_RESILIENT, build_contract_steps};

fn cortex_contract_config(url: String) -> CortexConfig {
    let mut config = CortexConfig::new("test-client-id", "test-client-secret");
    config.cortex_url = url;
    config.timeouts.rpc_timeout_secs = 2;
    config
}

fn resilient_contract_config(url: String) -> CortexConfig {
    let mut config = CortexConfig::new("test-client-id", "test-client-secret");
    config.cortex_url = url;
    config.reconnect.enabled = false;
    config.health.enabled = false;
    config.timeouts.rpc_timeout_secs = 2;
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

async fn drive_auth_handshake(connection: &mut MockConnection, token: &str) {
    let info = connection
        .recv_request_method(Methods::GET_CORTEX_INFO)
        .await;
    connection
        .send_result(rpc_id(&info), json!({"version": "mock"}))
        .await;

    let request_access = connection
        .recv_request_method(Methods::REQUEST_ACCESS)
        .await;
    connection
        .send_result(rpc_id(&request_access), json!({"accessGranted": true}))
        .await;

    let authorize = connection.recv_request_method(Methods::AUTHORIZE).await;
    connection
        .send_result(rpc_id(&authorize), json!({"cortexToken": token}))
        .await;
}

#[tokio::test]
async fn cortex_client_endpoint_contracts_table_driven() {
    let mut server =
        match start_server_or_skip("cortex_client_endpoint_contracts_table_driven").await {
            Some(server) => server,
            None => return,
        };

    let steps = build_contract_steps(TOKEN_CORTEX);
    let server_steps = steps.clone();
    let url = server.ws_url();
    let server_task = tokio::spawn(async move {
        let mut connection = server.accept_connection().await;
        for step in server_steps {
            let request = connection.recv_request().await;
            assert_request_matches_step(&request, &step);
            connection
                .send_result(rpc_id(&request), step.response.clone())
                .await;
        }
    });

    let config = cortex_contract_config(url);
    let mut client = CortexClient::connect(&config).await.unwrap();

    for step in &steps {
        execute_cortex_step(&client, &step.kind).await;
    }

    server_task.await.unwrap();
    client.disconnect().await.unwrap();
}

#[tokio::test]
async fn resilient_client_endpoint_contracts_table_driven() {
    let mut server =
        match start_server_or_skip("resilient_client_endpoint_contracts_table_driven").await {
            Some(server) => server,
            None => return,
        };

    let steps = build_contract_steps(TOKEN_RESILIENT);
    let server_steps = steps.clone();
    let url = server.ws_url();
    let server_task = tokio::spawn(async move {
        let mut connection = server.accept_connection().await;
        drive_auth_handshake(&mut connection, TOKEN_RESILIENT).await;

        for step in server_steps {
            let request = connection.recv_request().await;
            assert_request_matches_step(&request, &step);
            connection
                .send_result(rpc_id(&request), step.response.clone())
                .await;
        }
    });

    let config = resilient_contract_config(url);
    let client = ResilientClient::connect(config).await.unwrap();
    assert_eq!(client.cortex_token().await, TOKEN_RESILIENT);

    for step in &steps {
        execute_resilient_step(&client, &step.kind).await;
    }

    client.disconnect().await.unwrap();
    server_task.await.unwrap();
}
