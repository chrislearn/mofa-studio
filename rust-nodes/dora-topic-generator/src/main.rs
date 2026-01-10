// Dora Node: Topic Generator
// Generates conversation topics using Doubao API based on selected words

use dora_node_api::{DoraNode, Event, arrow::array::StringArray};
use eyre::{Context, Result};
use reqwest::{Client, header};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Serialize, Deserialize)]
struct WordSelection {
    words: Vec<String>,
    session_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct TopicOutput {
    session_id: String,
    topic: String,
    target_words: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    
    let api_key = std::env::var("DOUBAO_API_KEY")
        .wrap_err("DOUBAO_API_KEY environment variable not set")?;
    println!("=========api_key4: {}", api_key);
    
    let model = std::env::var("DOUBAO_MODEL")
        .unwrap_or_else(|_| "doubao-seed-1-8-251228".to_string());

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let (mut node, mut events) = DoraNode::init_from_env()?;
    
    log::info!("Topic Generator node started with model: {}", model);

    while let Some(event) = events.recv() {
        match event {
            Event::Input { id, data, metadata } => {
                match id.as_str() {
                    "selected_words" => {
                        log::info!("Received word selection");
                        
                        // Extract bytes from Arrow data
                        let raw_data = extract_bytes(&data);
                        if raw_data.is_empty() {
                            log::warn!("Empty data received");
                            continue;
                        }
                        
                        match serde_json::from_slice::<WordSelection>(&raw_data) {
                            Ok(selection) => {
                                log::info!(
                                    "Generating topic for {} words in session {}",
                                    selection.words.len(),
                                    selection.session_id
                                );
                                
                                match generate_topic(&client, &api_key, &model, &selection.words).await {
                                    Ok(topic) => {
                                        let output = TopicOutput {
                                            session_id: selection.session_id,
                                            topic: topic.clone(),
                                            target_words: selection.words,
                                        };
                                        
                                        let output_str = serde_json::to_string(&output)?;
                                        let output_array = StringArray::from(vec![output_str.as_str()]);
                                        node.send_output("topic".into(), metadata.parameters.clone(), output_array)?;
                                        
                                        log::info!("Generated topic: {}", topic);
                                    }
                                    Err(e) => {
                                        log::error!("Failed to generate topic: {}", e);
                                    }
                                }
                            }
                            Err(e) => {
                                log::error!("Failed to parse word selection: {}", e);
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

/// Extract bytes from ArrowData
fn extract_bytes(data: &dora_node_api::ArrowData) -> Vec<u8> {
    use dora_node_api::arrow::array::{Array, UInt8Array};
    
    if let Some(array) = data.0.as_any().downcast_ref::<StringArray>() {
        if array.len() > 0 {
            return array.value(0).as_bytes().to_vec();
        }
    }
    if let Some(array) = data.0.as_any().downcast_ref::<UInt8Array>() {
        return array.values().to_vec();
    }
    Vec::new()
}

async fn generate_topic(
    client: &Client,
    api_key: &str,
    model: &str,
    target_words: &[String],
) -> Result<String> {
    let system_prompt = "You are a professional English teacher. Your task is to help users speak authentic English. You should primarily speak in English, and only switch to Chinese to explain when the user indicates they cannot understand what you're saying. Generate authentic English conversation topics that naturally incorporate the target vocabulary words. Topics should be relevant to current events, work scenarios, or daily life. Keep your response concise - just the topic itself, not a full conversation.";

    let user_prompt = if target_words.is_empty() {
        "Generate an interesting conversation topic for English practice. Make it engaging and relevant to real-life situations like work, daily life, or current events.".to_string()
    } else {
        format!(
            "Generate a conversation topic that naturally uses these words: {}. Make it engaging and relevant to real-life situations like work, daily life, or current events. The topic should encourage the learner to use these words in context.",
            target_words.join(", ")
        )
    };

    let messages = vec![
        json!({
            "role": "system",
            "content": system_prompt
        }),
        json!({
            "role": "user",
            "content": user_prompt
        })
    ];

    let payload = json!({
        "model": model,
        "messages": messages,
        "temperature": 0.8,
        "max_tokens": 500
    });

    let response = client
        .post("https://ark.cn-beijing.volces.com/api/v3/responses")
        .header(header::AUTHORIZATION, format!("Bearer {}", api_key))
        .header(header::CONTENT_TYPE, "application/json")
        .json(&payload)
        .send()
        .await?;

    if !response.status().is_success() {
        let error_text = response.text().await?;
        eyre::bail!("API error: {}", error_text);
    }

    let result: serde_json::Value = response.json().await?;
    
    let content = result["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| eyre::eyre!("No content in response"))?
        .to_string();

    Ok(content)
}
