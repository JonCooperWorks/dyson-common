//! JSON-RPC 2.0 base envelope types shared across the MCP boundary.
//!
//! Only the transport envelope lives here — request / notification / response /
//! error. The MCP-specific payloads (`initialize`, `tools/list`, tool result
//! content) stay in dyson, which fully types them; swarm's MCP proxy forwards
//! opaque `serde_json::Value` bodies and only needs the envelope shape.
//!
//! `JsonRpcResponse` / `JsonRpcError` derive both `Deserialize` and `Serialize`
//! because the same types are parsed from remote servers and produced when
//! acting as a server.

use serde::{Deserialize, Serialize};

/// A JSON-RPC 2.0 request.
#[derive(Debug, Serialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: &'static str,
    pub id: u64,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

impl JsonRpcRequest {
    #[must_use]
    pub fn new(id: u64, method: &str, params: Option<serde_json::Value>) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            method: method.to_string(),
            params,
        }
    }
}

/// A JSON-RPC 2.0 notification (no id, no response expected).
#[derive(Debug, Serialize)]
pub struct JsonRpcNotification {
    pub jsonrpc: &'static str,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

impl JsonRpcNotification {
    #[must_use]
    pub fn new(method: &str, params: Option<serde_json::Value>) -> Self {
        Self {
            jsonrpc: "2.0",
            method: method.to_string(),
            params,
        }
    }
}

/// A JSON-RPC 2.0 response (success or error).
#[derive(Debug, Deserialize, Serialize)]
pub struct JsonRpcResponse {
    pub id: Option<u64>,
    pub result: Option<serde_json::Value>,
    pub error: Option<JsonRpcError>,
}

/// A JSON-RPC 2.0 error object, nested inside `JsonRpcResponse.error`.
///
/// Standard error codes: `-32700` parse error, `-32601` method not found,
/// `-32602` invalid params, `-32603` internal error.
#[derive(Debug, Deserialize, Serialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_omits_none_params_and_pins_version() {
        let r = JsonRpcRequest::new(1, "tools/list", None);
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains(r#""jsonrpc":"2.0""#));
        assert!(!json.contains("params"));
    }

    #[test]
    fn response_round_trips_error() {
        let json = r#"{"id":2,"result":null,"error":{"code":-32601,"message":"nope","data":null}}"#;
        let resp: JsonRpcResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.id, Some(2));
        assert_eq!(resp.error.as_ref().unwrap().code, -32601);
    }
}
