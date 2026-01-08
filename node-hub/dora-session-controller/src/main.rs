// Dora Node: Session Controller
// Controls learning session flow: start, stop, reset, next

use dora_node_api::{DoraNode, Event, arrow::array::{Array, StringArray, UInt8Array}, ArrowData};
use eyre::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ControlCommand {
    command: String,
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StatusOutput {
    status: String,
    session_id: Option<String>,
    message: String,
}

#[derive(Debug, Clone, PartialEq)]
enum SessionState {
    Idle,
    Active,
    WaitingForInput,
    Processing,
    Paused,
}

fn main() -> Result<()> {
    env_logger::init();
    
    let session_mode = std::env::var("SESSION_MODE")
        .unwrap_or_else(|_| "learning".to_string());
    
    let (mut node, mut events) = DoraNode::init_from_env()?;
    
    let mut state = SessionState::Idle;
    let mut current_session_id: Option<String> = None;
    
    log::info!("Session Controller started (mode: {})", session_mode);
    
    while let Some(event) = events.recv() {
        match event {
            Event::Input { id, data, metadata } => {
                let raw_data = extract_bytes(&data);
                
                match id.as_str() {
                    "user_input" => {
                        // Handle control commands from user input
                        if let Ok(cmd) = serde_json::from_slice::<ControlCommand>(&raw_data) {
                            log::info!("Received user control command: {:?}", cmd.command);
                            
                            match cmd.command.as_str() {
                                "start" => {
                                    if state == SessionState::Idle {
                                        // Generate new session ID
                                        let session_id = uuid::Uuid::new_v4().to_string();
                                        current_session_id = Some(session_id.clone());
                                        state = SessionState::Active;
                                        
                                        log::info!("Starting new session: {}", session_id);
                                        
                                        // Broadcast start signal
                                        let control = ControlCommand {
                                            command: "start".to_string(),
                                            session_id: Some(session_id.clone()),
                                            data: None,
                                        };
                                        let control_json = serde_json::to_string(&control)?;
                                        let control_array = StringArray::from(vec![control_json.as_str()]);
                                        node.send_output("control".to_string().into(), metadata.parameters.clone(), control_array)?;
                                        
                                        // Send status
                                        send_status(&mut node, &metadata, "session_started", Some(&session_id), "Learning session started")?;
                                    }
                                }
                                "stop" => {
                                    if state != SessionState::Idle {
                                        let session_id = current_session_id.clone();
                                        state = SessionState::Idle;
                                        current_session_id = None;
                                        
                                        log::info!("Stopping session");
                                        
                                        // Broadcast stop signal
                                        let control = ControlCommand {
                                            command: "stop".to_string(),
                                            session_id,
                                            data: None,
                                        };
                                        let control_json = serde_json::to_string(&control)?;
                                        let control_array = StringArray::from(vec![control_json.as_str()]);
                                        node.send_output("control".to_string().into(), metadata.parameters.clone(), control_array)?;
                                        
                                        send_status(&mut node, &metadata, "session_stopped", None, "Learning session stopped")?;
                                    }
                                }
                                "reset" => {
                                    log::info!("Resetting session");
                                    
                                    // Broadcast reset signal
                                    let control = ControlCommand {
                                        command: "reset".to_string(),
                                        session_id: current_session_id.clone(),
                                        data: None,
                                    };
                                    let control_json = serde_json::to_string(&control)?;
                                    let control_array = StringArray::from(vec![control_json.as_str()]);
                                    node.send_output("control".to_string().into(), metadata.parameters.clone(), control_array)?;
                                    
                                    send_status(&mut node, &metadata, "session_reset", current_session_id.as_deref(), "Session reset")?;
                                }
                                "pause" => {
                                    if state == SessionState::Active {
                                        state = SessionState::Paused;
                                        
                                        let control = ControlCommand {
                                            command: "pause".to_string(),
                                            session_id: current_session_id.clone(),
                                            data: None,
                                        };
                                        let control_json = serde_json::to_string(&control)?;
                                        let control_array = StringArray::from(vec![control_json.as_str()]);
                                        node.send_output("control".to_string().into(), metadata.parameters.clone(), control_array)?;
                                        
                                        send_status(&mut node, &metadata, "session_paused", current_session_id.as_deref(), "Session paused")?;
                                    }
                                }
                                "resume" => {
                                    if state == SessionState::Paused {
                                        state = SessionState::Active;
                                        
                                        let control = ControlCommand {
                                            command: "resume".to_string(),
                                            session_id: current_session_id.clone(),
                                            data: None,
                                        };
                                        let control_json = serde_json::to_string(&control)?;
                                        let control_array = StringArray::from(vec![control_json.as_str()]);
                                        node.send_output("control".to_string().into(), metadata.parameters.clone(), control_array)?;
                                        
                                        send_status(&mut node, &metadata, "session_resumed", current_session_id.as_deref(), "Session resumed")?;
                                    }
                                }
                                _ => {
                                    log::warn!("Unknown command: {}", cmd.command);
                                }
                            }
                        } else {
                            // If not a control command, treat as text input signal
                            let text = String::from_utf8_lossy(&raw_data);
                            log::debug!("Received user input signal: {}", text);
                        }
                    }
                    "audio_complete" => {
                        // Audio playback completed, ready for next input
                        log::debug!("Audio playback completed");
                        
                        if state == SessionState::Processing {
                            state = SessionState::WaitingForInput;
                            
                            // Signal ready for next input
                            let control = ControlCommand {
                                command: "ready".to_string(),
                                session_id: current_session_id.clone(),
                                data: None,
                            };
                            let control_json = serde_json::to_string(&control)?;
                            let control_array = StringArray::from(vec![control_json.as_str()]);
                            node.send_output("control".to_string().into(), metadata.parameters.clone(), control_array)?;
                        }
                    }
                    "analysis" => {
                        // Grammar analysis completed
                        log::debug!("Analysis completed");
                        
                        if state == SessionState::Active {
                            state = SessionState::Processing;
                        }
                    }
                    _ => {
                        log::debug!("Received input from: {}", id);
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

fn send_status(
    node: &mut DoraNode,
    metadata: &dora_node_api::Metadata,
    status: &str,
    session_id: Option<&str>,
    message: &str,
) -> Result<()> {
    let status_output = StatusOutput {
        status: status.to_string(),
        session_id: session_id.map(String::from),
        message: message.to_string(),
    };
    let status_json = serde_json::to_string(&status_output)?;
    let status_array = StringArray::from(vec![status_json.as_str()]);
    node.send_output("status".to_string().into(), metadata.parameters.clone(), status_array)?;
    
    // Also send to log
    let log_msg = format!("[session-controller] {}: {}", status, message);
    let log_array = StringArray::from(vec![log_msg.as_str()]);
    node.send_output("log".to_string().into(), metadata.parameters.clone(), log_array)?;
    
    Ok(())
}

fn extract_bytes(data: &ArrowData) -> Vec<u8> {
    // Try to extract as StringArray first
    if let Some(array) = data.0.as_any().downcast_ref::<StringArray>() {
        if array.len() > 0 {
            return array.value(0).as_bytes().to_vec();
        }
    }
    // Try as UInt8Array
    if let Some(array) = data.0.as_any().downcast_ref::<UInt8Array>() {
        return array.values().to_vec();
    }
    Vec::new()
}
