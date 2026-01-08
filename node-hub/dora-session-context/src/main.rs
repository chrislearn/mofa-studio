// Dora Node: Session Context Manager
// Manages conversation context including topic, without triggering AI responses
// Ensures AI only responds when user actually speaks

use dora_node_api::{DoraNode, Event, arrow::array::{Array, StringArray}};
use eyre::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::{Arc, Mutex};

#[derive(Debug, Serialize, Deserialize)]
struct TopicInfo {
    session_id: String,
    topic: String,
    target_words: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct UserInput {
    text: String,
    confidence: f32,
    session_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ContextualInput {
    user_text: String,
    session_id: String,
    topic: Option<String>,
    target_words: Option<Vec<String>>,
    is_first_in_session: bool,
}

struct SessionState {
    current_topic: Option<TopicInfo>,
    user_inputs_in_session: usize,
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    
    let state = Arc::new(Mutex::new(SessionState {
        current_topic: None,
        user_inputs_in_session: 0,
    }));

    let (mut node, mut events) = DoraNode::init_from_env()?;
    
    log::info!("Session Context Manager started");

    while let Some(event) = events.recv() {
        match event {
            Event::Input { id, data, metadata } => {
                let raw_data = extract_bytes(&data);
                match id.as_str() {
                    "topic" => {
                        // Receive topic but DON'T output anything yet
                        // Just store it for when user speaks
                        if let Some(bytes) = &raw_data {
                            match serde_json::from_slice::<TopicInfo>(bytes) {
                                Ok(topic_info) => {
                                    log::info!(
                                        "Topic received for session {}: {}",
                                        topic_info.session_id,
                                        topic_info.topic
                                    );
                                    
                                    let mut state = state.lock().unwrap();
                                    state.current_topic = Some(topic_info);
                                    state.user_inputs_in_session = 0;
                                    
                                    // Send status but no chat input
                                    let status = json!({
                                        "status": "topic_ready",
                                        "message": "Waiting for user input..."
                                    });
                                    let status_str = serde_json::to_string(&status)?;
                                    let status_array = StringArray::from(vec![status_str.as_str()]);
                                    node.send_output("status".to_string().into(), metadata.parameters.clone(), status_array)?;
                                }
                                Err(e) => {
                                    log::error!("Failed to parse topic: {}", e);
                                }
                            }
                        }
                    }
                    "user_text" | "text_input" | "asr_text" => {
                        // User spoke! Now we can combine context and forward to AI
                        if let Some(bytes) = &raw_data {
                            match serde_json::from_slice::<UserInput>(bytes) {
                                Ok(user_input) => {
                                    if user_input.text.trim().is_empty() {
                                        log::debug!("Ignoring empty user input");
                                        continue;
                                    }
                                    
                                    log::info!("User input received: {}", user_input.text);
                                    
                                    let mut state = state.lock().unwrap();
                                    state.user_inputs_in_session += 1;
                                    let is_first = state.user_inputs_in_session == 1;
                                    
                                    // Get current session context
                                    let (session_id, topic, target_words) = 
                                        if let Some(ref topic_info) = state.current_topic {
                                            (
                                                topic_info.session_id.clone(),
                                                Some(topic_info.topic.clone()),
                                                Some(topic_info.target_words.clone()),
                                            )
                                        } else {
                                            (
                                                user_input.session_id
                                                    .unwrap_or_else(|| "default".to_string()),
                                                None,
                                                None,
                                            )
                                        };
                                    
                                    // Create contextual input
                                    let contextual = ContextualInput {
                                        user_text: user_input.text,
                                        session_id: session_id.clone(),
                                        topic: topic.clone(),
                                        target_words: target_words.clone(),
                                        is_first_in_session: is_first,
                                    };
                                    
                                    // Output to AI
                                    let output_str = serde_json::to_string(&contextual)?;
                                    let output_array = StringArray::from(vec![output_str.as_str()]);
                                    node.send_output("user_text".to_string().into(), metadata.parameters.clone(), output_array)?;
                                    
                                    log::info!(
                                        "Forwarded user input to AI (session: {}, first: {})",
                                        session_id,
                                        is_first
                                    );
                                    
                                    // If this is the first input, include topic in log
                                    if is_first && topic.is_some() {
                                        log::info!("Topic for this session: {:?}", topic);
                                    }
                                }
                                Err(e) => {
                                    log::error!("Failed to parse user input: {}", e);
                                }
                            }
                        }
                    }
                    "reset" => {
                        // Reset session state
                        log::info!("Resetting session context");
                        let mut state = state.lock().unwrap();
                        state.current_topic = None;
                        state.user_inputs_in_session = 0;
                    }
                    _ => {
                        log::warn!("Received unknown input: {}", id);
                    }
                }
            }
            Event::InputClosed { id } => {
                log::info!("Input {} closed", id);
            }
            Event::Stop(_) => {
                log::info!("Received stop signal");
                break;
            }
            _ => {}
        }
    }

    Ok(())
}

/// Extract bytes from ArrowData
fn extract_bytes(data: &dora_node_api::ArrowData) -> Option<Vec<u8>> {
    use dora_node_api::arrow::datatypes::DataType;
    use dora_node_api::arrow::array::UInt8Array;
    
    let array = &data.0;
    match array.data_type() {
        DataType::UInt8 => {
            let arr = array.as_any().downcast_ref::<UInt8Array>()?;
            Some(arr.values().to_vec())
        }
        DataType::Utf8 => {
            let arr = array.as_any().downcast_ref::<StringArray>()?;
            if arr.len() > 0 {
                Some(arr.value(0).as_bytes().to_vec())
            } else {
                None
            }
        }
        _ => {
            log::warn!("Unsupported data type: {:?}", array.data_type());
            None
        }
    }
}
