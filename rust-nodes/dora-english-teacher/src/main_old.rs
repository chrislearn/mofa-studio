// Dora Node: English Teacher
// AI 英语老师 - 使用豆包 API 生成对话回复
// 接收 doubao-asr/text 或 mofa-prompt-input/text
// 输出: AI 生成的英语对话回复

use dora_node_api::{DoraNode, Event, arrow::array::{Array, StringArray, UInt8Array}};
use eyre::{Context, Result};
use reqwest::{Client, header};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::VecDeque;
use std::sync::Mutex;

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

/// 对话消息
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChatMessage {
    role: String,  // "user" | "assistant" | "system"
    content: String,
}

/// 对话历史管理器
struct ConversationHistory {
    messages: VecDeque<ChatMessage>,
    max_history: usize,
}

impl ConversationHistory {
    fn new(max_history: usize) -> Self {
        Self {
            messages: VecDeque::new(),
            max_history,
        }
    }
    
    fn add_user_message(&mut self, content: &str) {
        self.messages.push_back(ChatMessage {
            role: "user".to_string(),
            content: content.to_string(),
        });
        self.trim_history();
    }
    
    fn add_assistant_message(&mut self, content: &str) {
        self.messages.push_back(ChatMessage {
            role: "assistant".to_string(),
            content: content.to_string(),
        });
        self.trim_history();
    }
    
    fn trim_history(&mut self) {
        while self.messages.len() > self.max_history * 2 {
            self.messages.pop_front();
        }
    }
    
    fn get_messages(&self) -> Vec<&ChatMessage> {
        self.messages.iter().collect()
    }
}

/// 综合响应输出 (structured output from Doubao)
/// 一次性输出：用户文本 + AI回复 + 语法分析
#[derive(Debug, Serialize, Deserialize)]
struct ComprehensiveResponse {
    session_id: String,
    user_text: String,          // 用户最后一条消息
    ai_reply: String,           // AI对该消息的回复
    issues: Vec<TextIssue>,     // 语法/用词问题
    pronunciation_issues: Vec<PronunciationIssue>, // 发音问题
    timestamp: i64,
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

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    
    let api_key = std::env::var("DOUBAO_API_KEY")
        .wrap_err("DOUBAO_API_KEY environment variable not set")?;
    log::info!("DOUBAO_API_KEY loaded");
    
    let model = std::env::var("DOUBAO_MODEL")
        .unwrap_or_else(|_| "doubao-seed-1-8-251228".to_string());
    
    let system_prompt = std::env::var("SYSTEM_PROMPT")
        .unwrap_or_else(|_| DEFAULT_SYSTEM_PROMPT.to_string());
    
    let max_history: usize = std::env::var("MAX_HISTORY")
        .unwrap_or_else(|_| "10".to_string())
        .parse()
        .unwrap_or(10);

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()?;

    let (mut node, mut events) = DoraNode::init_from_env()?;
    
    // 对话历史
    let history = Mutex::new(ConversationHistory::new(max_history));
    let current_session: Mutex<Option<String>> = Mutex::new(None);
    
    log::info!("English Teacher node started (model: {})", model);

    while let Some(event) = events.recv() {
        match event {
            Event::Input { id, data, metadata } => {
                let raw_data = extract_bytes(&data);
                let (user_text, session_id, words) = match id.as_str() {
                    "asr_text" => {
                        // 处理 ASR 输出 (JSON 格式)
                        log::info!("Received ASR text");
                        if let Some(bytes) = &raw_data {
                            match serde_json::from_slice::<AsrOutput>(bytes) {
                                Ok(asr_result) => {
                                    if asr_result.text.trim().is_empty() {
                                        continue;
                                    }
                                    (asr_result.text, asr_result.session_id, Some(asr_result.words))
                                }
                                Err(e) => {
                                    log::error!("Failed to parse ASR output: {}", e);
                                    continue;
                                }
                            }
                        } else {
                            continue;
                        }
                    }
                    "text_input" => {
                        // 处理直接文本输入 (纯文本)
                        log::info!("Received direct text input");
                        if let Some(bytes) = &raw_data {
                            let text = String::from_utf8_lossy(bytes).to_string();
                            if text.trim().is_empty() {
                                continue;
                            }
                            (text, None, None)
                        } else {
                            continue;
                        }
                    }
                    _ => {
                        log::warn!("Received unknown input: {}", id);
                        continue;
                    }
                };
                
                // 更新或获取 session ID
                let session = {
                    let mut current = current_session.lock().unwrap();
                    if let Some(sid) = session_id {
                        *current = Some(sid.clone());
                        sid
                    } else {
                        current.clone().unwrap_or_else(|| {
                            let new_sid = uuid::Uuid::new_v4().to_string();
                            *current = Some(new_sid.clone());
                            new_sid
                        })
                    }
                };
                
                log::info!("Processing user input: {}", user_text);
                // 添加用户消息到历史
                {
                    let mut hist = history.lock().unwrap();
                    hist.add_user_message(&user_text);
                }
                
                // 使用 structured outputs 一次性生成回复和分析
                match generate_comprehensive_response(
                    &client,
                    &api_key,
                    &model,
                    &system_prompt,
                    &user_text,
                    &history.lock().unwrap(),
                    &session,
                    words.as_ref(),
                ).await {
                    Ok(response) => {
                        log::info!("AI reply: {}", response.ai_reply);
                        log::info!("Found {} issues, {} pronunciation issues", 
                            response.issues.len(), 
                            response.pronunciation_issues.len()
                        );
                        
                        // 添加 AI 回复到历史
                        {
                            let mut hist = history.lock().unwrap();
                            hist.add_assistant_message(&response.ai_reply);
                        }
                        
                        // 发送综合 JSON 输出 (report_text)
                        let output_str = serde_json::to_string(&response)?;
                        let output_array = StringArray::from(vec![output_str.as_str()]);
                        node.send_output(
                            "report_text".to_string().into(),
                            metadata.parameters.clone(),
                            output_array,
                        )?;
                        
                        // 发送状态
                        let status = json!({
                            "node": "english-teacher",
                            "status": "ok",
                            "session_id": session,
                        });
                        
                        let status_array = StringArray::from(vec![status.to_string().as_str()]);
                        node.send_output("status".to_string().into(), metadata.parameters.clone(), status_array)?;
                    }
                    Err(e) => {
                        log::error!("Failed to generate comprehensive response: {}", e);
                        
                        let status = json!({
                            "node": "english-teacher",
                            "status": "error",
                            "error": e.to_string(),
                        });
                        
                        let status_array = StringArray::from(vec![status.to_string().as_str()]);
                        node.send_output("status".to_string().into(), metadata.parameters.clone(), status_array)?;
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

/// 默认系统提示
const DEFAULT_SYSTEM_PROMPT: &str = r#"You are a professional English teacher. Your task is to help users speak authentic English.

Guidelines:
1. Speak English as much as possible
2. Only switch to Chinese when the user explicitly says they cannot understand
3. Keep responses concise and natural for conversation
4. Gently correct mistakes by modeling the correct usage
5. Encourage the user and provide positive feedback
6. Use current events, work scenarios, and daily life topics to make conversations engaging
7. Adjust your language complexity based on the user's level

Remember: Your goal is to help the user practice speaking naturally, not to lecture them."#;

/// 分析用户输入的语法/用词问题（在对话上下文中）
async fn analyze_user_input(
    client: &Client,
    api_key: &str,
    model: &str,
    user_text: &str,
    history: &ConversationHistory,
    session_id: &str,
    words: Option<&Vec<WordTiming>>,
) -> Result<AnalysisOutput> {
    let system_prompt = r#"You are a professional English teacher and writing coach.

Task:
Given a conversation history and the user's most recent message, analyze the user's last message for grammar mistakes, unnatural phrasing, and word choice improvements **in the context of the conversation**.

Output requirements (STRICT):
1) Return ONLY valid JSON. No Markdown, no code fences, no extra commentary.
2) The JSON MUST be an array named `issues` (not an object). If there are no issues, return `[]`.
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
- `start_position` and `end_position` are character offsets into the user's last message (0-based, end is exclusive).
- If you cannot determine an exact span, set both positions to null.
- Keep `original` and `suggested` concise.
- Focus on the user's last message, but consider the conversation context for naturalness.
"#;

    let mut input_items = vec![json!({
        "role": "system",
        "content": system_prompt
    })];

    // Include recent conversation history for context
    let messages = history.get_messages();
    let recent_count = messages.len().min(6); // Last 3 exchanges
    for msg in messages.iter().rev().take(recent_count).rev() {
        input_items.push(json!({
            "role": msg.role,
            "content": msg.content,
        }));
    }

    // Final instruction
    input_items.push(json!({
        "role": "user",
        "content": format!("Analyze the user's last message for issues: \"{}\"", user_text)
    }));

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

    let issues = parse_issues_json(&content)?;

    // Collect pronunciation issues from word timings
    let pronunciation_issues = if let Some(word_timings) = words {
        word_timings
            .iter()
            .filter(|w| w.confidence < 0.7)
            .map(|w| PronunciationIssue {
                word: w.word.clone(),
                confidence: w.confidence,
            })
            .collect()
    } else {
        Vec::new()
    };

    Ok(AnalysisOutput {
        session_id: session_id.to_string(),
        user_text: user_text.to_string(),
        issues,
        pronunciation_issues,
        timestamp: chrono::Utc::now().timestamp(),
    })
}

fn parse_issues_json(content: &str) -> Result<Vec<TextIssue>> {
    // Happy path: strict JSON array.
    if let Ok(issues) = serde_json::from_str::<Vec<TextIssue>>(content) {
        return Ok(issues);
    }

    // Some models may wrap in code fences.
    let cleaned = content
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    if let Ok(issues) = serde_json::from_str::<Vec<TextIssue>>(cleaned) {
        return Ok(issues);
    }

    // Last-resort: extract first JSON array.
    if let (Some(start), Some(end)) = (cleaned.find('['), cleaned.rfind(']')) {
        if end > start {
            let slice = &cleaned[start..=end];
            if let Ok(issues) = serde_json::from_str::<Vec<TextIssue>>(slice) {
                return Ok(issues);
            }
        }
    }

    log::warn!("Failed to parse AI analysis response. content={}", cleaned);
    Ok(Vec::new())
}

/// 生成 AI 回复
async fn generate_response(
    client: &Client,
    api_key: &str,
    model: &str,
    system_prompt: &str,
    history: &ConversationHistory,
) -> Result<String> {
    // NOTE: This node calls Volcengine Ark "Responses API" endpoint:
    //   POST https://ark.cn-beijing.volces.com/api/v3/responses
    // Per docs, request body uses `input` (string or message list), NOT `messages`.
    // See: https://www.volcengine.com/docs/82379/1399008?lang=zh

    let mut input_items = vec![json!({
        "role": "system",
        "content": system_prompt,
    })];

    // 添加对话历史
    for msg in history.get_messages() {
        input_items.push(json!({
            "role": msg.role,
            "content": msg.content,
        }));
    }

    let payload = json!({
        "model": model,
        "input": input_items,
        "stream": false
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

    extract_responses_text(&result)
        .ok_or_else(|| eyre::eyre!("No text content in response: {}", result))
}

fn extract_responses_text(result: &serde_json::Value) -> Option<String> {
    // Some SDKs expose an aggregated `output`.
    if let Some(text) = result.get("output").and_then(|v| v.as_str()) {
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }

    // Responses API commonly returns `output` items. Try to locate the first output text.
    if let Some(output) = result.get("output").and_then(|v| v.as_array()) {
        for item in output {
            // Common shape: { type: "message", role: "assistant", content: [{type:"output_text", text:"..."}] }
            if let Some(text) = extract_text_from_content_value(item.get("content")?) {
                return Some(text);
            }

            // Alternate shape: { message: { content: ... } }
            if let Some(message) = item.get("message") {
                if let Some(text) = extract_text_from_content_value(message.get("content")?) {
                    return Some(text);
                }
            }
        }
    }

    // Fallback for Chat Completions style responses, in case backend returns that shape.
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
            // Prefer explicit output_text.
            if item.get("type").and_then(|v| v.as_str()) == Some("output_text") {
                if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        return Some(trimmed.to_string());
                    }
                }
            }

            // Some variants may just carry {text:"..."}.
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
