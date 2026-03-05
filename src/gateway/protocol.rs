use serde::{Deserialize, Serialize};
use serde_json::Value;

// --- Outgoing Frames (client -> server) ---

/// Request frame: {"type":"req","id":"<uuid>","method":"...","params":{...}}
#[derive(Debug, Serialize)]
pub struct RequestFrame {
    #[serde(rename = "type")]
    pub frame_type: &'static str,
    pub id: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

impl RequestFrame {
    pub fn new(method: &str, params: Option<Value>) -> Self {
        Self {
            frame_type: "req",
            id: uuid_v4(),
            method: method.to_string(),
            params,
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

/// Generate a v4 UUID string using rand (avoids adding uuid crate)
fn uuid_v4() -> String {
    let bytes: [u8; 16] = rand::random();
    // Set version (4) and variant (RFC 4122)
    let b6 = (bytes[6] & 0x0F) | 0x40; // version 4
    let b8 = (bytes[8] & 0x3F) | 0x80; // variant 1
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0], bytes[1], bytes[2], bytes[3],
        bytes[4], bytes[5],
        b6, bytes[7],
        b8, bytes[9],
        bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15]
    )
}

// --- Incoming Frames (server -> client) ---

/// Raw deserialized frame — we discriminate on `type` field
#[derive(Debug, Deserialize)]
struct RawFrame {
    #[serde(rename = "type")]
    frame_type: Option<String>,
    // Response fields
    id: Option<String>,
    ok: Option<bool>,
    payload: Option<Value>,
    error: Option<RpcError>,
    // Event fields
    event: Option<String>,
    #[allow(dead_code)]
    seq: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct RpcError {
    #[allow(dead_code)]
    pub code: Option<String>,
    pub message: Option<String>,
}

/// Parsed incoming frame types
#[derive(Debug)]
pub enum IncomingFrame {
    /// connect.challenge event from server
    ConnectChallenge { nonce: String },
    /// Successful hello-ok response to our connect request
    HelloOk {
        conn_id: String,
        device_token: Option<String>,
    },
    /// Chat streaming delta (no `state` field in payload)
    ChatDelta { text: String },
    /// Chat final message (`state: "final"`)
    ChatComplete { text: String },
    /// Chat error (`state: "error"`)
    ChatError { message: String },
    /// Generic successful response to a request
    Response { id: String, payload: Value },
    /// Error response to a request
    ErrorResponse {
        id: String,
        code: String,
        message: String,
    },
    /// Mood update (if gateway sends mood.update events)
    MoodUpdate(crate::mood::MoodUpdate),
    /// Unknown / unhandled frame
    Unknown(String),
}

impl IncomingFrame {
    pub fn parse(text: &str) -> Self {
        let Ok(raw) = serde_json::from_str::<RawFrame>(text) else {
            return IncomingFrame::Unknown(text.to_string());
        };

        match raw.frame_type.as_deref() {
            Some("event") => Self::parse_event(raw, text),
            Some("res") => Self::parse_response(raw, text),
            _ => IncomingFrame::Unknown(text.to_string()),
        }
    }

    fn parse_event(raw: RawFrame, original: &str) -> Self {
        let payload = raw.payload.as_ref();

        match raw.event.as_deref() {
            Some("connect.challenge") => {
                let nonce = payload
                    .and_then(|p| p.get("nonce"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                IncomingFrame::ConnectChallenge { nonce }
            }
            Some("chat") => Self::parse_chat_event(payload),
            Some("mood.update") => {
                if let Some(p) = payload {
                    match serde_json::from_value::<crate::mood::MoodUpdate>(p.clone()) {
                        Ok(update) => return IncomingFrame::MoodUpdate(update),
                        Err(e) => {
                            eprintln!("[protocol] malformed mood.update: {e}");
                        }
                    }
                }
                IncomingFrame::Unknown(original.to_string())
            }
            _ => IncomingFrame::Unknown(original.to_string()),
        }
    }

    fn parse_chat_event(payload: Option<&Value>) -> Self {
        let Some(p) = payload else {
            return IncomingFrame::Unknown("chat event with no payload".to_string());
        };

        let state = p.get("state").and_then(|v| v.as_str());

        match state {
            Some("error") => {
                let msg = p
                    .get("errorMessage")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown error")
                    .to_string();
                IncomingFrame::ChatError { message: msg }
            }
            Some("final") => {
                let text = extract_message_text(p);
                IncomingFrame::ChatComplete { text }
            }
            // No state field = streaming delta
            None => {
                let text = extract_message_text(p);
                IncomingFrame::ChatDelta { text }
            }
            _ => IncomingFrame::Unknown(format!("chat event with unknown state: {:?}", state)),
        }
    }

    fn parse_response(raw: RawFrame, original: &str) -> Self {
        let id = raw.id.unwrap_or_default();

        if raw.ok == Some(false) {
            let err = raw.error;
            return IncomingFrame::ErrorResponse {
                id,
                code: err
                    .as_ref()
                    .and_then(|e| e.code.clone())
                    .unwrap_or_else(|| "UNKNOWN".to_string()),
                message: err
                    .and_then(|e| e.message)
                    .unwrap_or_else(|| "unknown error".to_string()),
            };
        }

        if let Some(ref payload) = raw.payload {
            // Check for hello-ok response
            if payload.get("type").and_then(|t| t.as_str()) == Some("hello-ok") {
                let conn_id = payload
                    .get("server")
                    .and_then(|s| s.get("connId"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let device_token = payload
                    .get("auth")
                    .and_then(|a| a.get("deviceToken"))
                    .and_then(|v| v.as_str())
                    .map(String::from);
                return IncomingFrame::HelloOk {
                    conn_id,
                    device_token,
                };
            }

            return IncomingFrame::Response {
                id,
                payload: payload.clone(),
            };
        }

        IncomingFrame::Unknown(original.to_string())
    }
}

/// Extract text content from a chat event message payload
/// Message structure: { message: { content: [{ type: "text", text: "..." }] } }
fn extract_message_text(payload: &Value) -> String {
    payload
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_array())
        .and_then(|arr| {
            arr.iter()
                .filter_map(|item| {
                    if item.get("type").and_then(|t| t.as_str()) == Some("text") {
                        item.get("text").and_then(|t| t.as_str()).map(String::from)
                    } else {
                        None
                    }
                })
                .next()
        })
        .unwrap_or_default()
}

/// Build `connect` request params
pub fn build_connect_params(
    token: Option<&str>,
    device: Option<ConnectDevice>,
) -> Value {
    let auth = token.map(|t| {
        serde_json::json!({
            "token": t,
        })
    });

    let mut params = serde_json::json!({
        "minProtocol": 3,
        "maxProtocol": 3,
        "client": {
            "id": "cli",
            "displayName": "Matrix Rain TUI",
            "version": env!("CARGO_PKG_VERSION"),
            "platform": std::env::consts::OS,
            "mode": "ui"
        },
        "caps": [],
        "role": "operator",
        "scopes": ["operator.admin"],
    });

    if let Some(auth_val) = auth {
        params["auth"] = auth_val;
    }
    if let Some(dev) = device {
        params["device"] = serde_json::json!({
            "id": dev.id,
            "publicKey": dev.public_key_base64url,
            "signature": dev.signature_base64url,
            "signedAt": dev.signed_at_ms,
            "nonce": dev.nonce,
        });
    }

    params
}

/// Device identity fields for the connect request
pub struct ConnectDevice {
    pub id: String,
    pub public_key_base64url: String,
    pub signature_base64url: String,
    pub signed_at_ms: u64,
    pub nonce: String,
}

/// Build chat.send request params
pub fn build_chat_send(message: &str, session_key: &str) -> Value {
    serde_json::json!({
        "sessionKey": session_key,
        "message": message,
        "idempotencyKey": uuid_v4(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_frame_format() {
        let frame = RequestFrame::new("connect", Some(serde_json::json!({"test": true})));
        let json = frame.to_json();
        let parsed: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["type"], "req");
        assert!(parsed["id"].as_str().unwrap().contains('-')); // UUID format
        assert_eq!(parsed["method"], "connect");
    }

    #[test]
    fn parse_connect_challenge_event() {
        let text = r#"{"type":"event","event":"connect.challenge","payload":{"nonce":"abc-123","ts":1234567890}}"#;
        match IncomingFrame::parse(text) {
            IncomingFrame::ConnectChallenge { nonce } => assert_eq!(nonce, "abc-123"),
            other => panic!("expected ConnectChallenge, got {:?}", other),
        }
    }

    #[test]
    fn parse_hello_ok_response() {
        let text = r#"{"type":"res","id":"req-1","ok":true,"payload":{"type":"hello-ok","protocol":3,"server":{"version":"2026.2.24","connId":"conn-abc"},"auth":{"deviceToken":"dt-xyz","role":"operator"}}}"#;
        match IncomingFrame::parse(text) {
            IncomingFrame::HelloOk {
                conn_id,
                device_token,
            } => {
                assert_eq!(conn_id, "conn-abc");
                assert_eq!(device_token.as_deref(), Some("dt-xyz"));
            }
            other => panic!("expected HelloOk, got {:?}", other),
        }
    }

    #[test]
    fn parse_chat_delta_event() {
        let text = r#"{"type":"event","event":"chat","payload":{"runId":"run-1","sessionKey":"sk","seq":1,"message":{"role":"assistant","content":[{"type":"text","text":"Hello "}],"timestamp":12345}}}"#;
        match IncomingFrame::parse(text) {
            IncomingFrame::ChatDelta { text } => assert_eq!(text, "Hello "),
            other => panic!("expected ChatDelta, got {:?}", other),
        }
    }

    #[test]
    fn parse_chat_final_event() {
        let text = r#"{"type":"event","event":"chat","payload":{"runId":"run-1","sessionKey":"sk","seq":2,"state":"final","message":{"role":"assistant","content":[{"type":"text","text":"Hello world!"}],"timestamp":12345}}}"#;
        match IncomingFrame::parse(text) {
            IncomingFrame::ChatComplete { text } => assert_eq!(text, "Hello world!"),
            other => panic!("expected ChatComplete, got {:?}", other),
        }
    }

    #[test]
    fn parse_chat_error_event() {
        let text = r#"{"type":"event","event":"chat","payload":{"runId":"run-1","sessionKey":"sk","seq":3,"state":"error","errorMessage":"rate limited"}}"#;
        match IncomingFrame::parse(text) {
            IncomingFrame::ChatError { message } => assert_eq!(message, "rate limited"),
            other => panic!("expected ChatError, got {:?}", other),
        }
    }

    #[test]
    fn parse_error_response() {
        let text = r#"{"type":"res","id":"req-2","ok":false,"error":{"code":"AUTH_FAILED","message":"invalid token"}}"#;
        match IncomingFrame::parse(text) {
            IncomingFrame::ErrorResponse {
                code, message, ..
            } => {
                assert_eq!(code, "AUTH_FAILED");
                assert_eq!(message, "invalid token");
            }
            other => panic!("expected ErrorResponse, got {:?}", other),
        }
    }

    #[test]
    fn uuid_v4_format() {
        let id = uuid_v4();
        assert_eq!(id.len(), 36);
        assert_eq!(&id[8..9], "-");
        assert_eq!(&id[13..14], "-");
        assert_eq!(&id[18..19], "-");
        assert_eq!(&id[23..24], "-");
        // Version nibble should be '4'
        assert_eq!(&id[14..15], "4");
    }
}
