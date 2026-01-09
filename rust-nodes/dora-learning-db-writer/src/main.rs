// Dora Node: Learning DB Writer
// 专门负责将英语学习问题写入数据库
// 接收分析结果，存储到 issue_words 和 conversation_annotations 表

use dora_node_api::{DoraNode, Event, arrow::array::{Array, StringArray, UInt8Array}};
use eyre::{Context, Result};
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Serialize, Deserialize)]
struct AnalysisOutput {
    session_id: String,
    user_text: String,
    issues: Vec<TextIssue>,
    pronunciation_issues: Vec<PronunciationIssue>,
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
struct StorageResult {
    success: bool,
    issues_stored: usize,
    pronunciation_issues_stored: usize,
    error: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite://learning_companion.db".to_string());
    
    log::info!("DB Writer connecting to database: {}", database_url);
    let pool = SqlitePool::connect(&database_url)
        .await
        .wrap_err("Failed to connect to database")?;

    log::info!("Running database migrations...");
    sqlx::migrate!("../../apps/colang/migrations")
        .run(&pool)
        .await
        .wrap_err("Failed to run migrations")?;

    let (mut node, mut events) = DoraNode::init_from_env()?;
    
    log::info!("Learning DB Writer node started");

    while let Some(event) = events.recv() {
        match event {
            Event::Input { id, data, metadata } => {
                match id.as_str() {
                    "analysis" => {
                        log::info!("Received analysis data for storage");
                        
                        let raw_data = extract_bytes(&data);
                        if raw_data.is_empty() {
                            log::warn!("Empty analysis data received");
                            continue;
                        }
                        
                        match serde_json::from_slice::<AnalysisOutput>(&raw_data) {
                            Ok(analysis) => {
                                log::info!(
                                    "Storing analysis: {} text issues, {} pronunciation issues",
                                    analysis.issues.len(),
                                    analysis.pronunciation_issues.len()
                                );
                                
                                let mut result = StorageResult {
                                    success: true,
                                    issues_stored: 0,
                                    pronunciation_issues_stored: 0,
                                    error: None,
                                };
                                
                                // Get conversation ID
                                let conv_id = match get_or_create_conversation_id(
                                    &pool,
                                    &analysis.session_id,
                                    &analysis.user_text
                                ).await {
                                    Ok(id) => id,
                                    Err(e) => {
                                        log::error!("Failed to get conversation ID: {}", e);
                                        result.success = false;
                                        result.error = Some(e.to_string());
                                        send_result(&mut node, &metadata, &result)?;
                                        continue;
                                    }
                                };
                                
                                // Store text issues
                                for issue in &analysis.issues {
                                    match save_text_issue(&pool, conv_id, issue, &analysis.user_text).await {
                                        Ok(_) => result.issues_stored += 1,
                                        Err(e) => {
                                            log::error!("Failed to save text issue: {}", e);
                                            result.success = false;
                                        }
                                    }
                                }
                                
                                // Store pronunciation issues
                                for p_issue in &analysis.pronunciation_issues {
                                    match save_pronunciation_issue(
                                        &pool,
                                        &p_issue.word,
                                        p_issue.confidence,
                                        &analysis.user_text
                                    ).await {
                                        Ok(_) => result.pronunciation_issues_stored += 1,
                                        Err(e) => {
                                            log::error!("Failed to save pronunciation issue: {}", e);
                                            result.success = false;
                                        }
                                    }
                                }
                                
                                log::info!(
                                    "Storage complete: {} text issues, {} pronunciation issues",
                                    result.issues_stored,
                                    result.pronunciation_issues_stored
                                );
                                
                                send_result(&mut node, &metadata, &result)?;
                            }
                            Err(e) => {
                                log::error!("Failed to parse analysis output: {}", e);
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

fn extract_bytes(data: &dora_node_api::ArrowData) -> Vec<u8> {
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

fn send_result(
    node: &mut DoraNode,
    _metadata: &dora_node_api::Metadata,
    result: &StorageResult
) -> Result<()> {
    let output_str = serde_json::to_string(result)?;
    let output_array = StringArray::from(vec![output_str.as_str()]);
    node.send_output("result".into(), Default::default(), output_array)?;
    Ok(())
}

async fn get_or_create_conversation_id(
    pool: &SqlitePool,
    session_id: &str,
    user_text: &str,
) -> Result<i64> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_secs() as i64;
    
    // Try to get existing conversation
    let existing: Option<i64> = sqlx::query_scalar(
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
    
    if let Some(id) = existing {
        return Ok(id);
    }
    
    // Create new conversation
    let result = sqlx::query(
        r#"
        INSERT INTO conversations (session_id, speaker, text, created_at)
        VALUES (?, 'user', ?, ?)
        "#
    )
    .bind(session_id)
    .bind(user_text)
    .bind(now)
    .execute(pool)
    .await?;
    
    Ok(result.last_insert_rowid())
}

async fn save_text_issue(
    pool: &SqlitePool,
    conversation_id: i64,
    issue: &TextIssue,
    context: &str,
) -> Result<()> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_secs() as i64;
    
    // Save annotation
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
    
    // Extract words and save to issue_words
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
                context = excluded.context,
                difficulty_level = MAX(difficulty_level, 3)
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
    confidence: f32,
    context: &str,
) -> Result<()> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_secs() as i64;
    
    let clean_word = word.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
    
    if clean_word.len() < 2 {
        return Ok(());
    }

    let description = format!("Low confidence in pronunciation (confidence: {:.2})", confidence);

    sqlx::query(
        r#"
        INSERT INTO issue_words (
            word, issue_type, issue_description, created_at, pick_count,
            review_interval_days, difficulty_level, context
        ) VALUES (?, 'pronunciation', ?, ?, 0, 1, 2, ?)
        ON CONFLICT(word, issue_type) DO UPDATE SET
            difficulty_level = MIN(difficulty_level + 1, 5),
            issue_description = excluded.issue_description
        "#
    )
    .bind(&clean_word)
    .bind(&description)
    .bind(now)
    .bind(context)
    .execute(pool)
    .await?;

    Ok(())
}
