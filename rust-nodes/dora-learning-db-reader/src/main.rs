// Dora Node: Learning DB Reader
// 专门负责从数据库随机读取问题词汇
// 基于间隔重复算法选择需要复习的词汇

use dora_node_api::{DoraNode, Event, arrow::array::{Array, StringArray, UInt8Array}};
use eyre::{Context, Result};
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;
use sqlx::Row;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct IssueWord {
    id: i64,
    word: String,
    issue_type: String,
    description_en: Option<String>,
    description_zh: Option<String>,
    difficulty_level: i64,
    context: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct WordSelectionOutput {
    words: Vec<String>,
    word_details: Vec<IssueWord>,
    session_id: String,
    total_selected: usize,
}

#[derive(Debug, Serialize, Deserialize)]
struct TriggerCommand {
    command: String,
    min_words: Option<usize>,
    max_words: Option<usize>,
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite://learning_companion.db".to_string());
    
    log::info!("DB Reader connecting to database: {}", database_url);
    let pool = SqlitePool::connect(&database_url)
        .await
        .wrap_err("Failed to connect to database")?;

    // log::info!("Running database migrations...");
    // sqlx::migrate!("../.. /apps/colang/migrations")
    //     .run(&pool)
    //     .await
    //     .wrap_err("Failed to run migrations")?;

    let (mut node, mut events) = DoraNode::init_from_env()?;
    
    let default_min_words = std::env::var("MIN_WORDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(20);
    
    let default_max_words = std::env::var("MAX_WORDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(30);

    log::info!(
        "Learning DB Reader node started (default min: {}, max: {})",
        default_min_words,
        default_max_words
    );

    while let Some(event) = events.recv() {
        match event {
            Event::Input { id, data, metadata } => {
                match id.as_str() {
                    "trigger" => {
                        log::info!("Received trigger to select words");
                        
                        let raw_data = extract_bytes(&data);
                        let (min_words, max_words) = if !raw_data.is_empty() {
                            // Try to parse trigger command
                            match serde_json::from_slice::<TriggerCommand>(&raw_data) {
                                Ok(cmd) => (
                                    cmd.min_words.unwrap_or(default_min_words),
                                    cmd.max_words.unwrap_or(default_max_words),
                                ),
                                Err(_) => (default_min_words, default_max_words),
                            }
                        } else {
                            (default_min_words, default_max_words)
                        };
                        
                        log::info!("Selecting words (min: {}, max: {})", min_words, max_words);
                        
                        // Select words from database
                        match select_words(&pool, max_words).await {
                            Ok(words) => {
                                // Generate new session ID
                                let session_id = uuid::Uuid::new_v4().to_string();
                                
                                let word_strings: Vec<String> = words.iter()
                                    .map(|w| w.word.clone())
                                    .collect();
                                
                                let output = WordSelectionOutput {
                                    words: word_strings.clone(),
                                    word_details: words.clone(),
                                    session_id: session_id.clone(),
                                    total_selected: words.len(),
                                };
                                
                                let output_json = serde_json::to_string(&output)?;
                                let output_array = StringArray::from(vec![output_json.as_str()]);
                                node.send_output("selected_words".to_string().into(), metadata.parameters.clone(), output_array)?;
                                
                                log::info!(
                                    "Selected {} words for session {}: {:?}",
                                    words.len(),
                                    session_id,
                                    word_strings
                                );

                                // Create learning session in database
                                if let Err(e) = create_learning_session(&pool, &session_id, &words).await {
                                    log::error!("Failed to create learning session: {}", e);
                                }
                            }
                            Err(e) => {
                                log::error!("Failed to select words: {}", e);
                                
                                // Send empty result
                                let output = WordSelectionOutput {
                                    words: vec![],
                                    word_details: vec![],
                                    session_id: uuid::Uuid::new_v4().to_string(),
                                    total_selected: 0,
                                };
                                let output_json = serde_json::to_string(&output)?;
                                let output_array = StringArray::from(vec![output_json.as_str()]);
                                node.send_output("selected_words".to_string().into(), metadata.parameters.clone(), output_array)?;
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

async fn select_words(pool: &SqlitePool, limit: usize) -> Result<Vec<IssueWord>> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_secs() as i64;
    
    let one_day_ago = now - 86400; // 24 hours in seconds

    // Select words based on spaced repetition:
    // 1. Priority to words due for review (next_review_at <= now)
    // 2. Limit to 5 times per day per word
    // 3. Sort by difficulty and creation date
    let rows = sqlx::query(
        r#"
        SELECT 
            w.id, w.word, w.issue_type, w.description_en, w.description_zh,
            w.difficulty_level, w.context,
            COALESCE(
                (SELECT COUNT(*) FROM word_practice_log 
                 WHERE word_id = w.id 
                 AND practiced_at >= ?),
                0
            ) as today_count
        FROM issue_words w
        WHERE 
            (w.next_review_at IS NULL OR w.next_review_at <= ?)
        HAVING today_count < 5
        ORDER BY 
            CASE WHEN w.next_review_at IS NULL THEN 0 ELSE 1 END,
            w.next_review_at ASC,
            w.difficulty_level DESC,
            w.created_at ASC
        LIMIT ?
        "#
    )
    .bind(one_day_ago)
    .bind(now)
    .bind(limit as i64)
    .fetch_all(pool)
    .await?;

    let mut words = Vec::new();
    for row in rows {
        words.push(IssueWord {
            id: row.get("id"),
            word: row.get("word"),
            issue_type: row.get("issue_type"),
            description_en: row.get("description_en"),
            description_zh: row.get("description_zh"),
            difficulty_level: row.get("difficulty_level"),
            context: row.get("context"),
        });
    }

    log::info!("Found {} words for review", words.len());
    Ok(words)
}

async fn create_learning_session(
    pool: &SqlitePool,
    session_id: &str,
    words: &[IssueWord],
) -> Result<()> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_secs() as i64;
    
    let target_words_json = serde_json::to_string(
        &words.iter().map(|w| &w.word).collect::<Vec<_>>()
    )?;

    sqlx::query(
        r#"
        INSERT INTO learning_sessions (
            session_id, topic, target_words, started_at, total_exchanges
        ) VALUES (?, ?, ?, ?, ?)
        "#
    )
    .bind(session_id)
    .bind("English Learning Session")
    .bind(target_words_json)
    .bind(now)
    .bind(0)
    .execute(pool)
    .await?;

    log::info!("Created learning session: {}", session_id);
    Ok(())
}
