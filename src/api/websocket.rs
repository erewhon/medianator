use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::Response,
};
use futures::{sink::SinkExt, stream::StreamExt};
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, info};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::handlers::AppState;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsMessage {
    Connected {
        client_id: String,
    },
    ScanProgress {
        path: String,
        files_scanned: usize,
        files_added: usize,
        files_updated: usize,
    },
    TranscriptionProgress {
        media_id: String,
        status: String,
        progress: f32,
        message: Option<String>,
    },
    TranscriptionSegment {
        media_id: String,
        segment: TranscriptionSegmentUpdate,
    },
    MediaUpdated {
        media_id: String,
        update_type: String,
    },
    FaceDetectionProgress {
        media_id: String,
        faces_detected: usize,
    },
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct TranscriptionSegmentUpdate {
    pub start_time: f64,
    pub end_time: f64,
    pub text: String,
    pub confidence: Option<f32>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    Subscribe {
        events: Vec<String>,
    },
    Unsubscribe {
        events: Vec<String>,
    },
    Ping,
}

lazy_static::lazy_static! {
    static ref BROADCAST_CHANNEL: broadcast::Sender<WsMessage> = {
        let (tx, _) = broadcast::channel(1000);
        tx
    };
}

pub fn get_broadcast_channel() -> broadcast::Sender<WsMessage> {
    BROADCAST_CHANNEL.clone()
}

pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(_state): State<Arc<AppState>>,
) -> Response {
    ws.on_upgrade(handle_socket)
}

async fn handle_socket(socket: WebSocket) {
    let client_id = Uuid::new_v4().to_string();
    info!("New WebSocket connection: {}", client_id);
    
    let (mut sender, mut receiver) = socket.split();
    let mut rx = BROADCAST_CHANNEL.subscribe();
    
    // Send initial connection message
    let connect_msg = WsMessage::Connected {
        client_id: client_id.clone(),
    };
    
    if let Ok(msg) = serde_json::to_string(&connect_msg) {
        let _ = sender.send(Message::Text(msg)).await;
    }
    
    // Spawn task to handle incoming messages from client
    let client_id_clone = client_id.clone();
    let mut send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if let Ok(json) = serde_json::to_string(&msg) {
                if sender.send(Message::Text(json)).await.is_err() {
                    break;
                }
            }
        }
    });
    
    // Spawn task to handle outgoing messages to client
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Text(text) => {
                    if let Ok(client_msg) = serde_json::from_str::<ClientMessage>(&text) {
                        handle_client_message(client_msg, &client_id_clone).await;
                    }
                }
                Message::Ping(ping) => {
                    debug!("Received ping from {}: {:?}", client_id_clone, ping);
                }
                Message::Close(_) => {
                    info!("Client {} disconnected", client_id_clone);
                    break;
                }
                _ => {}
            }
        }
    });
    
    // Wait for either task to complete
    tokio::select! {
        _ = (&mut send_task) => {
            recv_task.abort();
        }
        _ = (&mut recv_task) => {
            send_task.abort();
        }
    }
    
    info!("WebSocket connection closed: {}", client_id);
}

async fn handle_client_message(msg: ClientMessage, client_id: &str) {
    match msg {
        ClientMessage::Subscribe { events } => {
            debug!("Client {} subscribing to events: {:?}", client_id, events);
        }
        ClientMessage::Unsubscribe { events } => {
            debug!("Client {} unsubscribing from events: {:?}", client_id, events);
        }
        ClientMessage::Ping => {
            debug!("Received ping from client {}", client_id);
        }
    }
}

// Helper functions to broadcast messages
pub fn broadcast_scan_progress(path: &str, files_scanned: usize, files_added: usize, files_updated: usize) {
    let msg = WsMessage::ScanProgress {
        path: path.to_string(),
        files_scanned,
        files_added,
        files_updated,
    };
    let _ = BROADCAST_CHANNEL.send(msg);
}

pub fn broadcast_transcription_progress(media_id: &str, status: &str, progress: f32, message: Option<String>) {
    let msg = WsMessage::TranscriptionProgress {
        media_id: media_id.to_string(),
        status: status.to_string(),
        progress,
        message,
    };
    let _ = BROADCAST_CHANNEL.send(msg);
}

pub fn broadcast_transcription_segment(media_id: &str, segment: TranscriptionSegmentUpdate) {
    let msg = WsMessage::TranscriptionSegment {
        media_id: media_id.to_string(),
        segment,
    };
    let _ = BROADCAST_CHANNEL.send(msg);
}

pub fn broadcast_media_updated(media_id: &str, update_type: &str) {
    let msg = WsMessage::MediaUpdated {
        media_id: media_id.to_string(),
        update_type: update_type.to_string(),
    };
    let _ = BROADCAST_CHANNEL.send(msg);
}

pub fn broadcast_face_detection_progress(media_id: &str, faces_detected: usize) {
    let msg = WsMessage::FaceDetectionProgress {
        media_id: media_id.to_string(),
        faces_detected,
    };
    let _ = BROADCAST_CHANNEL.send(msg);
}

pub fn broadcast_error(message: &str) {
    let msg = WsMessage::Error {
        message: message.to_string(),
    };
    let _ = BROADCAST_CHANNEL.send(msg);
}