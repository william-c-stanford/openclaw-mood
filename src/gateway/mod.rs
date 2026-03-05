pub mod config;
pub mod device;
pub mod protocol;

use futures::{SinkExt, StreamExt};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

use self::config::GatewayConfig;
use self::device::DeviceIdentity;
use self::protocol::{ConnectDevice, IncomingFrame, RequestFrame, build_connect_params};

/// Commands sent TO the gateway task
#[derive(Debug)]
pub enum GatewayCommand {
    SendMessage(String),
    #[allow(dead_code)]
    Disconnect,
}

/// Actions received FROM the gateway task
#[derive(Debug)]
pub enum GatewayAction {
    Connected,
    Disconnected(String),
    ChatDelta(String),
    ChatComplete(String),
    Error(String),
    MoodUpdate(crate::mood::MoodUpdate),
}

/// Connection status for UI display
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
}

/// Spawn the gateway WebSocket task.
/// Returns (command_sender, action_receiver).
pub fn spawn_gateway(
    gateway_config: GatewayConfig,
) -> (mpsc::Sender<GatewayCommand>, mpsc::Receiver<GatewayAction>) {
    let (cmd_tx, cmd_rx) = mpsc::channel::<GatewayCommand>(32);
    let (act_tx, act_rx) = mpsc::channel::<GatewayAction>(64);

    tokio::spawn(gateway_task(gateway_config, cmd_rx, act_tx));

    (cmd_tx, act_rx)
}

async fn gateway_task(
    config: GatewayConfig,
    mut cmd_rx: mpsc::Receiver<GatewayCommand>,
    act_tx: mpsc::Sender<GatewayAction>,
) {
    let mut backoff_ms: u64 = 1000;

    // Load device identity once (persisted across reconnects)
    let identity = match DeviceIdentity::load_or_create() {
        Ok(id) => id,
        Err(e) => {
            let _ = act_tx
                .send(GatewayAction::Error(format!("device identity error: {e}")))
                .await;
            return;
        }
    };

    // Session key for this TUI session (persists across reconnects within same run)
    let session_key = format!("matrix-tui-{}", &identity.device_id[..8.min(identity.device_id.len())]);

    loop {
        // Connect with timeout
        let connect_result = tokio::time::timeout(
            Duration::from_secs(10),
            tokio_tungstenite::connect_async(&config.url),
        )
        .await;

        let (ws_stream, _) = match connect_result {
            Ok(Ok(conn)) => {
                backoff_ms = 1000;
                conn
            }
            Ok(Err(e)) => {
                let _ = act_tx
                    .send(GatewayAction::Disconnected(format!("connect failed: {e}")))
                    .await;
                sleep_with_jitter(backoff_ms).await;
                backoff_ms = (backoff_ms * 2).min(30_000);
                continue;
            }
            Err(_) => {
                let _ = act_tx
                    .send(GatewayAction::Disconnected(
                        "connect timeout (10s)".to_string(),
                    ))
                    .await;
                sleep_with_jitter(backoff_ms).await;
                backoff_ms = (backoff_ms * 2).min(30_000);
                continue;
            }
        };

        let (mut ws_write, mut ws_read) = ws_stream.split();

        // --- Auth handshake ---
        // Step 1: Wait for connect.challenge event (with timeout)
        let nonce = match wait_for_challenge(&mut ws_read, Duration::from_secs(5)).await {
            Some(n) => n,
            None => {
                let _ = act_tx
                    .send(GatewayAction::Disconnected(
                        "auth: no connect.challenge received".to_string(),
                    ))
                    .await;
                sleep_with_jitter(backoff_ms).await;
                backoff_ms = (backoff_ms * 2).min(30_000);
                continue;
            }
        };

        // Step 2: Sign and send connect request
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let device = ConnectDevice {
            id: identity.device_id.clone(),
            public_key_base64url: identity.public_key_base64url(),
            signature_base64url: identity.sign_connect_payload(
                &nonce,
                config.token.as_deref(),
                now_ms,
            ),
            signed_at_ms: now_ms,
            nonce: nonce.clone(),
        };

        let connect_params = build_connect_params(config.token.as_deref(), Some(device));
        let connect_frame = RequestFrame::new("connect", Some(connect_params));
        if ws_write
            .send(Message::Text(connect_frame.to_json().into()))
            .await
            .is_err()
        {
            let _ = act_tx
                .send(GatewayAction::Disconnected(
                    "failed to send connect request".to_string(),
                ))
                .await;
            sleep_with_jitter(backoff_ms).await;
            backoff_ms = (backoff_ms * 2).min(30_000);
            continue;
        }

        // Step 3: Wait for hello-ok response (with timeout)
        match wait_for_hello(&mut ws_read, Duration::from_secs(5)).await {
            HelloResult::Ok => {}
            HelloResult::Rejected(msg) => {
                let _ = act_tx
                    .send(GatewayAction::Error(format!("auth rejected: {msg}")))
                    .await;
                // Don't reconnect on auth rejection — it's permanent
                let _ = act_tx
                    .send(GatewayAction::Disconnected("auth failed".to_string()))
                    .await;
                return;
            }
            HelloResult::Timeout => {
                let _ = act_tx
                    .send(GatewayAction::Disconnected(
                        "auth: no hello-ok response".to_string(),
                    ))
                    .await;
                sleep_with_jitter(backoff_ms).await;
                backoff_ms = (backoff_ms * 2).min(30_000);
                continue;
            }
        }

        // Connected and authenticated!
        let _ = act_tx.send(GatewayAction::Connected).await;

        // Step 4: Main event loop
        let disconnected =
            run_event_loop(&mut ws_read, &mut ws_write, &mut cmd_rx, &act_tx, &session_key).await;

        if !disconnected {
            // Graceful disconnect requested by user
            return;
        }

        // Connection lost, reconnect
        let _ = act_tx
            .send(GatewayAction::Disconnected(
                "connection lost, reconnecting...".to_string(),
            ))
            .await;
        sleep_with_jitter(backoff_ms).await;
        backoff_ms = (backoff_ms * 2).min(30_000);
    }
}

/// Wait for the connect.challenge event. Returns the nonce, or None on timeout.
async fn wait_for_challenge<S>(ws_read: &mut S, timeout: Duration) -> Option<String>
where
    S: StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    let deadline = tokio::time::sleep(timeout);
    tokio::pin!(deadline);

    loop {
        tokio::select! {
            _ = &mut deadline => return None,
            msg = ws_read.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        if let IncomingFrame::ConnectChallenge { nonce } = IncomingFrame::parse(&text) {
                            return Some(nonce);
                        }
                    }
                    Some(Ok(_)) => continue,
                    _ => return None,
                }
            }
        }
    }
}

enum HelloResult {
    Ok,
    Rejected(String),
    Timeout,
}

/// Wait for the hello-ok response or error. Returns the result.
async fn wait_for_hello<S>(ws_read: &mut S, timeout: Duration) -> HelloResult
where
    S: StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    let deadline = tokio::time::sleep(timeout);
    tokio::pin!(deadline);

    loop {
        tokio::select! {
            _ = &mut deadline => return HelloResult::Timeout,
            msg = ws_read.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        match IncomingFrame::parse(&text) {
                            IncomingFrame::HelloOk { .. } => return HelloResult::Ok,
                            IncomingFrame::ErrorResponse { message, .. } => {
                                return HelloResult::Rejected(message);
                            }
                            _ => continue, // ignore other frames during handshake
                        }
                    }
                    Some(Ok(_)) => continue,
                    _ => return HelloResult::Rejected("connection closed during auth".to_string()),
                }
            }
        }
    }
}

/// Main event loop after authentication. Returns true if disconnected (should reconnect),
/// false if user requested disconnect (should exit).
async fn run_event_loop<S, W>(
    ws_read: &mut S,
    ws_write: &mut W,
    cmd_rx: &mut mpsc::Receiver<GatewayCommand>,
    act_tx: &mpsc::Sender<GatewayAction>,
    session_key: &str,
) -> bool
where
    S: StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
    W: SinkExt<Message> + Unpin,
{
    loop {
        tokio::select! {
            msg = ws_read.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        match IncomingFrame::parse(&text) {
                            IncomingFrame::ChatDelta { text } => {
                                let _ = act_tx.send(GatewayAction::ChatDelta(text)).await;
                            }
                            IncomingFrame::ChatComplete { text } => {
                                let _ = act_tx.send(GatewayAction::ChatComplete(text)).await;
                            }
                            IncomingFrame::ChatError { message } => {
                                let _ = act_tx.send(GatewayAction::Error(message)).await;
                            }
                            IncomingFrame::MoodUpdate(update) => {
                                let _ = act_tx.send(GatewayAction::MoodUpdate(update)).await;
                            }
                            IncomingFrame::ErrorResponse { message, .. } => {
                                let _ = act_tx.send(GatewayAction::Error(message)).await;
                            }
                            // Ignore responses to our requests, unknown frames, etc.
                            _ => {}
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        return true; // disconnected, should reconnect
                    }
                    Some(Err(_)) => {
                        return true; // error, should reconnect
                    }
                    _ => {}
                }
            }
            cmd = cmd_rx.recv() => {
                match cmd {
                    Some(GatewayCommand::SendMessage(content)) => {
                        let frame = RequestFrame::new(
                            "chat.send",
                            Some(protocol::build_chat_send(&content, session_key)),
                        );
                        let _ = ws_write.send(Message::Text(frame.to_json().into())).await;
                    }
                    Some(GatewayCommand::Disconnect) | None => {
                        let _ = ws_write.close().await;
                        return false; // user disconnect, don't reconnect
                    }
                }
            }
        }
    }
}

async fn sleep_with_jitter(base_ms: u64) {
    let jitter: u64 = rand::random::<u64>() % (base_ms / 4 + 1);
    tokio::time::sleep(Duration::from_millis(base_ms + jitter)).await;
}
