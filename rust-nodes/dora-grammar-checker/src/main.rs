// Dora Node: Grammar Checker
// 语法检查器 - 使用豆包 API 检查语法和词汇
// 接收 doubao-asr/text 或 mofa-prompt-input/text 进行分析
// 输出 JSON 格式: {session_id, user_text, issues[], pronunciation_issues[]}

use dora_node_api::{DoraNode, Event, arrow::array::{Array, StringArray, UInt8Array}};
use eyre::{Context, Result};
use reqwest::{Client, header};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

/// ASR 输出格式
#[derive(Debug, Serialize, Deserialize)]
struct AsrOutput {
    text: String,
    confidence: f32,
    #[serde(default)]
    words: Vec<WordTiming>,
    session_id: Option<String>,
}

/// 词汇时序信息
#[derive(Debug, Serialize, Deserialize)]
struct WordTiming {
    word: String,
    start_time: f64,
    end_time: f64,
    confidence: f32,
}

/// 文本问题
#[derive(Debug, Serialize, Deserialize)]
struct TextIssue {
    #[serde(rename = "type")]
    issue_type: String,     // grammar | word_choice | suggestion
    original: String,
    suggested: String,
    description: String,
    severity: String,       // low | medium | high

    /// Character offsets into `user_text`.
    /// 0-based, end_position is exclusive. Optional to keep backward compatibility.
    #[serde(default)]
    start_position: Option<i32>,
    #[serde(default)]
    end_position: Option<i32>,
}

/// 发音问题
#[derive(Debug, Serialize, Deserialize)]
struct PronunciationIssue {
    word: String,
    confidence: f32,
}

/// 分析输出格式
#[derive(Debug, Serialize, Deserialize)]
struct AnalysisOutput {
    session_id: String,
    user_text: String,
    issues: Vec<TextIssue>,
    pronunciation_issues: Vec<PronunciationIssue>,
    timestamp: i64,
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    
    let api_key = std::env::var("DOUBAO_API_KEY")
        .wrap_err("DOUBAO_API_KEY environment variable not set")?;
    println!("=========api_key3: {}", api_key);
    
    let model = std::env::var("DOUBAO_MODEL")
        .unwrap_or_else(|_| "doubao-seed-1-8-251228".to_string());

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let (mut node, mut events) = DoraNode::init_from_env()?;
    
    log::info!("Grammar Checker node started (model: {})", model);

    while let Some(event) = events.recv() {
        match event {
            Event::Input { id, data, metadata } => {
                let raw_data = extract_bytes(&data);
                
                match id.as_str() {
                    "asr_text" => {
                        // 处理 ASR 输出 (JSON 格式)
                        log::info!("Received ASR text for analysis");
                        
                        if let Some(bytes) = &raw_data {
                            match serde_json::from_slice::<AsrOutput>(bytes) {
                                Ok(asr_result) => {
                                    if asr_result.text.trim().is_empty() {
                                        log::debug!("Empty ASR text, skipping analysis");
                                        continue;
                                    }
                                    
                                    process_text(
                                        &client, &api_key, &model,
                                        &asr_result.text,
                                        asr_result.session_id.as_deref(),
                                        Some(&asr_result.words),
                                        &mut node,
                                        &metadata,
                                    ).await?;
                                }
                                Err(e) => {
                                    log::error!("Failed to parse ASR output: {}", e);
                                }
                            }
                        }
                    }
                    "text_input" => {
                        // 处理直接文本输入 (纯文本)
                        log::info!("Received direct text input for analysis");
                        
                        if let Some(bytes) = &raw_data {
                            let text = String::from_utf8_lossy(bytes);
                            if text.trim().is_empty() {
                                log::debug!("Empty text input, skipping analysis");
                                continue;
                            }
                            
                            process_text(
                                &client, &api_key, &model,
                                &text,
                                None,
                                None,
                                &mut node,
                                &metadata,
                            ).await?;
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

/// 处理文本并发送分析结果
async fn process_text(
    client: &Client,
    api_key: &str,
    model: &str,
    text: &str,
    session_id: Option<&str>,
    words: Option<&Vec<WordTiming>>,
    node: &mut DoraNode,
    metadata: &dora_node_api::Metadata,
) -> Result<()> {
    let session = session_id
        .map(|s| s.to_string())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    
    log::info!("Analyzing text: {}", text);
    
    // 调用 AI API 分析文本
    let issues = match analyze_text(client, api_key, model, text).await {
        Ok(issues) => {
            log::info!("Found {} grammar/vocabulary issues", issues.len());
            issues
        }
        Err(e) => {
            log::error!("Analysis failed: {}", e);
            Vec::new()
        }
    };
    
    // 收集发音问题 (基于 ASR 置信度)
    let pronunciation_issues = if let Some(word_timings) = words {
        word_timings
            .iter()
            .filter(|w| w.confidence < 0.7)
            .map(|w| {
                log::debug!("Low confidence word: {} ({})", w.word, w.confidence);
                PronunciationIssue {
                    word: w.word.clone(),
                    confidence: w.confidence,
                }
            })
            .collect()
    } else {
        Vec::new()
    };
    
    log::info!(
        "Analysis complete: {} text issues, {} pronunciation issues",
        issues.len(),
        pronunciation_issues.len()
    );
    
    // 构建输出
    let output = AnalysisOutput {
        session_id: session,
        user_text: text.to_string(),
        issues,
        pronunciation_issues,
        timestamp: chrono::Utc::now().timestamp(),
    };
    
    let output_str = serde_json::to_string(&output)?;
    let output_array = StringArray::from(vec![output_str.as_str()]);
    
    node.send_output(
        "analysis".into(),
        metadata.parameters.clone(),
        output_array,
    )?;
    
    // 发送状态
    let status = json!({
        "node": "grammar-checker",
        "status": "ok",
        "issues_count": output.issues.len(),
        "pronunciation_issues_count": output.pronunciation_issues.len(),
    });
    
    let status_array = StringArray::from(vec![status.to_string().as_str()]);
    node.send_output("status".into(), metadata.parameters.clone(), status_array)?;
    
    Ok(())
}

/// 使用豆包 API 分析文本中的语法和词汇问题
async fn analyze_text(
    client: &Client,
    api_key: &str,
    model: &str,
    text: &str,
) -> Result<Vec<TextIssue>> {
    // This node calls Volcengine Ark "Responses API" endpoint:
    //   POST https://ark.cn-beijing.volces.com/api/v3/responses
    // Per docs, request body uses `input` (string or message list), NOT `messages`.

    let system_prompt = r#"You are a professional English teacher and writing coach.

Task:
Given a user's English text, find grammar mistakes, unnatural phrasing, and word choice improvements.

Output requirements (STRICT):
1) Return ONLY valid JSON. No Markdown, no code fences, no extra commentary.
2) The JSON MUST be an array `issues` (not an object). If there are no issues, return `[]`.
3) Each item MUST follow this schema (keys exactly as written):
{
  "type": "grammar" | "word_choice" | "suggestion",
  "original": string,
  "suggested": string,
  "description": string,
  "severity": "low" | "medium" | "high",
  "start_position": number | null,
  "end_position": number | null
}

Notes:
- `start_position` and `end_position` are character offsets into the original user text.
- Use 0-based indexing, and `end_position` is exclusive.
- If you cannot determine an exact span, set both positions to null.
- Keep `original` and `suggested` concise (a phrase/sentence fragment when possible).
"#;

    let input_items = vec![
        json!({
            "role": "system",
            "content": system_prompt
        }),
        json!({
            "role": "user",
            "content": format!("User text:\n{}", text)
        }),
    ];

    let payload = json!({
        "model": model,
        "input": input_items,
        "stream": false,
        "temperature": 0.3,
        "max_tokens": 1000
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

    let content = extract_responses_text(&result)
        .ok_or_else(|| eyre::eyre!("No text content in response: {}", result))?;

    parse_issues_json(&content)
}

fn parse_issues_json(content: &str) -> Result<Vec<TextIssue>> {
    // Happy path: strict JSON array.
    if let Ok(issues) = serde_json::from_str::<Vec<TextIssue>>(content) {
        return Ok(issues);
    }

    // Some models may still wrap in code fences or add a tiny prefix.
    let cleaned = content
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    if let Ok(issues) = serde_json::from_str::<Vec<TextIssue>>(cleaned) {
        return Ok(issues);
    }

    // Last-resort: extract the first JSON array substring.
    if let (Some(start), Some(end)) = (cleaned.find('['), cleaned.rfind(']')) {
        if end > start {
            let slice = &cleaned[start..=end];
            if let Ok(issues) = serde_json::from_str::<Vec<TextIssue>>(slice) {
                return Ok(issues);
            }
        }
    }

    log::warn!("Failed to parse AI response as issues array. content={}", cleaned);
    Ok(Vec::new())
}

fn extract_responses_text(result: &serde_json::Value) -> Option<String> {
    // Some SDKs expose an aggregated `output_text`.
    if let Some(text) = result.get("output_text").and_then(|v| v.as_str()) {
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }

    // Responses API commonly returns `output` items.
    if let Some(output) = result.get("output").and_then(|v| v.as_array()) {
        for item in output {
            if let Some(content) = item.get("content") {
                if let Some(text) = extract_text_from_content_value(content) {
                    return Some(text);
                }
            }

            if let Some(message) = item.get("message") {
                if let Some(content) = message.get("content") {
                    if let Some(text) = extract_text_from_content_value(content) {
                        return Some(text);
                    }
                }
            }
        }
    }

    // Fallback for Chat Completions style responses.
    result
        .get("choices")
        .and_then(|v| v.as_array())
        .and_then(|choices| choices.first())
        .and_then(|first| first.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn extract_text_from_content_value(content: &serde_json::Value) -> Option<String> {
    // Content may be a string.
    if let Some(text) = content.as_str() {
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }

    // Or an array of content items.
    if let Some(items) = content.as_array() {
        for item in items {
            if item.get("type").and_then(|v| v.as_str()) == Some("output_text") {
                if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        return Some(trimmed.to_string());
                    }
                }
            }

            if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
    }

    None
}

/// 从 ArrowData 提取字节
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
