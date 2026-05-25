#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// JSON-RPC 2.0 Request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub jsonrpc: String,
    pub id: RequestId,
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

/// JSON-RPC 2.0 Response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub jsonrpc: String,
    pub id: RequestId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

/// JSON-RPC 2.0 Notification (no id)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

/// JSON-RPC error object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// Request ID can be a number or string
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(untagged)]
pub enum RequestId {
    Number(u64),
    String(String),
}

/// Any message that can arrive over the wire
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Message {
    Request(Request),
    Response(Response),
    Notification(Notification),
}

// Standard JSON-RPC error codes
pub const PARSE_ERROR: i32 = -32700;
pub const INVALID_REQUEST: i32 = -32600;
pub const METHOD_NOT_FOUND: i32 = -32601;
pub const INVALID_PARAMS: i32 = -32602;
pub const INTERNAL_ERROR: i32 = -32603;

impl Request {
    pub fn new(id: u64, method: &str, params: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_owned(),
            id: RequestId::Number(id),
            method: method.to_owned(),
            params,
        }
    }
}

impl Response {
    pub fn success(id: RequestId, result: Value) -> Self {
        Self { jsonrpc: "2.0".to_owned(), id, result: Some(result), error: None }
    }

    pub fn error(id: RequestId, code: i32, message: &str) -> Self {
        Self {
            jsonrpc: "2.0".to_owned(),
            id,
            result: None,
            error: Some(RpcError { code, message: message.to_owned(), data: None }),
        }
    }
}

impl Notification {
    pub fn new(method: &str, params: Option<Value>) -> Self {
        Self { jsonrpc: "2.0".to_owned(), method: method.to_owned(), params }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_serialize_request() {
        let req = Request::new(1, "initialize", Some(json!({"clientInfo": {"name": "aintegrix"}})));
        let s = serde_json::to_string(&req).unwrap();
        assert!(s.contains("\"jsonrpc\":\"2.0\""));
        assert!(s.contains("\"id\":1"));
        assert!(s.contains("\"method\":\"initialize\""));
    }

    #[test]
    fn test_serialize_response_success() {
        let resp = Response::success(RequestId::Number(1), json!({"session_id": "abc"}));
        let s = serde_json::to_string(&resp).unwrap();
        assert!(s.contains("\"result\""));
        assert!(!s.contains("\"error\""));
    }

    #[test]
    fn test_serialize_response_error() {
        let resp = Response::error(RequestId::Number(1), METHOD_NOT_FOUND, "not found");
        let s = serde_json::to_string(&resp).unwrap();
        assert!(s.contains("\"error\""));
        assert!(s.contains("-32601"));
    }

    #[test]
    fn test_deserialize_notification() {
        let json_str = r#"{"jsonrpc":"2.0","method":"session/update","params":{"type":"text"}}"#;
        let notif: Notification = serde_json::from_str(json_str).unwrap();
        assert_eq!(notif.method, "session/update");
    }

    #[test]
    fn test_request_id_string() {
        let req = Request {
            jsonrpc: "2.0".to_owned(),
            id: RequestId::String("req-42".to_owned()),
            method: "test".to_owned(),
            params: None,
        };
        let s = serde_json::to_string(&req).unwrap();
        assert!(s.contains("\"req-42\""));
    }

    #[test]
    fn test_roundtrip_message() {
        let req = Request::new(5, "session/prompt", Some(json!({"messages": []})));
        let serialized = serde_json::to_string(&req).unwrap();
        let deserialized: Request = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.method, "session/prompt");
        assert_eq!(deserialized.id, RequestId::Number(5));
    }
}
