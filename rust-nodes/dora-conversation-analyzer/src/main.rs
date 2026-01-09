// Dora Node: Conversation Analyzer
// Analyzes user speech for pronunciation, grammar, and vocabulary issues
// 只负责调用 AI API 分析，不操作数据库

use dora_node_api::{DoraNode, Event, arrow::array::{Array, StringArray, UInt8Array}};
use eyre::{Context, Result};
use reqwest::{Client, header};
use serde::{Deserialize, Serialize};
use serde_json::json;

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

#[derive(Debug, Serialize, Deserialize)]
struct TextIssue {
    #[serde(rename = "type")]
    issue_type: String,
    original: String,
    suggested: String,
    description: String,
    severity: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct PronunciationIssue {
    word: String,
    confidence: f32,
}

#[derive(Debug, Serialize, Deserialize)]
struct AnalysisOutput {
    session_id: String,
    user_text: String,
    issues: Vec<TextIssue>,
    pronunciation_issues: Vec<PronunciationIssue>,
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    
    let api_key = std::env::var("DOUBAO_API_KEY")
        .wrap_err("DOUBAO_API_KEY environment variable not set")?;
    println!("=========api_key2: {}", api_key);
    
    let model = std::env::var("DOUBAO_MODEL")
        .unwrap_or_else(|_| "doubao-pro-32k".to_string());

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let (mut node, mut events) = DoraNode::init_from_env()?;
    
    log::info!("Conversation Analyzer node started (API only, no DB)");


    while let Some(event) = events.recv() {
        match event {
            Event::Input { id, data, metadata } => {
                let raw_data = extract_bytes(&data);
                match id.as_str() {
                    "user_text" => {
                        log::info!("Received user text for analysis");
                        
                        if let Some(bytes) = &raw_data {
                            match serde_json::from_slice::<AsrOutput>(bytes) {
                                Ok(asr_result) => {
                                    if asr_result.text.trim().is_empty() {
                                        continue;
                                    }
                                    
                                    let session_id = asr_result.session_id
                                        .as_ref()
                                        .map(|s| s.clone())
                                        .unwrap_or_else(|| "unknown".to_string());
                                    
                                    log::info!("Analyzing: {}", asr_result.text);
                                    
                                    // Analyze text for issues
                                    match analyze_text(&client, &api_key, &model, &asr_result.text).await {
                                        Ok(issues) => {
                                            log::info!("Found {} issues", issues.len());
                                            
                                            // Collect pronunciation issues (low confidence words)
                                            let mut pronunciation_issues = Vec::new();
                                            for word_info in &asr_result.words {
                                                if word_info.confidence < 0.7 {
                                                    log::info!(
                                                        "Low confidence word detected: {} ({})",
                                                        word_info.word,
                                                        word_info.confidence
                                                    );
                                                    pronunciation_issues.push(PronunciationIssue {
                                                        word: word_info.word.clone(),
                                                        confidence: word_info.confidence,
                                                    });
                                                }
                                            }
                                            
                                            // Create output with all analysis results
                                            let output = AnalysisOutput {
                                                session_id: session_id.clone(),
                                                user_text: asr_result.text.clone(),
                                                issues,
                                                pronunciation_issues,
                                            };
                                            
                                            let output_str = serde_json::to_string(&output)?;
                                            let output_array = StringArray::from(vec![output_str.as_str()]);
                                            node.send_output("analysis".to_string().into(), metadata.parameters.clone(), output_array)?;
                                            
                                            log::info!("Analysis complete, sent to output");
                                        }
                                        Err(e) => {
                                            log::error!("Analysis failed: {}", e);
                                        }
                                    }
                                }
                                Err(e) => {
                                    log::error!("Failed to parse ASR output: {}", e);
                                }
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
fn extract_bytes(data: &dora_node_api::ArrowData) -> Option<Vec<u8>> {
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
        _ => {
            log::warn!("Unsupported data type: {:?}", array.data_type());
            None
        }
    }
}

async fn analyze_text(
    client: &Client,
    api_key: &str,
    model: &str,
    text: &str,
) -> Result<Vec<TextIssue>> {
    let system_prompt = r#"You are an English language expert. Analyze the user's English text and identify issues including: grammar errors, word choice problems, better alternatives, and suggest improvements. Return your analysis in JSON format as an array of issues, each with: {"type": "grammar|word_choice|suggestion", "original": "text", "suggested": "better text", "description": "explanation", "severity": "low|medium|high"}. Only return the JSON array, no additional text."#;

    let messages = vec![
        json!({
            "role": "system",
            "content": system_prompt
        }),
        json!({
            "role": "user",
            "content": format!("Analyze this text: \"{}\"", text)
        })
    ];

    let payload = json!({
        "model": model,
        "messages": messages,
        "temperature": 0.3,
        "max_tokens": 1000
    });

    let response = client
        .post("https://ark.cn-beijing.volces.com/api/v3/chat/completions")
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
        .ok_or_else(|| eyre::eyre!("No content in response"))?;

    // Try to parse JSON response
    match serde_json::from_str::<Vec<TextIssue>>(content) {
        Ok(issues) => Ok(issues),
        Err(_) => {
            // Try extracting from markdown code blocks
            let cleaned = content
                .trim_start_matches("```json")
                .trim_start_matches("```")
                .trim_end_matches("```")
                .trim();
            
            Ok(serde_json::from_str(cleaned).unwrap_or_else(|_| Vec::new()))
        }
    }
}
