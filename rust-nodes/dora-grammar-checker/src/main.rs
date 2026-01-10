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
    let system_prompt = r#"You are an English language expert. Analyze the user's English text and identify issues including:
- Grammar errors
- Word choice problems  
- Better alternatives and suggestions

Return your analysis in JSON format as an array of issues. Each issue should have:
{
    "type": "grammar" | "word_choice" | "suggestion",
    "original": "the problematic text",
    "suggested": "the corrected/better text",
    "description": "explanation of the issue",
    "severity": "low" | "medium" | "high"
}

Only return the JSON array, no additional text. If no issues found, return an empty array []."#;

    let messages = vec![
        json!({
            "role": "system",
            "content": system_prompt
        }),
        json!({
            "role": "user",
            "content": format!("Analyze this English text for grammar, vocabulary, and expression issues: \"{}\"", text)
        })
    ];

    let payload = json!({
        "model": model,
        "messages": messages,
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
    
    let content = result["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| eyre::eyre!("No content in response"))?;

    // 尝试解析 JSON 响应
    match serde_json::from_str::<Vec<TextIssue>>(content) {
        Ok(issues) => Ok(issues),
        Err(_) => {
            // 尝试从 markdown 代码块中提取
            let cleaned = content
                .trim_start_matches("```json")
                .trim_start_matches("```")
                .trim_end_matches("```")
                .trim();
            
            Ok(serde_json::from_str(cleaned).unwrap_or_else(|e| {
                log::warn!("Failed to parse AI response: {}, content: {}", e, cleaned);
                Vec::new()
            }))
        }
    }
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
