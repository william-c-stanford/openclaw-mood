use serde::{Deserialize, Serialize};
use serde_json::Value;

/// JSON-RPC v3 request frame
#[derive(Debug, Serialize)]
pub struct RequestFrame {
    pub jsonrpc: &'static str,
    pub id: u64,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

impl RequestFrame {
    pub fn new(id: u64, method: &str, params: Option<Value>) -> Self {
        Self {
            jsonrpc: "3.0",
            id,
            method: method.to_string(),
            params,
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

/// JSON-RPC v3 response/notification frame
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct ResponseFrame {
    pub jsonrpc: Option<String>,
    pub id: Option<u64>,
    pub method: Option<String>,
    pub result: Option<Value>,
    pub params: Option<Value>,
    pub error: Option<RpcError>,
}

#[derive(Debug, Deserialize)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
}

/// Parsed incoming frame types
#[derive(Debug)]
#[allow(dead_code)]
pub enum IncomingFrame {
    /// Challenge from server during handshake
    Challenge { challenge: String },
    /// Hello/welcome after auth
    Hello,
    /// Chat message delta (streaming token)
    ChatDelta { delta: String },
    /// Chat message complete
    ChatComplete { content: String },
    /// Error response
    Error { code: i64, message: String },
    /// Mood update from agent
    MoodUpdate(crate::mood::MoodUpdate),
    /// Unknown frame
    Unknown(String),
}

impl IncomingFrame {
    pub fn parse(text: &str) -> Self {
        let Ok(frame) = serde_json::from_str::<ResponseFrame>(text) else {
            return IncomingFrame::Unknown(text.to_string());
        };

        // Check for error
        if let Some(err) = frame.error {
            return IncomingFrame::Error {
                code: err.code,
                message: err.message,
            };
        }

        // Check method-based notifications
        if let Some(method) = &frame.method {
            match method.as_str() {
                "auth.challenge" => {
                    let challenge = frame
                        .params
                        .as_ref()
                        .and_then(|p| p.get("challenge"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    return IncomingFrame::Challenge { challenge };
                }
                "auth.hello" => return IncomingFrame::Hello,
                "chat.delta" => {
                    let delta = frame
                        .params
                        .as_ref()
                        .and_then(|p| p.get("delta"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    return IncomingFrame::ChatDelta { delta };
                }
                "chat.complete" => {
                    let content = frame
                        .params
                        .as_ref()
                        .and_then(|p| p.get("content"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    return IncomingFrame::ChatComplete { content };
                }
                "mood.update" => {
                    if let Some(params) = frame.params {
                        match serde_json::from_value::<crate::mood::MoodUpdate>(params.clone()) {
                            Ok(update) => return IncomingFrame::MoodUpdate(update),
                            Err(e) => {
                                eprintln!("[protocol] malformed mood.update: {e} — raw: {params}");
                            }
                        }
                    }
                    return IncomingFrame::Unknown(text.to_string());
                }
                _ => {}
            }
        }

        IncomingFrame::Unknown(text.to_string())
    }
}

/// Build auth.respond params
pub fn build_auth_respond(device_id: &str, signature: &str) -> Value {
    serde_json::json!({
        "device_id": device_id,
        "signature": signature,
    })
}

/// Build chat.send params
pub fn build_chat_send(content: &str) -> Value {
    serde_json::json!({
        "content": content,
    })
}
