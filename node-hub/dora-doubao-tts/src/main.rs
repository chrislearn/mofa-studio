// Dora Node: Doubao TTS (Text-to-Speech)
// Converts AI text responses to speech using Doubao Volcanic Engine API

use dora_node_api::{DoraNode, Event, arrow::array::{Array, StringArray, UInt8Array}, ArrowData};
use eyre::{Context, Result};
use reqwest::{Client, header};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::sqlite::SqlitePool;
use std::time::{SystemTime, UNIX_EPOCH};

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
    
    let speed_ratio = std::env::var("SPEED_RATIO")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1.0);
    
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite://learning_companion.db".to_string());
    
    let pool = SqlitePool::connect(&database_url).await?;

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
                        
                        match serde_json::from_slice::<TextInput>(&raw_data) {
                            Ok(input) => {
                                log::info!("Converting to speech: {}", input.text);
                                
                                match perform_tts(
                                    &client,
                                    &app_id,
                                    &access_token,
                                    &voice_type,
                                    speed_ratio,
                                    &input.text
                                ).await {
                                    Ok(audio_output) => {
                                        log::info!(
                                            "TTS generated {} bytes, duration: {}ms",
                                            audio_output.audio_data.len(),
                                            audio_output.duration_ms
                                        );
                                        
                                        // Save AI conversation to database
                                        if let Some(ref session_id) = input.session_id {
                                            if let Err(e) = save_conversation(
                                                &pool,
                                                session_id,
                                                &input.text
                                            ).await {
                                                log::error!("Failed to save conversation: {}", e);
                                            }
                                        }
                                        
                                        let output_json = serde_json::to_string(&audio_output)?;
                                        let output_array = StringArray::from(vec![output_json.as_str()]);
                                        node.send_output("audio".to_string().into(), metadata.parameters.clone(), output_array)?;
                                    }
                                    Err(e) => {
                                        log::error!("TTS failed: {}", e);
                                    }
                                }
                            }
                            Err(e) => {
                                log::error!("Failed to parse text input: {}", e);
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
    
    let audio_data = base64::decode(audio_base64)?;
    
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

async fn save_conversation(
    pool: &SqlitePool,
    session_id: &str,
    text: &str,
) -> Result<()> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_secs() as i64;

    sqlx::query(
        r#"
        INSERT INTO conversations (
            session_id, speaker, content_text, created_at
        ) VALUES (?, ?, ?, ?)
        "#
    )
    .bind(session_id)
    .bind("ai")
    .bind(text)
    .bind(now)
    .execute(pool)
    .await?;

    Ok(())
}
