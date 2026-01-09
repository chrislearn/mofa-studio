// Dora Node: History DB Writer
// 历史记录写入器 - 保存对话历史到数据库
// 接收 doubao-asr/text 和 english-teacher/text 的文本输出

use dora_node_api::{
    arrow::array::{Array, StringArray, UInt8Array},
    DoraNode, Event,
};
use eyre::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::sqlite::SqlitePool;
use std::time::{SystemTime, UNIX_EPOCH};

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

/// AI 教师输出格式
#[derive(Debug, Serialize, Deserialize)]
struct TeacherOutput {
    text: String,
    session_id: String,
    timestamp: i64,
}

/// 存储结果
#[derive(Debug, Serialize, Deserialize)]
struct StorageResult {
    success: bool,
    speaker: String,
    session_id: String,
    error: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite://learning_companion.db".to_string());

    log::info!("History DB Writer connecting to database: {}", database_url);
    let pool = SqlitePool::connect(&database_url)
        .await
        .wrap_err("Failed to connect to database")?;

    log::info!("Running database migrations...");
    // if let Err(e) = sqlx::migrate!("../../apps/colang/migrations")
    //     .run(&pool)
    //     .await
    // {
    //     log::error!("Database migration failed: {}", e);
    // }

    let (mut node, mut events) = DoraNode::init_from_env()?;

    log::info!("History DB Writer node started");

    while let Some(event) = events.recv() {
        match event {
            Event::Input { id, data, metadata } => {
                let raw_data = extract_bytes(&data);

                match id.as_str() {
                    "asr_text" => {
                        // 保存用户语音转文字的内容
                        log::info!("Received ASR text for storage");

                        if raw_data.is_empty() {
                            log::warn!("Empty ASR data received");
                            continue;
                        }

                        match serde_json::from_slice::<AsrOutput>(&raw_data) {
                            Ok(asr_result) => {
                                if asr_result.text.trim().is_empty() {
                                    log::debug!("Empty ASR text, skipping storage");
                                    continue;
                                }

                                let session_id = asr_result
                                    .session_id
                                    .unwrap_or_else(|| "default".to_string());

                                log::info!("Saving user conversation: {}", asr_result.text);

                                let result = match save_conversation(
                                    &pool,
                                    &session_id,
                                    "user",
                                    &asr_result.text,
                                )
                                .await
                                {
                                    Ok(_) => StorageResult {
                                        success: true,
                                        speaker: "user".to_string(),
                                        session_id: session_id.clone(),
                                        error: None,
                                    },
                                    Err(e) => {
                                        log::error!("Failed to save user conversation: {}", e);
                                        StorageResult {
                                            success: false,
                                            speaker: "user".to_string(),
                                            session_id: session_id.clone(),
                                            error: Some(e.to_string()),
                                        }
                                    }
                                };

                                send_result(&mut node, &metadata, &result)?;
                            }
                            Err(e) => {
                                log::error!("Failed to parse ASR output: {}", e);
                            }
                        }
                    }
                    "ai_text" => {
                        // 保存 AI 回复的内容
                        log::info!("Received AI text for storage");

                        if raw_data.is_empty() {
                            log::warn!("Empty AI text data received");
                            continue;
                        }

                        match serde_json::from_slice::<TeacherOutput>(&raw_data) {
                            Ok(teacher_output) => {
                                if teacher_output.text.trim().is_empty() {
                                    log::debug!("Empty AI text, skipping storage");
                                    continue;
                                }

                                log::info!("Saving AI conversation: {}", teacher_output.text);

                                let result = match save_conversation(
                                    &pool,
                                    &teacher_output.session_id,
                                    "ai",
                                    &teacher_output.text,
                                )
                                .await
                                {
                                    Ok(_) => StorageResult {
                                        success: true,
                                        speaker: "ai".to_string(),
                                        session_id: teacher_output.session_id.clone(),
                                        error: None,
                                    },
                                    Err(e) => {
                                        log::error!("Failed to save AI conversation: {}", e);
                                        StorageResult {
                                            success: false,
                                            speaker: "ai".to_string(),
                                            session_id: teacher_output.session_id.clone(),
                                            error: Some(e.to_string()),
                                        }
                                    }
                                };

                                send_result(&mut node, &metadata, &result)?;
                            }
                            Err(e) => {
                                // 尝试作为纯文本处理
                                let text = String::from_utf8_lossy(&raw_data);
                                if !text.trim().is_empty() {
                                    log::info!("Saving AI text as plain string: {}", text);

                                    let result = match save_conversation(
                                        &pool, "default", "ai", &text,
                                    )
                                    .await
                                    {
                                        Ok(_) => StorageResult {
                                            success: true,
                                            speaker: "ai".to_string(),
                                            session_id: "default".to_string(),
                                            error: None,
                                        },
                                        Err(e) => {
                                            log::error!("Failed to save AI text: {}", e);
                                            StorageResult {
                                                success: false,
                                                speaker: "ai".to_string(),
                                                session_id: "default".to_string(),
                                                error: Some(e.to_string()),
                                            }
                                        }
                                    };

                                    send_result(&mut node, &metadata, &result)?;
                                } else {
                                    log::error!("Failed to parse AI output: {}", e);
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

/// 从 ArrowData 提取字节
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

/// 发送存储结果
fn send_result(
    node: &mut DoraNode,
    metadata: &dora_node_api::Metadata,
    result: &StorageResult,
) -> Result<()> {
    let output_str = serde_json::to_string(result)?;
    let output_array = StringArray::from(vec![output_str.as_str()]);
    node.send_output("result".into(), metadata.parameters.clone(), output_array)?;

    // 发送状态
    let status = json!({
        "node": "history-db-writer",
        "status": if result.success { "ok" } else { "error" },
        "speaker": result.speaker,
    });

    let status_array = StringArray::from(vec![status.to_string().as_str()]);
    node.send_output("status".into(), metadata.parameters.clone(), status_array)?;

    Ok(())
}

/// 保存对话到数据库
async fn save_conversation(
    pool: &SqlitePool,
    session_id: &str,
    speaker: &str,
    text: &str,
) -> Result<i64> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;

    let result = sqlx::query(
        r#"
        INSERT INTO conversations (
            session_id, speaker, content_text, created_at
        ) VALUES (?, ?, ?, ?)
        "#,
    )
    .bind(session_id)
    .bind(speaker)
    .bind(text)
    .bind(now)
    .execute(pool)
    .await?;

    log::info!(
        "Saved conversation: session={}, speaker={}, id={}",
        session_id,
        speaker,
        result.last_insert_rowid()
    );

    Ok(result.last_insert_rowid())
}
