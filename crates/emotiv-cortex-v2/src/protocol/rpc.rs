//! JSON-RPC request/response protocol structures.

use serde::{Deserialize, Serialize};

/// A JSON-RPC 2.0 request to the Cortex API.
#[derive(Debug, Serialize)]
pub struct CortexRequest {
    pub id: u64,
    pub jsonrpc: &'static str,
    pub method: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    // Using `serde_json::Value` allows us to flexibly construct params for different methods without needing a separate struct for each method's parameters.
    pub params: Option<serde_json::Value>,
}

impl CortexRequest {
    /// Create a new request with the given method and params.
    pub fn new(id: u64, method: &'static str, params: serde_json::Value) -> Self {
        let params = if params.is_object() && params.as_object().is_some_and(|m| m.is_empty()) {
            None
        } else {
            Some(params)
        };

        Self {
            jsonrpc: "2.0",
            id,
            method,
            params,
        }
    }
}

/// A JSON-RPC 2.0 response from the Cortex API.
#[derive(Debug, Deserialize)]
pub struct CortexResponse {
    pub id: Option<u64>,
    pub result: Option<serde_json::Value>,
    pub error: Option<RpcError>,
}

/// A JSON-RPC 2.0 error payload from the Cortex API.
///
/// This is the raw error object from the protocol. Use
/// [`CortexError::from_api_error`](crate::CortexError::from_api_error)
/// to convert to a semantic error type.
#[derive(Debug, Clone, Deserialize)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
}

impl std::fmt::Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Cortex API error {}: {}", self.code, self.message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::constants::{ErrorCodes, Methods};

    #[test]
    fn test_serialize_request_no_params() {
        // Empty params should be omitted entirely (matching official Cortex examples)
        let req = CortexRequest::new(1, Methods::QUERY_HEADSETS, serde_json::json!({}));

        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"method\":\"queryHeadsets\""));
        assert!(
            !json.contains("\"params\""),
            "empty params should be omitted: {}",
            json
        );
    }

    #[test]
    fn test_serialize_request_with_params() {
        let req = CortexRequest::new(
            1,
            Methods::AUTHORIZE,
            serde_json::json!({"clientId": "abc", "clientSecret": "xyz"}),
        );

        let json = serde_json::to_string(&req).unwrap();
        assert!(
            json.contains("\"params\""),
            "non-empty params should be present: {}",
            json
        );
        assert!(json.contains("\"clientId\":\"abc\""));
    }

    #[test]
    fn test_deserialize_rpc_error() {
        let json = r#"{
            "id": 1,
            "error": {
                "code": -32002,
                "message": "Access denied"
            }
        }"#;

        let resp: CortexResponse = serde_json::from_str(json).unwrap();
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, ErrorCodes::ACCESS_DENIED);
    }
}
