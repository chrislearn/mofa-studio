// Dora Node: Doubao TTS (Text-to-Speech)
// Converts AI text responses to speech using Doubao Volcanic Engine Bidirectional WebSocket API
// 不再直接操作数据库，由 history-db-writer 负责保存对话历史

use std::fs;

use dora_node_api::{
    arrow::array::{Array, StringArray, UInt8Array},
    ArrowData, DoraNode, Event,
};
use eyre::{Context, Result};
use minimp3::{Decoder, Frame};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

/// 综合响应格式 (来自 english-teacher/json_data)
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


/// 简单文本输入格式
#[derive(Debug, Serialize, Deserialize)]
struct TextInput {
    text: String,
    session_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct AudioMetadata {
    duration_ms: u64,
    format: String,
    sample_rate: u32,
    bytes: usize,
}

// WebSocket 协议相关常量
const PROTOCOL_VERSION: u8 = 0x11; // v1, 4-byte header
const MSG_TYPE_FULL_CLIENT: u8 = 0x14; // Full-client request with event
const MSG_TYPE_FULL_SERVER: u8 = 0x94; // Full-server response with event
const MSG_TYPE_AUDIO_ONLY: u8 = 0xB4; // Audio-only response with event
const SERIALIZATION_JSON: u8 = 0x10; // JSON serialization
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
                            comprehensive_response.reply_en
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
                            Ok((audio_bytes, audio_metadata)) => {
                                log::info!("TTS generated {} bytes", audio_bytes.len());

                                // Convert MP3 bytes to Float32 mono samples for audio player
                                // Decode MP3 to PCM samples using minimp3
                                // minimp3 Frame.data contains interleaved i16 samples: [L, R, L, R, ...] for stereo
                                
                                let mut decoder = Decoder::new(&audio_bytes[..]);
                                let mut audio_samples: Vec<f32> = Vec::new();
                                let mut actual_sample_rate = 24000; // Default from API config
                                let mut total_channels = 1;
                                
                                loop {
                                    match decoder.next_frame() {
                                        Ok(Frame { data, sample_rate, channels, .. }) => {
                                            actual_sample_rate = sample_rate as u32;
                                            total_channels = channels;
                                            
                                            log::debug!("Frame: {} samples, {} channels, {} Hz", 
                                                data.len(), channels, sample_rate);
                                            
                                            // Convert i16 PCM to f32 normalized samples
                                            // data is Vec<i16> with interleaved channels: [L,R,L,R,...] for stereo
                                            if channels == 2 {
                                                // Stereo: data is interleaved [L0, R0, L1, R1, ...]
                                                // Convert to mono by averaging left and right
                                                for chunk in data.chunks_exact(2) {
                                                    let left = chunk[0] as f32 / 32768.0;
                                                    let right = chunk[1] as f32 / 32768.0;
                                                    audio_samples.push((left + right) / 2.0);
                                                }
                                            } else if channels == 1 {
                                                // Mono: direct conversion
                                                for &sample in &data {
                                                    audio_samples.push(sample as f32 / 32768.0);
                                                }
                                            } else {
                                                // Multi-channel: just take first channel
                                                for chunk in data.chunks(channels) {
                                                    if let Some(&sample) = chunk.first() {
                                                        audio_samples.push(sample as f32 / 32768.0);
                                                    }
                                                }
                                            }
                                        }
                                        Err(minimp3::Error::Eof) => break,
                                        Err(e) => {
                                            log::error!("MP3 decode error: {}", e);
                                            break;
                                        }
                                    }
                                }
                                
                                log::info!("Decoded {} MP3 bytes to {} mono samples at {}Hz (source: {} channels)", 
                                    audio_bytes.len(), audio_samples.len(), actual_sample_rate, total_channels);

                                let audio_array =
                                    dora_node_api::arrow::array::ListArray::from_iter_primitive::<
                                        dora_node_api::arrow::datatypes::Float32Type,
                                        _,
                                        _,
                                    >(std::iter::once(Some(
                                        audio_samples.iter().map(|&s| Some(s)),
                                    )));

                                // Add sample rate to metadata parameters for audio player
                                let mut output_params = metadata.parameters.clone();
                                output_params.insert(
                                    "sample_rate".to_string(),
                                    dora_node_api::Parameter::Integer(actual_sample_rate as i64),
                                );

                                node.send_output(
                                    "audio_bytes".to_string().into(),
                                    output_params.clone(),
                                    audio_array,
                                )?;

                                let metadata_json = serde_json::to_string(&audio_metadata)?;
                                let audio_metadata =
                                    StringArray::from(vec![metadata_json.as_str()]);

                                node.send_output(
                                    "audio_metadata".to_string().into(),
                                    output_params.clone(),
                                    audio_metadata,
                                )?;

                                let status = json!({
                                    "node": "doubao-tts",
                                    "status": "ok",
                                });
                                let status_array =
                                    StringArray::from(vec![status.to_string().as_str()]);
                                node.send_output(
                                    "status".to_string().into(),
                                    output_params,
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
) -> Result<(Vec<u8>, AudioMetadata)> {
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

    // 等待 ConnectionStarted
    wait_for_event(&mut read, EVENT_CONNECTION_STARTED).await?;
    log::debug!("Received ConnectionStarted");

    // 2. 发送 StartSession
    let user_id = uuid::Uuid::new_v4().to_string();
    let start_session_payload = json!({
        "user": {
            "uid": user_id
        },
        "event": EVENT_START_SESSION,
        "namespace": "BidirectionalTTS",
        "req_params": {
            "speaker": speaker,
            "audio_params": {
                "format": "mp3",
                "sample_rate": 48000,
                "speech_rate": speech_rate,
                "enable_timestamp": true
            },
            "additions": json!({
                "disable_markdown_filter": false
            }).to_string()
        }
    });
    let start_session_frame = build_event_frame(
        EVENT_START_SESSION,
        Some(&session_id),
        start_session_payload,
    );
    write.send(Message::Binary(start_session_frame)).await?;
    log::debug!("Sent StartSession");

    // 等待 SessionStarted
    wait_for_event(&mut read, EVENT_SESSION_STARTED).await?;
    log::debug!("Received SessionStarted");

    // 3. 发送 TaskRequest (文本)
    let task_payload = json!({
        "user": {
            "uid": user_id
        },
        "event": EVENT_TASK_REQUEST,
        "namespace": "BidirectionalTTS",
        "req_params": {
            "speaker": speaker,
            "audio_params": {
                "format": "mp3",
                "sample_rate": 24000,
                "speech_rate": speech_rate,
                "enable_timestamp": true
            },
            "text": text,
            "additions": json!({
                "disable_markdown_filter": false
            }).to_string()
        }
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
                println!("==================10 -- 0?");
                log::debug!("Received event: {}", event);
                println!("Received eventd: {}", event);

                match event {
                    EVENT_TTS_RESPONSE => {
                        println!("==================11");
                        // 提取音频数据
                        if let Some(audio) = extract_audio_from_frame(&data) {
                            println!("==================12 audio {}", audio.len());
                            audio_data.extend_from_slice(&audio);
                            println!("==================12 audio 1");
                        }
                    }
                    EVENT_SESSION_FINISHED => {
                        println!("==================13");
                        log::debug!("Session finished");
                        break;
                    }
                    _ => {
                        println!("xxxxxxxxxxxx ? 4");
                        println!("Received evenxxx: {} {:?}", event, data);
                    }
                }
                println!("xxxxxxxxxxxx ? 5");
            }
            Some(Ok(Message::Text(txt))) => {
                log::warn!("Received unexpected text message: {}", txt);
            }
            // Some(Ok(Message::Ping(_))) => {
            //     log::warn!("Received  ping message");
            //     break;
            // }
            Some(Err(e)) => {
                log::error!("WebSocket error: {}", e);
                break;
            }
            None => {
                log::debug!("WebSocket stream ended");
                break;
            }
            _ => {
                break;
            }
        }
    }

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
    if fs::metadata(output_path).is_ok() {
        fs::remove_file(output_path).wrap_err("Failed to remove existing test_output.mp3")?;
    }
    std::fs::write(output_path, &audio_data)
        .wrap_err("Failed to write audio data to test_output.mp3")?;
    log::info!(
        "Audio saved to test_output.mp3 ({} bytes)",
        audio_data.len()
    );

    // Calculate actual duration using minimp3
    let (duration_ms, sample_rate) = calculate_mp3_duration(&audio_data)?;

    let metadata = AudioMetadata {
        duration_ms,
        format: "mp3".to_string(),
        sample_rate,
        bytes: audio_data.len(),
    };

    Ok((audio_data, metadata))
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
    let payload = payload.to_string();
    let payload = payload.as_bytes();
    frame.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    frame.extend_from_slice(payload);

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

fn calculate_mp3_duration(audio_data: &[u8]) -> Result<(u64, u32)> {
    let mut decoder = Decoder::new(std::io::Cursor::new(audio_data));
    let mut total_samples = 0u64;
    let mut sample_rate = 24000u32;

    loop {
        match decoder.next_frame() {
            Ok(Frame {
                data,
                sample_rate: rate,
                channels,
                ..
            }) => {
                sample_rate = rate as u32;
                // data.len() is the number of samples (for all channels)
                // Divide by channels to get actual number of sample frames
                total_samples += (data.len() / channels) as u64;
            }
            Err(minimp3::Error::Eof) => break,
            Err(e) => {
                log::warn!("Error decoding MP3 for duration calculation: {}", e);
                break;
            }
        }
    }

    let duration_ms = if sample_rate > 0 {
        (total_samples * 1000) / sample_rate as u64
    } else {
        0
    };

    Ok((duration_ms, sample_rate))
}

fn extract_audio_from_frame(data: &[u8]) -> Option<Vec<u8>> {
    // 检查消息类型
    if data.len() < 4 {
        return None;
    }

    let msg_type = data[1];

    // 如果是 audio-only 响应 (0xB4)
    if msg_type == MSG_TYPE_AUDIO_ONLY {
        println!("==MSG_TYPE_AUDIO_ONLY");
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
        println!("==MSG_TYPE_FULL_SERVER");
        // 需要解析 JSON 然后提取音频
        // 这里简化处理,返回 None
    }

    None
}
