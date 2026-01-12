// Dora Node: Learning DB Writer
// 英语学习数据库写入器
// 功能：
// 1. 接收 user_text 输入（纯文本），存储用户消息到 conversations 表
// 2. 接收 ai_json 输入（综合JSON），存储用户消息+AI回复+语法分析到数据库

use dora_node_api::{
    arrow::array::{Array, StringArray, UInt8Array},
    DoraNode, Event,
};
use eyre::{Context, Result};
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;
use std::time::{SystemTime, UNIX_EPOCH};

/// 综合响应（来自 english-teacher）
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

/// ASR 输出格式（从 doubao-asr 接收）
#[derive(Debug, Serialize, Deserialize)]
struct AsrOutput {
    text: String,
    confidence: f32,
    #[serde(default)]
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
    description_en: String,
    description_zh: String,
    severity: String,

    #[serde(default)]
    start_position: Option<i32>,
    #[serde(default)]
    end_position: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
struct StorageResult {
    success: bool,
    issues_stored: usize,
    error: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite://learning_companion.db".to_string());

    log::info!(
        "Learning DB Writer connecting to database: {}",
        database_url
    );
    let pool = SqlitePool::connect(&database_url)
        .await
        .wrap_err("Failed to connect to database")?;

    let (mut node, mut events) = DoraNode::init_from_env()?;

    log::info!("Learning DB Writer node started");

    while let Some(event) = events.recv() {
        match event {
            Event::Input { id, data, metadata } => {
                let raw_data = extract_bytes(&data);

                match id.as_str() {
                    "user_text" => {
                        // 处理纯文本用户输入（来自 mofa-text-input 或 doubao-asr）
                        log::info!("Received user text input");

                        if raw_data.is_empty() {
                            log::warn!("Empty user text received");
                            continue;
                        }

                        // 尝试解析为 ASR 输出（JSON）或纯文本
                        let (user_text, session_id) =
                            if let Ok(asr) = serde_json::from_slice::<AsrOutput>(&raw_data) {
                                (
                                    asr.text,
                                    asr.session_id.unwrap_or_else(|| "default".to_string()),
                                )
                            } else {
                                let text = String::from_utf8_lossy(&raw_data).to_string();
                                (text, "default".to_string())
                            };

                        if user_text.trim().is_empty() {
                            log::debug!("Empty text, skipping storage");
                            continue;
                        }

                        log::info!("Storing user message: {}", user_text);

                        let mut result = StorageResult {
                            success: true,
                            issues_stored: 0,
                            error: None,
                        };

                        // match save_comprehensive(&pool, &session_id, "user", &user_text).await {
                        //     Ok(_) => result.conversations_stored += 1,
                        //     Err(e) => {
                        //         log::error!("Failed to save user message: {}", e);
                        //         result.success = false;
                        //         result.error = Some(e.to_string());
                        //     }
                        // }

                        send_result(&mut node, &metadata, &result)?;
                    }
                    "ai_json" => {
                        // 处理综合 JSON 输入（来自 english-teacher/json_data）
                        log::info!("Received AI JSON data");

                        if raw_data.is_empty() {
                            log::warn!("Empty AI JSON received");
                            continue;
                        }

                        match serde_json::from_slice::<ComprehensiveResponse>(&raw_data) {
                            Ok(response) => {
                                log::info!(
                                    "Storing comprehensive response: user='{}', ai='{}', {} issues",
                                    response.original_en,
                                    response.reply_en,
                                    response.issues.len()
                                );

                                let mut result = StorageResult {
                                    success: true,
                                    issues_stored: 0,
                                    error: None,
                                };

                                // 2. 存储 AI 回复到 conversations
                                match save_comprehensive(&pool, &response.session_id, &response)
                                    .await
                                {
                                    Ok(_) => {}
                                    Err(e) => {
                                        log::error!("Failed to save AI conversation: {}", e);
                                        result.success = false;
                                    }
                                }

                                // 3. 获取 conversation ID 用于关联 annotations
                                let conv_id = match get_latest_conversation_id(
                                    &pool,
                                    &response.session_id,
                                    "user",
                                )
                                .await
                                {
                                    Ok(id) => id,
                                    Err(e) => {
                                        log::error!("Failed to get conversation ID: {}", e);
                                        send_result(&mut node, &metadata, &result)?;
                                        continue;
                                    }
                                };

                                // 4. 存储语法/用词问题
                                for issue in &response.issues {
                                    match save_text_issue(
                                        &pool,
                                        conv_id,
                                        issue,
                                        &response.original_en,
                                    )
                                    .await
                                    {
                                        Ok(_) => result.issues_stored += 1,
                                        Err(e) => {
                                            log::error!("Failed to save text issue: {}", e);
                                            result.success = false;
                                        }
                                    }
                                }
                                log::info!("Storage complete, {} issues", result.issues_stored,);

                                send_result(&mut node, &metadata, &result)?;
                            }
                            Err(e) => {
                                log::error!("Failed to parse AI JSON: {}", e);
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

/// 保存对话记录到 conversations 表
async fn save_comprehensive(
    pool: &SqlitePool,
    session_id: &str,
    comprehensive: &ComprehensiveResponse,
) -> Result<i64> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;

    // Update the last user message with comprehensive data
    sqlx::query(
        r#"
        INSERT INTO conversations (session_id, speaker, use_lang, content_en, content_zh, created_at)
        VALUES (?, ?, ?, ?, ?, ?)
        "#,
    )
   .bind(session_id)
    .bind("user")
    .bind(comprehensive.use_lang.as_str())
    .bind(&comprehensive.original_en)
    .bind(&comprehensive.original_zh)
    .bind(now)
    .execute(pool)
    .await?;

    // Determine use_lang based on speaker (user typically uses en, teacher can use both)
    let result = sqlx::query(
        r#"
        INSERT INTO conversations (session_id, speaker, use_lang, content_en, content_zh, created_at)
        VALUES (?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(session_id)
    .bind("teacher")
    .bind("en")
    .bind(&comprehensive.reply_en)
    .bind(&comprehensive.reply_zh)
    .bind(now)
    .execute(pool)
    .await?;

    Ok(result.last_insert_rowid())
}

/// 获取最新的 conversation ID
async fn get_latest_conversation_id(
    pool: &SqlitePool,
    session_id: &str,
    speaker: &str,
) -> Result<i64> {
    let id: i64 = sqlx::query_scalar(
        r#"
        SELECT id FROM conversations 
        WHERE session_id = ? AND speaker = ?
        ORDER BY created_at DESC 
        LIMIT 1
        "#,
    )
    .bind(session_id)
    .bind(speaker)
    .fetch_one(pool)
    .await?;

    Ok(id)
}

/// 保存文本问题到 conversation_annotations 和 issue_words 表
async fn save_text_issue(
    pool: &SqlitePool,
    conversation_id: i64,
    issue: &TextIssue,
    context: &str,
) -> Result<()> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;

    // 保存到 conversation_annotations
    let annotation_type = match issue.issue_type.as_str() {
        "grammar" => "grammar_error",
        "word_choice" => "word_choice",
        "suggestion" => "suggestion",
        _ => "correction",
    };

    sqlx::query(
        r#"
        INSERT INTO conversation_annotations (
            conversation_id, annotation_type,
            start_position, end_position,
            original_text, suggested_text,
            description_en, description_zh, severity, created_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(conversation_id)
    .bind(annotation_type)
    .bind(issue.start_position)
    .bind(issue.end_position)
    .bind(&issue.original)
    .bind(&issue.suggested)
    .bind(&issue.description_en)
    .bind(&issue.description_zh)
    .bind(&issue.severity)
    .bind(now)
    .execute(pool)
    .await?;

    // 提取单词并保存到 issue_words
    let words: Vec<&str> = issue.original.split_whitespace().collect();

    let issue_type_db = match issue.issue_type.as_str() {
        "grammar" => "grammar",
        "word_choice" => "usage",
        _ => "unfamiliar",
    };

    for word in words {
        let clean_word = word
            .trim_matches(|c: char| !c.is_alphanumeric())
            .to_lowercase();

        if clean_word.len() < 2 {
            continue;
        }

        sqlx::query(
            r#"
            INSERT INTO issue_words (
                word, issue_type, description_en, description_zh, created_at, pick_count,
                review_interval_days, difficulty_level, context
            ) VALUES (?, ?, ?, ?, ?, 0, 1, 3, ?)
            ON CONFLICT(word, issue_type) DO UPDATE SET
                description_en = excluded.description_en,
                description_zh = excluded.description_zh,
                context = excluded.context,
                difficulty_level = MAX(difficulty_level, 3)
            "#,
        )
        .bind(&clean_word)
        .bind(issue_type_db)
        .bind(&issue.description_en)
        .bind(&issue.description_zh)
        .bind(now)
        .bind(context)
        .execute(pool)
        .await?;
    }

    Ok(())
}

/// 保存发音问题到 issue_words 表
async fn save_pronunciation_issue(
    pool: &SqlitePool,
    word: &str,
    confidence: f32,
    context: &str,
) -> Result<()> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;

    let clean_word = word
        .trim_matches(|c: char| !c.is_alphanumeric())
        .to_lowercase();

    if clean_word.len() < 2 {
        return Ok(());
    }

    let description = format!(
        "Low confidence in pronunciation (confidence: {:.2})",
        confidence
    );

    sqlx::query(
        r#"
        INSERT INTO issue_words (
            word, issue_type, description_en, description_zh, created_at, pick_count,
            review_interval_days, difficulty_level, context
        ) VALUES (?, 'pronunciation', ?, ?, ?, 0, 1, 2, ?)
        ON CONFLICT(word, issue_type) DO UPDATE SET
            difficulty_level = MIN(difficulty_level + 1, 5),
            description_en = excluded.description_en
        "#,
    )
    .bind(&clean_word)
    .bind(&description)
    .bind(None::<String>) // description_zh placeholder
    .bind(now)
    .bind(context)
    .execute(pool)
    .await?;

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
    result: &StorageResult,
) -> Result<()> {
    let output_str = serde_json::to_string(result)?;
    let output_array = StringArray::from(vec![output_str.as_str()]);
    node.send_output("result".into(), Default::default(), output_array)?;
    Ok(())
}
