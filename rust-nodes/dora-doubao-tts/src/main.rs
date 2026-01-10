// Dora Node: Doubao TTS (Text-to-Speech)
// Converts AI text responses to speech using Doubao Volcanic Engine Bidirectional WebSocket API
// 不再直接操作数据库，由 history-db-writer 负责保存对话历史

use dora_node_api::{
    arrow::array::{Array, StringArray, UInt8Array},
    ArrowData, DoraNode, Event,
};
use eyre::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
/// 综合响应格式 (来自 english-teacher/json_data)
#[derive(Debug, Serialize, Deserialize)]
struct ComprehensiveResponse {
    session_id: String,
    user_text: String,
    reply_text: String,
    issues: Vec<serde_json::Value>,
    pronunciation_issues: Vec<serde_json::Value>,
    timestamp: i64,
}

/// 简单文本输入格式
#[derive(Debug, Serialize, Deserialize)]
struct TextInput {
    text: String,
    session_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct AudioOutput {
    audio_data: Vec<u8>,
    duration_ms: u64,
    format: String,
    sample_rate: u32,
}

// WebSocket 协议相关常量
const PROTOCOL_VERSION: u8 = 0x11; // v1, 4-byte header
const MSG_TYPE_FULL_CLIENT: u8 = 0x14; // Full-client request with event
const MSG_TYPE_FULL_SERVER: u8 = 0x94; // Full-server response with event
const MSG_TYPE_AUDIO_ONLY: u8 = 0xB4; // Audio-only response with event
const SERIALIZATION_JSON: u8 = 0x10; // JSON serialization
const SERIALIZATION_RAW: u8 = 0x00; // Raw (binary audio)
const NO_COMPRESSION: u8 = 0x00;
const RESERVED: u8 = 0x00;

// Event 定义
const EVENT_START_CONNECTION: i32 = 1;
const EVENT_CONNECTION_STARTED: i32 = 50;
const EVENT_START_SESSION: i32 = 100;
const EVENT_SESSION_STARTED: i32 = 150;
const EVENT_TASK_REQUEST: i32 = 200;
const EVENT_TTS_RESPONSE: i32 = 352;
const EVENT_SESSION_FINISHED: i32 = 152;
const EVENT_FINISH_SESSION: i32 = 102;
const EVENT_FINISH_CONNECTION: i32 = 2;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let app_id =
        std::env::var("DOUBAO_APP_ID").wrap_err("DOUBAO_APP_ID environment variable not set")?;

    let api_key =
        std::env::var("DOUBAO_API_KEY").wrap_err("DOUBAO_API_KEY environment variable not set")?;

    let access_token = std::env::var("DOUBAO_ACCESS_TOKEN")
        .wrap_err("DOUBAO_ACCESS_TOKEN environment variable not set")?;

    println!("========app_id: {}, api_key: {}", app_id, api_key);
    let resource_id =
        std::env::var("DOUBAO_RESOURCE_ID").unwrap_or_else(|_| "seed-tts-2.0".to_string()); // 默认使用豆包2.0
    println!("========resource_id: {resource_id}");

    let voice_type =
        std::env::var("VOICE_TYPE").unwrap_or_else(|_| "zh_female_cancan_mars_bigtts".to_string());

    let speed_ratio: i32 = std::env::var("SPEED_RATIO")
        .ok()
        .and_then(|s| s.parse::<f32>().ok())
        .map(|f| ((f - 1.0) * 100.0) as i32) // 转换为 [-50, 100] 范围
        .unwrap_or(0);

    let (mut node, mut events) = DoraNode::init_from_env()?;

    log::info!(
        "Doubao TTS node started (voice: {}, speed: {}, resource: {})",
        voice_type,
        speed_ratio,
        resource_id
    );

    while let Some(event) = events.recv() {
        match event {
            Event::Input { id, data, metadata } => {
                let raw_data = extract_bytes(&data);
                match id.as_str() {
                    "text" => {
                        log::debug!("Received text input");

                        // 尝试解析为 ComprehensiveResponse
                        let text_to_convert = if let Ok(comprehensive_response) =
                            serde_json::from_slice::<ComprehensiveResponse>(&raw_data)
                        {
                            comprehensive_response.reply_text
                        } else if let Ok(text_input) =
                            serde_json::from_slice::<TextInput>(&raw_data)
                        {
                            text_input.text
                        } else {
                            String::from_utf8_lossy(&raw_data).to_string()
                        };

                        if text_to_convert.trim().is_empty() {
                            log::debug!("Empty text, skipping TTS");
                            continue;
                        }

                        log::info!("Converting to speech: {}", text_to_convert);

                        match perform_tts_websocket(
                            &app_id,
                            &api_key,
                            &resource_id,
                            &access_token,
                            &voice_type,
                            speed_ratio,
                            &text_to_convert,
                        )
                        .await
                        {
                            Ok(audio_output) => {
                                log::info!("TTS generated {} bytes", audio_output.audio_data.len());

                                let output_json = serde_json::to_string(&audio_output)?;
                                let output_array = StringArray::from(vec![output_json.as_str()]);
                                node.send_output(
                                    "audio".to_string().into(),
                                    metadata.parameters.clone(),
                                    output_array,
                                )?;

                                let status = json!({
                                    "node": "doubao-tts",
                                    "status": "ok",
                                    "bytes": audio_output.audio_data.len(),
                                });
                                let status_array =
                                    StringArray::from(vec![status.to_string().as_str()]);
                                node.send_output(
                                    "status".to_string().into(),
                                    metadata.parameters.clone(),
                                    status_array,
                                )?;
                            }
                            Err(e) => {
                                log::error!("TTS failed: {}", e);

                                let status = json!({
                                    "node": "doubao-tts",
                                    "status": "error",
                                    "error": e.to_string(),
                                });
                                let status_array =
                                    StringArray::from(vec![status.to_string().as_str()]);
                                node.send_output(
                                    "status".to_string().into(),
                                    metadata.parameters.clone(),
                                    status_array,
                                )?;
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

async fn perform_tts_websocket(
    app_id: &str,
    api_key: &str,
    resource_id: &str,
    access_token: &str,
    speaker: &str,
    speech_rate: i32,
    text: &str,
) -> Result<AudioOutput> {
    let url = "wss://openspeech.bytedance.com/api/v3/tts/bidirection";

    // 根据文档,认证参数应该在 HTTP 头中,而不是 URL 参数中
    // 生成唯一的连接 ID (用于追踪)
    let connect_id = uuid::Uuid::new_v4().to_string();

    // 创建 Authorization header (格式: "Bearer;{token}")
    let authorization = format!("Bearer;{}", api_key);

    // 使用 tokio_tungstenite 的 http 模块创建请求
    let request = tokio_tungstenite::tungstenite::http::Request::builder()
        .uri(url)
        .header("Host", "openspeech.bytedance.com")
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Sec-WebSocket-Version", "13")
        .header(
            "Sec-WebSocket-Key",
            tokio_tungstenite::tungstenite::handshake::client::generate_key(),
        )
        .header("Authorization", &authorization)
        .header("X-Api-App-Key", app_id)
        .header("X-Api-Access-Key", access_token)
        .header("X-Api-Resource-Id", resource_id)
        .header("X-Api-Connect-Id", &connect_id)
        .body(())?;

    println!("===========resource_id in header: {}", resource_id);
    // 建立 WebSocket 连接
    let (ws_stream, response) = connect_async(request).await.unwrap();

    // 打印响应头中的 X-Tt-Logid 便于调试
    if let Some(logid) = response.headers().get("X-Tt-Logid") {
        log::info!("X-Tt-Logid: {:?}", logid);
    }

    let (mut write, mut read) = ws_stream.split();

    // 生成唯一 ID
    let session_id = uuid::Uuid::new_v4().to_string();

    // 1. 发送 StartConnection
    let start_conn_frame = build_event_frame(EVENT_START_CONNECTION, None, json!({}));
    write.send(Message::Binary(start_conn_frame)).await?;
    log::debug!("Sent StartConnection");

    println!("=================2");
    // 等待 ConnectionStarted
    wait_for_event(&mut read, EVENT_CONNECTION_STARTED).await?;
    log::debug!("Received ConnectionStarted");

    // 2. 发送 StartSession
    let session_meta = json!({
        "user": {
            "uid": "user_001"
        },
        "req_params": {
            "speaker": speaker,
            "audio_params": {
                "format": "mp3",
                "sample_rate": 24000,
                "speech_rate": speech_rate
            }
        }
    });
    println!("==================3");
    let start_session_frame =
        build_event_frame(EVENT_START_SESSION, Some(&session_id), session_meta);
    write.send(Message::Binary(start_session_frame)).await?;
    log::debug!("Sent StartSession");

    println!("==================4");
    // 等待 SessionStarted
    wait_for_event(&mut read, EVENT_SESSION_STARTED).await?;
    println!("==================5");
    log::debug!("Received SessionStarted");

    // 3. 发送 TaskRequest (文本)
    let task_payload = json!({
        "text": text
    });
    let task_frame = build_event_frame(EVENT_TASK_REQUEST, Some(&session_id), task_payload);
    println!("==================6");
    write.send(Message::Binary(task_frame)).await?;
    log::debug!("Sent TaskRequest with text");

    println!("==================7");
    // 4. 接收音频数据
    let mut audio_data = Vec::new();
    loop {
        match read.next().await {
            Some(Ok(Message::Binary(data))) => {
                println!("==================8");
                if data.len() < 4 {
                    println!("==================9");
                    continue;
                }

                println!("==================10");
                let event = parse_event(&data)?;
                println!("==================10 -- 0");
                log::debug!("Received event: {}", event);
                println!("Received event: {}", event);

                match event {
                    EVENT_TTS_RESPONSE => {
                        println!("==================11");
                        // 提取音频数据
                        if let Some(audio) = extract_audio_from_frame(&data) {
                            println!("==================12");
                            audio_data.extend_from_slice(&audio);
                        }
                    }
                    EVENT_SESSION_FINISHED => {
                        println!("==================13");
                        log::debug!("Session finished");
                        break;
                    }
                    _ => {
                        log::debug!("Received evenxxx: {}", event);
                    }
                }
            }
            Some(Ok(Message::Text(txt))) => {
                log::warn!("Received unexpected text message: {}", txt);
            }
            Some(Err(e)) => {
                log::error!("WebSocket error: {}", e);
                break;
            }
            None => {
                println!("==================15");
                log::debug!("WebSocket stream ended");
                break;
            }
            _ => {}
        }
    }

    println!("==================x 8");
    // 5. 发送 FinishSession
    let finish_session_frame =
        build_event_frame(EVENT_FINISH_SESSION, Some(&session_id), json!({}));
    write.send(Message::Binary(finish_session_frame)).await.ok();

    println!("==================x 9");
    // 6. 发送 FinishConnection
    let finish_conn_frame = build_event_frame(EVENT_FINISH_CONNECTION, None, json!({}));
    write.send(Message::Binary(finish_conn_frame)).await.ok();

    println!("==================x 10");
    // Save audio data to test_output.mp3 in project root
    let output_path = std::path::Path::new("test_output.mp3");
    std::fs::write(output_path, &audio_data)
        .wrap_err("Failed to write audio data to test_output.mp3")?;
    log::info!(
        "Audio saved to test_output.mp3 ({} bytes)",
        audio_data.len()
    );
    Ok(AudioOutput {
        audio_data,
        duration_ms: 0,
        format: "mp3".to_string(),
        sample_rate: 24000,
    })
}

fn build_event_frame(event: i32, session_id: Option<&str>, payload: serde_json::Value) -> Vec<u8> {
    let mut frame = Vec::new();

    // Header (4 bytes)
    frame.push(PROTOCOL_VERSION); // byte 0
    frame.push(MSG_TYPE_FULL_CLIENT); // byte 1
    frame.push(SERIALIZATION_JSON | NO_COMPRESSION); // byte 2
    frame.push(RESERVED); // byte 3

    // Event number (4 bytes, big-endian)
    frame.extend_from_slice(&event.to_be_bytes());

    // Session ID (if provided)
    if let Some(sid) = session_id {
        let sid_bytes = sid.as_bytes();
        frame.extend_from_slice(&(sid_bytes.len() as u32).to_be_bytes());
        frame.extend_from_slice(sid_bytes);
    }

    // Payload
    let payload_bytes = payload.to_string().into_bytes();
    frame.extend_from_slice(&(payload_bytes.len() as u32).to_be_bytes());
    frame.extend_from_slice(&payload_bytes);

    frame
}

async fn wait_for_event(
    read: &mut futures_util::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    >,
    expected_event: i32,
) -> Result<()> {
    while let Some(Ok(Message::Binary(data))) = read.next().await {
        if data.len() >= 8 {
            let event = i32::from_be_bytes([data[4], data[5], data[6], data[7]]);
            if event == expected_event {
                return Ok(());
            }
        }
    }
    eyre::bail!("Expected event {} not received", expected_event)
}

fn parse_event(data: &[u8]) -> Result<i32> {
    println!("==================parse_event {}", data.len());
    if data.len() < 8 {
        println!("==================parse_event too short");
        eyre::bail!("Frame too short");
    }
    println!("==================parse_event2");
    Ok(i32::from_be_bytes([data[4], data[5], data[6], data[7]]))
}

fn extract_audio_from_frame(data: &[u8]) -> Option<Vec<u8>> {
    // 检查消息类型
    if data.len() < 4 {
        return None;
    }

    let msg_type = data[1];

    // 如果是 audio-only 响应 (0xB4)
    if msg_type == MSG_TYPE_AUDIO_ONLY {
        // Header (4 bytes) + Event (4 bytes) + Session ID length (4 bytes)
        if data.len() < 12 {
            return None;
        }

        let session_id_len = u32::from_be_bytes([data[8], data[9], data[10], data[11]]) as usize;
        let audio_offset = 12 + session_id_len + 4; // +4 for payload size

        if data.len() > audio_offset {
            return Some(data[audio_offset..].to_vec());
        }
    }
    // 如果是 full-server 响应,可能包含混合数据
    else if msg_type == MSG_TYPE_FULL_SERVER {
        // 需要解析 JSON 然后提取音频
        // 这里简化处理,返回 None
    }

    None
}
