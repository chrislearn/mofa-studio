// Dora Node: Conversation Analyzer
// Analyzes user speech for pronunciation, grammar, and vocabulary issues

use dora_node_api::{DoraNode, Event, arrow::array::{Array, StringArray, UInt8Array}};
use eyre::{Context, Result};
use reqwest::{Client, header};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::sqlite::SqlitePool;
use std::time::{SystemTime, UNIX_EPOCH};

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
struct AnalysisResult {
    session_id: String,
    issues_found: usize,
    new_words_added: usize,
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    
    let api_key = std::env::var("DOUBAO_API_KEY")
        .wrap_err("DOUBAO_API_KEY environment variable not set")?;
    
    let model = std::env::var("DOUBAO_MODEL")
        .unwrap_or_else(|_| "doubao-pro-32k".to_string());
    
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite://learning_companion.db".to_string());
    
    let pool = SqlitePool::connect(&database_url).await?;

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let (mut node, mut events) = DoraNode::init_from_env()?;
    
    log::info!("Conversation Analyzer node started");

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
                                            
                                            // Get conversation ID
                                            if let Ok(Some(conv_id)) = get_latest_conversation_id(
                                                &pool,
                                                &session_id
                                            ).await {
                                                // Store issues in database
                                                let mut new_words = 0;
                                                
                                                for issue in &issues {
                                                    // Save annotation
                                                    if let Err(e) = save_annotation(
                                                        &pool,
                                                        conv_id,
                                                        issue
                                                    ).await {
                                                        log::error!("Failed to save annotation: {}", e);
                                                        continue;
                                                    }
                                                    
                                                    // Extract words and save to issue_words
                                                    if let Err(e) = save_issue_word(
                                                        &pool,
                                                        issue,
                                                        &asr_result.text
                                                    ).await {
                                                        log::error!("Failed to save issue word: {}", e);
                                                    } else {
                                                        new_words += 1;
                                                    }
                                                }
                                                
                                                let result = AnalysisResult {
                                                    session_id: session_id.clone(),
                                                    issues_found: issues.len(),
                                                    new_words_added: new_words,
                                                };
                                                
                                                let output_str = serde_json::to_string(&result)?;
                                                let output_array = StringArray::from(vec![output_str.as_str()]);
                                                node.send_output("analysis".to_string().into(), metadata.parameters.clone(), output_array)?;
                                            }
                                        }
                                        Err(e) => {
                                            log::error!("Analysis failed: {}", e);
                                        }
                                    }
                                    
                                    // Analyze pronunciation (low confidence words)
                                    for word_info in &asr_result.words {
                                        if word_info.confidence < 0.7 {
                                            log::info!(
                                                "Low confidence word detected: {} ({})",
                                                word_info.word,
                                                word_info.confidence
                                            );
                                            
                                            if let Err(e) = save_pronunciation_issue(
                                                &pool,
                                                &word_info.word,
                                                &asr_result.text
                                            ).await {
                                                log::error!("Failed to save pronunciation issue: {}", e);
                                            }
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

async fn get_latest_conversation_id(
    pool: &SqlitePool,
    session_id: &str,
) -> Result<Option<i64>> {
    let result = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT id FROM conversations 
        WHERE session_id = ? AND speaker = 'user'
        ORDER BY created_at DESC 
        LIMIT 1
        "#
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await?;

    Ok(result)
}

async fn save_annotation(
    pool: &SqlitePool,
    conversation_id: i64,
    issue: &TextIssue,
) -> Result<()> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_secs() as i64;
    
    let annotation_type = match issue.issue_type.as_str() {
        "grammar" => "grammar_error",
        "word_choice" => "word_choice",
        "suggestion" => "suggestion",
        _ => "correction",
    };

    sqlx::query(
        r#"
        INSERT INTO conversation_annotations (
            conversation_id, annotation_type, original_text, suggested_text,
            description, severity, created_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?)
        "#
    )
    .bind(conversation_id)
    .bind(annotation_type)
    .bind(&issue.original)
    .bind(&issue.suggested)
    .bind(&issue.description)
    .bind(&issue.severity)
    .bind(now)
    .execute(pool)
    .await?;

    Ok(())
}

async fn save_issue_word(
    pool: &SqlitePool,
    issue: &TextIssue,
    context: &str,
) -> Result<()> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_secs() as i64;
    
    // Extract words from original text
    let words: Vec<&str> = issue.original.split_whitespace().collect();
    
    let issue_type_db = match issue.issue_type.as_str() {
        "grammar" => "grammar",
        "word_choice" => "usage",
        _ => "unfamiliar",
    };

    for word in words {
        let clean_word = word.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
        
        if clean_word.len() < 2 {
            continue;
        }

        sqlx::query(
            r#"
            INSERT INTO issue_words (
                word, issue_type, issue_description, created_at, pick_count,
                review_interval_days, difficulty_level, context
            ) VALUES (?, ?, ?, ?, 0, 1, 3, ?)
            ON CONFLICT(word, issue_type) DO UPDATE SET
                issue_description = excluded.issue_description,
                context = excluded.context
            "#
        )
        .bind(&clean_word)
        .bind(issue_type_db)
        .bind(&issue.description)
        .bind(now)
        .bind(context)
        .execute(pool)
        .await?;
    }

    Ok(())
}

async fn save_pronunciation_issue(
    pool: &SqlitePool,
    word: &str,
    context: &str,
) -> Result<()> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_secs() as i64;
    
    let clean_word = word.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase();

    sqlx::query(
        r#"
        INSERT INTO issue_words (
            word, issue_type, issue_description, created_at, pick_count,
            review_interval_days, difficulty_level, context
        ) VALUES (?, 'pronunciation', 'Low confidence in pronunciation', ?, 0, 1, 2, ?)
        ON CONFLICT(word, issue_type) DO UPDATE SET
            difficulty_level = difficulty_level + 1
        "#
    )
    .bind(&clean_word)
    .bind(now)
    .bind(context)
    .execute(pool)
    .await?;

    Ok(())
}
