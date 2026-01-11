// Dora Node: Doubao ASR (Automatic Speech Recognition)
// Converts user audio to text using Doubao Volcanic Engine API
// 不再直接操作数据库，由 history-db-writer 负责保存对话历史

use base64::Engine;
use dora_node_api::{
    arrow::array::{Array, StringArray, UInt8Array},
    DoraNode, Event,
};
use eyre::{Context, Result};
use reqwest::{header, Client};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Serialize, Deserialize)]
struct AudioInput {
    audio_data: Vec<u8>,
    sample_rate: u32,
    format: String,
    session_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct AsrOutput {
    text: String,
    confidence: f32,
    words: Vec<WordTiming>,
    session_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct WordTiming {
    word: String,
    start_time: f64,
    end_time: f64,
    confidence: f32,
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let app_id =
        std::env::var("DOUBAO_APP_ID").wrap_err("DOUBAO_APP_ID environment variable not set")?;

    let access_token = std::env::var("DOUBAO_ACCESS_TOKEN")
        .wrap_err("DOUBAO_ACCESS_TOKEN environment variable not set")?;

    let cluster =
        std::env::var("DOUBAO_CLUSTER").wrap_err("DOUBAO_CLUSTER environment variable not set")?;

    let language = std::env::var("LANGUAGE").unwrap_or_else(|_| "en".to_string());

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let (mut node, mut events) = DoraNode::init_from_env()?;

    log::info!(
        "Doubao ASR node started (language: {}, cluster: {})",
        language,
        cluster
    );

    while let Some(event) = events.recv() {
        match event {
            Event::Input { id, data, metadata } => {
                match id.as_str() {
                    "audio" => {
                        println!("Received audio input");
                        // Extract raw bytes from arrow data
                        let raw_data: Vec<u8> = extract_bytes(&data)
                            .ok_or_else(|| eyre::eyre!("Failed to get bytes from arrow data"))?;
                        log::debug!("Received audio input ({} bytes)", raw_data.len());

                        match serde_json::from_slice::<AudioInput>(&raw_data) {
                            Ok(input) => {
                                match perform_asr(
                                    &client,
                                    &app_id,
                                    &access_token,
                                    &cluster,
                                    &language,
                                    &input,
                                )
                                .await
                                {
                                    Ok(asr_result) => {
                                        log::info!("ASR result: {}", asr_result.text);

                                        // Send output as StringArray (JSON)
                                        // history-db-writer 会负责保存到数据库
                                        let output_json = serde_json::to_string(&asr_result)?;
                                        let output_array =
                                            StringArray::from(vec![output_json.as_str()]);
                                        node.send_output(
                                            "text".to_string().into(),
                                            metadata.parameters.clone(),
                                            output_array,
                                        )?;

                                        // Send status
                                        let status = json!({
                                            "node": "doubao-asr",
                                            "status": "ok",
                                            "text_length": asr_result.text.len(),
                                        });
                                        let status_array =
                                            StringArray::from(vec![status.to_string().as_str()]);
                                        node.send_output(
                                            "status".to_string().into(),
                                            metadata.parameters.clone(),
                                            status_array,
                                        )?;
                                    }
                                    Err(e) => {
                                        log::error!("ASR failed: {}", e);

                                        let status = json!({
                                            "node": "doubao-asr",
                                            "status": "error",
                                            "error": e.to_string(),
                                        });
                                        let status_array =
                                            StringArray::from(vec![status.to_string().as_str()]);
                                        node.send_output(
                                            "status".to_string().into(),
                                            metadata.parameters.clone(),
                                            status_array,
                                        )?;
                                    }
                                }
                            }
                            Err(e) => {
                                log::error!("Failed to parse audio input: {}", e);
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

async fn perform_asr(
    client: &Client,
    app_id: &str,
    access_token: &str,
    cluster: &str,
    language: &str,
    input: &AudioInput,
) -> Result<AsrOutput> {
    let url = "https://openspeech.bytedance.com/api/v1/asr";

    let audio_base64 = base64::engine::general_purpose::STANDARD.encode(&input.audio_data);

    let payload = json!({
        "app": {
            "appid": app_id,
            "token": access_token,
            "cluster": cluster
        },
        "user": {
            "uid": "user_001"
        },
        "audio": {
            "format": input.format,
            "rate": input.sample_rate,
            "language": language,
            "data": audio_base64
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
        eyre::bail!("ASR API error: {}", error_text);
    }

    let result: serde_json::Value = response.json().await?;

    let text = result["result"]["text"].as_str().unwrap_or("").to_string();

    let confidence = result["result"]["confidence"].as_f64().unwrap_or(0.0) as f32;

    let words = result["result"]["words"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|w| {
                    Some(WordTiming {
                        word: w["word"].as_str()?.to_string(),
                        start_time: w["start_time"].as_f64()?,
                        end_time: w["end_time"].as_f64()?,
                        confidence: w["confidence"].as_f64()? as f32,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(AsrOutput {
        text,
        confidence,
        words,
        session_id: input.session_id.clone(),
    })
}

/// Extract bytes from ArrowData (handles both UInt8Array and StringArray)
fn extract_bytes(data: &dora_node_api::ArrowData) -> Option<Vec<u8>> {
    use dora_node_api::arrow::array::ListArray;
    use dora_node_api::arrow::datatypes::DataType;

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
        DataType::List(_) => {
            let list_array = array.as_any().downcast_ref::<ListArray>()?;
            if list_array.len() > 0 {
                let values = list_array.values();
                if let Some(float_array) = values
                    .as_any()
                    .downcast_ref::<dora_node_api::arrow::array::Float32Array>()
                {
                    let bytes: Vec<u8> = float_array
                        .values()
                        .iter()
                        .flat_map(|f| f.to_le_bytes())
                        .collect();
                    Some(bytes)
                } else {
                    log::warn!("List contains non-Float32 data");
                    None
                }
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
