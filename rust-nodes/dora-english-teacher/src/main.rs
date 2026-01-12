// Dora Node: English Teacher
// AI 英语老师 - 使用豆包 API 生成对话回复和语法分析
// 使用 structured outputs 一次性输出: 用户文本 + AI回复 + 语法分析
// 输出: json_data (JSON: {session_id, user_text, reply_text, issues[], pronunciation_issues[]})

use dora_node_api::{
    arrow::array::{Array, StringArray, UInt8Array},
    DoraNode, Event,
};
use eyre::{Context, Result};
use reqwest::{header, Client};
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
    role: String, // "user" | "assistant" | "system"
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
    use_lang: String,       // 用户文本语言: "en" | "zh" | "mix"
    original_en: String,    // 用户原始文本（英文）
    original_zh: String,    // 用户原始文本（中文）
    reply_en: String,       // AI对该消息的英文回复
    reply_zh: String,       // AI对该消息的中文回复
    issues: Vec<TextIssue>, // 语法/用词问题
    timestamp: i64,
}

/// 文本问题
#[derive(Debug, Serialize, Deserialize)]
struct TextIssue {
    #[serde(rename = "type")]
    issue_type: String, // grammar | word_choice | suggestion
    original: String,
    suggested: String,
    description_en: String,
    description_zh: String,
    severity: String, // low | medium | high

    #[serde(default)]
    start_position: Option<i32>,
    #[serde(default)]
    end_position: Option<i32>,
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let api_key =
        std::env::var("DOUBAO_API_KEY").wrap_err("DOUBAO_API_KEY environment variable not set")?;
    log::info!("DOUBAO_API_KEY {api_key}");

    let model =
        std::env::var("DOUBAO_MODEL").unwrap_or_else(|_| "doubao-seed-1-8-251228".to_string());

    let system_prompt =
        std::env::var("SYSTEM_PROMPT").unwrap_or_else(|_| DEFAULT_SYSTEM_PROMPT.to_string());

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
                                    (
                                        asr_result.text,
                                        asr_result.session_id,
                                        Some(asr_result.words),
                                    )
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
                let response = generate_comprehensive_response(
                    &client,
                    &api_key,
                    &model,
                    &system_prompt,
                    &user_text,
                    &history.lock().unwrap(),
                    &session,
                    words.as_ref(),
                )
                .await;
                match response {
                    Ok(response) => {
                        log::info!("AI reply (en): {}", response.reply_en);
                        log::info!("AI reply (zh): {}", response.reply_zh);
                        log::info!("Found issues: {:#?}", response.issues,);

                        // 添加 AI 回复到历史
                        {
                            let mut hist = history.lock().unwrap();
                            hist.add_assistant_message(&response.reply_en);
                        }

                        println!("====================techer 2");
                        // 发送综合 JSON 输出 (json_data)
                        let output_str = serde_json::to_string(&response)?;
                        let output_array = StringArray::from(vec![output_str.as_str()]);
                        node.send_output(
                            "json_data".to_string().into(),
                            metadata.parameters.clone(),
                            output_array,
                        )?;

                        println!("====================techer 3");
                        // 发送状态
                        let status = json!({
                            "node": "english-teacher",
                            "status": "ok",
                            "session_id": session,
                        });

                        let status_array = StringArray::from(vec![status.to_string().as_str()]);
                        println!("====================techer 4");
                        node.send_output(
                            "status".to_string().into(),
                            metadata.parameters.clone(),
                            status_array,
                        )?;
                    }
                    Err(e) => {
                        log::error!("Failed to generate comprehensive response: {}", e);

                        let status = json!({
                            "node": "english-teacher",
                            "status": "error",
                            "error": e.to_string(),
                        });

                        let status_array = StringArray::from(vec![status.to_string().as_str()]);
                        node.send_output(
                            "status".to_string().into(),
                            metadata.parameters.clone(),
                            status_array,
                        )?;
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
8. Try to guide the conversation to follow up with your answers. For example, if a user asks you what you like, after you answer, you should ask the user what they like in return.

Remember: Your goal is to help the user practice speaking naturally, not to lecture them."#;

/// 使用 structured outputs 一次性生成 AI 回复和语法分析
async fn generate_comprehensive_response(
    client: &Client,
    api_key: &str,
    model: &str,
    system_prompt: &str,
    user_text: &str,
    history: &ConversationHistory,
    session_id: &str,
    words: Option<&Vec<WordTiming>>,
) -> Result<ComprehensiveResponse> {
    // Use Chat Completions API with response_format for structured outputs
    // Per https://www.volcengine.com/docs/82379/1568221

    let mut messages = vec![json!({
        "role": "system",
        "content": format!(
            "{}\n\nIMPORTANT: You must respond with a JSON object containing:\n\
            1. A natural conversational reply to the user\n\
            2. Grammar/vocabulary analysis of the user's last message",
            system_prompt
        )
    })];

    // Add conversation history
    for msg in history.get_messages() {
        messages.push(json!({
            "role": msg.role,
            "content": msg.content,
        }));
    }

    // JSON Schema for structured output
    let response_schema = json!({
        "type": "object",
        "properties": {
            "use_lang": {
                "type": "string",
                "description": "The language of the original text, either 'en' for English, 'zh' for Chinese, or 'mix' for mixed. ONLY contains issues if this value is 'en'."
            },
            "original_en": {
                "type": "string",
                "description": "The original user text in English. If the user wrote in Chinese or mixed language, translate it to English here."
            },
            "original_zh": {
                "type": "string",
                "description": "The original user text in Chinese. If the user wrote in English or mixed language, translate it to Chinese here."
            },
            "reply_en": {
                "type": "string",
                "description": "Your natural conversational response to the user in English. Keep it concise and encouraging."
            },
            "reply_zh": {
                "type": "string",
                "description": "Translation of your reply_en into Chinese."
            },
            "issues": {
                "type": "array",
                "description": "Grammar, word choice, or phrasing issues found in the user's last message. Empty array if no issues.",
                "items": {
                    "type": "object",
                    "properties": {
                        "type": {
                            "type": "string",
                            "enum": ["grammar", "word_choice", "suggestion"],
                            "description": "Type of issue"
                        },
                        "original": {
                            "type": "string",
                            "description": "The problematic text from user's message"
                        },
                        "suggested": {
                            "type": "string",
                            "description": "The corrected or better alternative"
                        },
                        "description_en": {
                            "type": "string",
                            "description": "Explanation of the issue using simple English"
                        },
                        "description_zh": {
                            "type": "string",
                            "description": "Explanation of the issue using simple Chinese"
                        },
                        "severity": {
                            "type": "string",
                            "enum": ["low", "medium", "high"],
                            "description": "Severity level of the issue"
                        },
                        "start_position": {
                            "type": ["integer", "null"],
                            "description": "0-based character offset where issue starts (null if unknown)"
                        },
                        "end_position": {
                            "type": ["integer", "null"],
                            "description": "0-based character offset where issue ends, exclusive (null if unknown)"
                        }
                    },
                    "required": ["type", "original", "suggested", "description_en", "description_zh", "severity"],
                    "additionalProperties": false
                }
            }
        },
        "required": ["use_lang", "original_en", "original_zh", "reply_en", "reply_zh", "issues"],
        "additionalProperties": false
    });

    let payload = json!({
        "model": model,
        "messages": messages,
        "response_format": {
            "type": "json_schema",
            "json_schema": {
                "name": "english_teacher_response",
                "strict": true,
                "schema": response_schema
            }
        },
        "temperature": 0.7,
        "max_tokens": 2000
    });

    println!("Request payload");
    let response = client
        .post("https://ark.cn-beijing.volces.com/api/v3/chat/completions")
        .header(header::AUTHORIZATION, format!("Bearer {}", api_key))
        .header(header::CONTENT_TYPE, "application/json")
        .json(&payload)
        .send()
        .await?;

    println!("Request payload 1");
    if !response.status().is_success() {
        let error_text = response.text().await?;
        eyre::bail!("API error: {}", error_text);
    }

    println!("Request payload 2");
    let result: serde_json::Value = response.json().await?;
    println!("Request payload 13");

    // Extract content from Chat Completions response
    let content = result["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| eyre::eyre!("No content in response"))?;

    println!("Request payload 4");
    log::debug!("Structured response: {}", content);

    // Parse structured JSON response
    let structured: serde_json::Value = serde_json::from_str(content)?;

    let use_lang = structured["use_lang"]
        .as_str()
        .ok_or_else(|| eyre::eyre!("Missing use_lang in structured response"))?
        .to_string();

    let original_en = structured["original_en"]
        .as_str()
        .ok_or_else(|| eyre::eyre!("Missing original_en in structured response"))?
        .to_string();

    let original_zh = structured["original_zh"]
        .as_str()
        .ok_or_else(|| eyre::eyre!("Missing original_zh in structured response"))?
        .to_string();

    let reply_en = structured["reply_en"]
        .as_str()
        .ok_or_else(|| eyre::eyre!("Missing reply_en in structured response"))?
        .to_string();

    let reply_zh = structured["reply_zh"]
        .as_str()
        .ok_or_else(|| eyre::eyre!("Missing reply_zh in structured response"))?
        .to_string();

    let issues: Vec<TextIssue> =
        serde_json::from_value(structured["issues"].clone()).unwrap_or_default();

    Ok(ComprehensiveResponse {
        session_id: session_id.to_string(),
        use_lang,
        original_en,
        original_zh,
        reply_en,
        reply_zh,
        issues,
        timestamp: chrono::Utc::now().timestamp(),
    })
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
