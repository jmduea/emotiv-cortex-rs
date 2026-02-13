#![allow(dead_code)]

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::timeout;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;

pub const STEP_TIMEOUT: Duration = Duration::from_secs(3);

enum ConnectionCommand {
    SendJson(Value),
    ForceClose,
}

pub struct MockConnection {
    index: usize,
    request_rx: mpsc::Receiver<Value>,
    command_tx: mpsc::Sender<ConnectionCommand>,
}

impl MockConnection {
    pub fn index(&self) -> usize {
        self.index
    }

    pub async fn recv_request(&mut self) -> Value {
        timeout(STEP_TIMEOUT, self.request_rx.recv())
            .await
            .expect("timed out waiting for request")
            .expect("mock connection request channel closed")
    }

    pub async fn recv_request_method(&mut self, expected_method: &str) -> Value {
        let request = self.recv_request().await;
        let method = request.get("method").and_then(Value::as_str);
        assert_eq!(method, Some(expected_method), "unexpected method request");
        request
    }

    pub async fn send_json(&self, value: Value) {
        self.command_tx
            .send(ConnectionCommand::SendJson(value))
            .await
            .expect("failed to send command to mock connection");
    }

    pub async fn send_result(&self, id: u64, result: Value) {
        self.send_json(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": result,
        }))
        .await;
    }

    pub async fn send_error(&self, id: u64, code: i32, message: &str) {
        self.send_json(json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {
                "code": code,
                "message": message,
            }
        }))
        .await;
    }

    pub async fn push_event(&self, event: Value) {
        self.send_json(event).await;
    }

    pub async fn force_close(&self) {
        let _ = self.command_tx.send(ConnectionCommand::ForceClose).await;
    }
}

pub struct MockCortexServer {
    addr: SocketAddr,
    connection_rx: mpsc::Receiver<MockConnection>,
    server_task: JoinHandle<()>,
}

impl MockCortexServer {
    pub async fn start() -> std::io::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let addr = listener.local_addr()?;
        let (connection_tx, connection_rx) = mpsc::channel(16);
        let next_connection_index = Arc::new(AtomicUsize::new(0));

        let server_task = tokio::spawn(async move {
            loop {
                let (stream, _) = match listener.accept().await {
                    Ok(pair) => pair,
                    Err(_) => break,
                };

                let connection_tx = connection_tx.clone();
                let connection_index = next_connection_index.fetch_add(1, Ordering::SeqCst);

                tokio::spawn(async move {
                    let ws_stream = match accept_async(stream).await {
                        Ok(ws) => ws,
                        Err(_) => return,
                    };

                    let (mut ws_sink, mut ws_source) = ws_stream.split();
                    let (request_tx, request_rx) = mpsc::channel(64);
                    let (command_tx, mut command_rx) = mpsc::channel(64);

                    let connection = MockConnection {
                        index: connection_index,
                        request_rx,
                        command_tx: command_tx.clone(),
                    };

                    if connection_tx.send(connection).await.is_err() {
                        return;
                    }

                    loop {
                        tokio::select! {
                            maybe_command = command_rx.recv() => {
                                match maybe_command {
                                    Some(ConnectionCommand::SendJson(value)) => {
                                        let message = Message::Text(value.to_string().into());
                                        if ws_sink.send(message).await.is_err() {
                                            break;
                                        }
                                    }
                                    Some(ConnectionCommand::ForceClose) => {
                                        break;
                                    }
                                    None => break,
                                }
                            }
                            maybe_message = ws_source.next() => {
                                match maybe_message {
                                    Some(Ok(Message::Text(text))) => {
                                        if let Ok(value) = serde_json::from_str::<Value>(&text) {
                                            let _ = request_tx.send(value).await;
                                        }
                                    }
                                    Some(Ok(Message::Close(_))) => break,
                                    Some(Ok(_)) => {}
                                    Some(Err(_)) => break,
                                    None => break,
                                }
                            }
                        }
                    }
                });
            }
        });

        Ok(Self {
            addr,
            connection_rx,
            server_task,
        })
    }

    pub fn ws_url(&self) -> String {
        format!("ws://{}", self.addr)
    }

    pub async fn accept_connection(&mut self) -> MockConnection {
        timeout(STEP_TIMEOUT, self.connection_rx.recv())
            .await
            .expect("timed out waiting for client connection")
            .expect("mock server connection channel closed")
    }

    pub async fn try_accept_connection(&mut self, wait: Duration) -> Option<MockConnection> {
        match timeout(wait, self.connection_rx.recv()).await {
            Ok(Some(connection)) => Some(connection),
            _ => None,
        }
    }
}

impl Drop for MockCortexServer {
    fn drop(&mut self) {
        self.server_task.abort();
    }
}
