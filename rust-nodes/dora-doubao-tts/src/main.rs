// Dora Node: Doubao TTS (Text-to-Speech)
// Converts AI text responses to speech using Doubao Volcanic Engine API
// 不再直接操作数据库，由 history-db-writer 负责保存对话历史

use dora_node_api::{DoraNode, Event, arrow::array::{Array, StringArray, UInt8Array}, ArrowData};
use eyre::{Context, Result};
use reqwest::{Client, header};
use serde::{Deserialize, Serialize};
use serde_json::json;
use base64::Engine;

/// 文本输入格式 (来自 english-teacher)
#[derive(Debug, Serialize, Deserialize)]
struct TeacherOutput {
    text: String,
    session_id: String,
    timestamp: i64,
}

/// 简单文本输入格式
#[derive(Debug, Serialize, Deserialize)]
struct TextInput {
    text: String,
    session_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct AudioOutput {
    audio_data: Vec<u8>,
    duration_ms: u64,
    format: String,
    sample_rate: u32,
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    
    let app_id = std::env::var("DOUBAO_APP_ID")
        .wrap_err("DOUBAO_APP_ID environment variable not set")?;
    
    let access_token = std::env::var("DOUBAO_ACCESS_TOKEN")
        .wrap_err("DOUBAO_ACCESS_TOKEN environment variable not set")?;
    
    let voice_type = std::env::var("VOICE_TYPE")
        .unwrap_or_else(|_| "BV700_V2_streaming".to_string()); // Default US English voice
    
    let speed_ratio: f32 = std::env::var("SPEED_RATIO")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1.0);

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let (mut node, mut events) = DoraNode::init_from_env()?;
    
    log::info!("Doubao TTS node started (voice: {}, speed: {})", voice_type, speed_ratio);

    while let Some(event) = events.recv() {
        match event {
            Event::Input { id, data, metadata } => {
                let raw_data = extract_bytes(&data);
                match id.as_str() {
                    "text" => {
                        log::debug!("Received text input");
                        
                        // 尝试解析为 TeacherOutput (来自 english-teacher)
                        let text_to_convert = if let Ok(teacher_output) = serde_json::from_slice::<TeacherOutput>(&raw_data) {
                            teacher_output.text
                        } else if let Ok(text_input) = serde_json::from_slice::<TextInput>(&raw_data) {
                            text_input.text
                        } else {
                            // 尝试作为纯文本
                            String::from_utf8_lossy(&raw_data).to_string()
                        };
                        
                        if text_to_convert.trim().is_empty() {
                            log::debug!("Empty text, skipping TTS");
                            continue;
                        }
                        
                        log::info!("Converting to speech: {}", text_to_convert);
                        
                        match perform_tts(
                            &client,
                            &app_id,
                            &access_token,
                            &voice_type,
                            speed_ratio,
                            &text_to_convert
                        ).await {
                            Ok(audio_output) => {
                                log::info!(
                                    "TTS generated {} bytes, duration: {}ms",
                                    audio_output.audio_data.len(),
                                    audio_output.duration_ms
                                );
                                
                                let output_json = serde_json::to_string(&audio_output)?;
                                let output_array = StringArray::from(vec![output_json.as_str()]);
                                node.send_output("audio".to_string().into(), metadata.parameters.clone(), output_array)?;
                                
                                // 发送状态
                                let status = json!({
                                    "node": "doubao-tts",
                                    "status": "ok",
                                    "duration_ms": audio_output.duration_ms,
                                });
                                let status_array = StringArray::from(vec![status.to_string().as_str()]);
                                node.send_output("status".to_string().into(), metadata.parameters.clone(), status_array)?;
                            }
                            Err(e) => {
                                log::error!("TTS failed: {}", e);
                                
                                let status = json!({
                                    "node": "doubao-tts",
                                    "status": "error",
                                    "error": e.to_string(),
                                });
                                let status_array = StringArray::from(vec![status.to_string().as_str()]);
                                node.send_output("status".to_string().into(), metadata.parameters.clone(), status_array)?;
                            }
                        }
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

async fn perform_tts(
    client: &Client,
    app_id: &str,
    access_token: &str,
    voice_type: &str,
    speed_ratio: f32,
    text: &str,
) -> Result<AudioOutput> {
    let url = "https://openspeech.bytedance.com/api/v1/tts";
    
    let payload = json!({
        "app": {
            "appid": app_id,
            "token": access_token
        },
        "user": {
            "uid": "user_001"
        },
        "audio": {
            "voice_type": voice_type,
            "encoding": "wav",
            "speed_ratio": speed_ratio,
            "volume_ratio": 1.0,
            "pitch_ratio": 1.0,
            "sample_rate": 24000
        },
        "request": {
            "text": text,
            "operation": "submit"
        }
    });

    let response = client
        .post(url)
        .header(header::CONTENT_TYPE, "application/json")
        .json(&payload)
        .send()
        .await?;

    if !response.status().is_success() {
        let error_text = response.text().await?;
        eyre::bail!("TTS API error: {}", error_text);
    }

    let result: serde_json::Value = response.json().await?;
    
    let audio_base64 = result["data"]
        .as_str()
        .ok_or_else(|| eyre::eyre!("Missing audio data in response"))?;
    
    let audio_data = base64::engine::general_purpose::STANDARD.decode(audio_base64)?;
    
    let duration_ms = result["duration"]
        .as_u64()
        .unwrap_or(0);

    Ok(AudioOutput {
        audio_data,
        duration_ms,
        format: "wav".to_string(),
        sample_rate: 24000,
    })
}
