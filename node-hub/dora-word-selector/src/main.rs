// Dora Node: Word Selector
// Selects 20-30 words from the database for review based on spaced repetition

use dora_node_api::{DoraNode, Event, arrow::array::{Array, StringArray, UInt8Array}, ArrowData};
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
    issue_description: Option<String>,
    difficulty_level: i64,
    context: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct WordSelectionOutput {
    words: Vec<String>,
    word_details: Vec<IssueWord>,
    session_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ControlCommand {
    command: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite://learning_companion.db".to_string());
    
    log::info!("Connecting to database: {}", database_url);
    let pool = SqlitePool::connect(&database_url)
        .await
        .wrap_err("Failed to connect to database")?;

    log::info!("Running database migrations...");
    sqlx::migrate!("../../apps/colang/migrations")
        .run(&pool)
        .await
        .wrap_err("Failed to run migrations")?;

    let (mut node, mut events) = DoraNode::init_from_env()?;
    
    let mut current_session_id: Option<String> = None;
    let min_words = std::env::var("MIN_WORDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(20);
    
    let max_words = std::env::var("MAX_WORDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(30);

    log::info!("Word Selector node started (min: {}, max: {})", min_words, max_words);

    while let Some(event) = events.recv() {
        match event {
            Event::Input { id, data, metadata } => {
                let raw_data = extract_bytes(&data);
                match id.as_str() {
                    "trigger" => {
                        log::info!("Received trigger to select words");
                        
                        // Select words from database
                        match select_words(&pool, max_words).await {
                            Ok(words) => {
                                // Generate new session ID
                                let session_id = uuid::Uuid::new_v4().to_string();
                                current_session_id = Some(session_id.clone());
                                
                                let word_strings: Vec<String> = words.iter()
                                    .map(|w| w.word.clone())
                                    .collect();
                                
                                let output = WordSelectionOutput {
                                    words: word_strings.clone(),
                                    word_details: words.clone(),
                                    session_id: session_id.clone(),
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
                                };
                                let output_json = serde_json::to_string(&output)?;
                                let output_array = StringArray::from(vec![output_json.as_str()]);
                                node.send_output("selected_words".to_string().into(), metadata.parameters.clone(), output_array)?;
                            }
                        }
                    }
                    "control" => {
                        // Handle control commands (reset, pause, etc.)
                        if let Ok(cmd) = serde_json::from_slice::<ControlCommand>(&raw_data) {
                            log::info!("Received control command: {:?}", cmd.command);
                            
                            match cmd.command.as_str() {
                                "reset" => {
                                    current_session_id = None;
                                    log::info!("Session reset");
                                }
                                _ => {}
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

fn extract_bytes(data: &ArrowData) -> Vec<u8> {
    // Try to extract as StringArray first
    if let Some(array) = data.0.as_any().downcast_ref::<StringArray>() {
        if array.len() > 0 {
            return array.value(0).as_bytes().to_vec();
        }
    }
    // Try as UInt8Array
    if let Some(array) = data.0.as_any().downcast_ref::<UInt8Array>() {
        return array.values().to_vec();
    }
    Vec::new()
}

async fn select_words(pool: &SqlitePool, limit: i64) -> Result<Vec<IssueWord>> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_secs() as i64;
    
    let one_day_ago = now - 86400;

    let rows = sqlx::query(
        r#"
        SELECT 
            w.id, w.word, w.issue_type, w.issue_description,
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
    .bind(limit)
    .fetch_all(pool)
    .await?;

    let mut words = Vec::new();
    for row in rows {
        words.push(IssueWord {
            id: row.get("id"),
            word: row.get("word"),
            issue_type: row.get("issue_type"),
            issue_description: row.get("issue_description"),
            difficulty_level: row.get("difficulty_level"),
            context: row.get("context"),
        });
    }

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

    Ok(())
}
