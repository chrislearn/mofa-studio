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

/// AI 回复输出
#[derive(Debug, Serialize, Deserialize)]
struct TeacherOutput {
    text: String,
    session_id: String,
    timestamp: i64,
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    
    let api_key = std::env::var("DOUBAO_API_KEY")
        .wrap_err("DOUBAO_API_KEY environment variable not set")?;
    println!("=========api_key1: {}", api_key);
    
    let model = std::env::var("DOUBAO_MODEL")
        .unwrap_or_else(|_| "doubao-seed-1-8-251228".to_string());
    println!("=========model1??: {}", model);
    let model = "doubao-seed-1-8-251228".to_string(); // 强制使用该模型，test
    
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
                let (user_text, session_id) = match id.as_str() {
                    "asr_text" => {
                        // 处理 ASR 输出 (JSON 格式)
                        log::info!("Received ASR text");
                        if let Some(bytes) = &raw_data {
                            match serde_json::from_slice::<AsrOutput>(bytes) {
                                Ok(asr_result) => {
                                    if asr_result.text.trim().is_empty() {
                                        continue;
                                    }
                                    (asr_result.text, asr_result.session_id)
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
                            (text, None)
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
                
                // 生成 AI 回复
                match generate_response(
                    &client,
                    &api_key,
                    &model,
                    &system_prompt,
                    &history.lock().unwrap(),
                ).await {
                    Ok(response) => {
                        log::info!("AI response: {}", response);
                        // 添加 AI 回复到历史
                        {
                            let mut hist = history.lock().unwrap();
                            hist.add_assistant_message(&response);
                        }
                        
                        // 发送输出
                        let output = TeacherOutput {
                            text: response.clone(),
                            session_id: session.clone(),
                            timestamp: chrono::Utc::now().timestamp(),
                        };
                        
                        let output_str = serde_json::to_string(&output)?;
                        let output_array = StringArray::from(vec![output_str.as_str()]);
                        
                        node.send_output(
                            "text".to_string().into(),
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
                        log::error!("Failed to generate response: {}", e);
                        
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

/// 生成 AI 回复
async fn generate_response(
    client: &Client,
    api_key: &str,
    model: &str,
    system_prompt: &str,
    history: &ConversationHistory,
) -> Result<String> {
    let mut messages = vec![
        json!({
            "role": "system",
            "content": system_prompt
        })
    ];
    
    // 添加对话历史
    for msg in history.get_messages() {
        messages.push(json!({
            "role": msg.role,
            "content": msg.content
        }));
    }

    let payload = json!({
        "model": model,
        "messages": messages,
        "temperature": 0.7,
        "max_tokens": 500,
        "presence_penalty": 0.1,
        "frequency_penalty": 0.1
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

    Ok(content.to_string())
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
