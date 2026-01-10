// Doubao (豆包) Volcanic Engine API Integration
// Provides ASR (Speech Recognition), TTS (Text-to-Speech), and Chat capabilities

use reqwest::{Client, header};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::error::Error;
use futures::stream::StreamExt;

const DOUBAO_API_BASE: &str = "https://openspeech.bytedance.com/api/v1";
const DOUBAO_CHAT_API_BASE: &str = "https://ark.cn-beijing.volces.com/api/v3";

#[derive(Debug, Clone)]
pub struct DoubaoClient {
    client: Client,
    app_id: String,
    access_token: String,
    chat_api_key: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AsrRequest {
    pub audio_format: String, // "wav", "mp3", "pcm"
    pub sample_rate: u32,
    pub language: String, // "en", "zh", "auto"
    pub audio_data: Vec<u8>, // Base64 encoded audio
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AsrResponse {
    pub text: String,
    pub confidence: f32,
    pub words: Vec<WordTiming>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WordTiming {
    pub word: String,
    pub start_time: f64,
    pub end_time: f64,
    pub confidence: f32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TtsRequest {
    pub text: String,
    pub voice_type: String, // Voice ID from Doubao
    pub speed_ratio: f32,   // 0.5 - 2.0
    pub volume_ratio: f32,  // 0.0 - 3.0
    pub pitch_ratio: f32,   // 0.5 - 2.0
    pub audio_format: String, // "wav", "mp3", "pcm"
    pub sample_rate: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TtsResponse {
    pub audio_data: Vec<u8>,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String, // "system", "user", "assistant"
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub temperature: f32,
    pub max_tokens: u32,
    pub stream: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatResponse {
    pub content: String,
    pub finish_reason: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PronunciationAnalysis {
    pub overall_score: f32,
    pub fluency_score: f32,
    pub pronunciation_score: f32,
    pub completeness_score: f32,
    pub word_scores: Vec<WordPronunciationScore>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WordPronunciationScore {
    pub word: String,
    pub score: f32,
    pub error_type: Option<String>,
    pub suggestion: Option<String>,
}

impl DoubaoClient {
    /// Create a new Doubao client
    pub fn new(app_id: String, access_token: String, chat_api_key: String) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            app_id,
            access_token,
            chat_api_key,
        }
    }

    /// Perform speech recognition (ASR)
    pub async fn speech_to_text(&self, request: AsrRequest) -> Result<AsrResponse, Box<dyn Error>> {
        let url = format!("{}/asr", DOUBAO_API_BASE);
        
        let audio_base64 = base64::encode(&request.audio_data);
        
        let payload = json!({
            "app": {
                "appid": self.app_id,
                "token": self.access_token
            },
            "user": {
                "uid": "user_001"
            },
            "audio": {
                "format": request.audio_format,
                "rate": request.sample_rate,
                "language": request.language,
                "data": audio_base64
            }
        });

        let response = self.client
            .post(&url)
            .header(header::CONTENT_TYPE, "application/json")
            .json(&payload)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(format!("ASR API error: {}", error_text).into());
        }

        let result: serde_json::Value = response.json().await?;
        
        // Parse Doubao response format
        let text = result["result"]["text"]
            .as_str()
            .unwrap_or("")
            .to_string();
        
        let confidence = result["result"]["confidence"]
            .as_f64()
            .unwrap_or(0.0) as f32;

        let words = result["result"]["words"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|w| {
                        Some(WordTiming {
                            word: w["word"].as_str()?.to_string(),
                            start_time: w["start_time"].as_f64()?,
                            end_time: w["end_time"].as_f64()?,
                            confidence: w["confidence"].as_f64()? as f32,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(AsrResponse {
            text,
            confidence,
            words,
        })
    }

    /// Perform text-to-speech (TTS)
    pub async fn text_to_speech(&self, request: TtsRequest) -> Result<TtsResponse, Box<dyn Error>> {
        let url = format!("{}/tts", DOUBAO_API_BASE);
        
        let payload = json!({
            "app": {
                "appid": self.app_id,
                "token": self.access_token
            },
            "user": {
                "uid": "user_001"
            },
            "audio": {
                "voice_type": request.voice_type,
                "encoding": request.audio_format,
                "speed_ratio": request.speed_ratio,
                "volume_ratio": request.volume_ratio,
                "pitch_ratio": request.pitch_ratio,
                "sample_rate": request.sample_rate
            },
            "request": {
                "text": request.text,
                "operation": "submit"
            }
        });

        let response = self.client
            .post(&url)
            .header(header::CONTENT_TYPE, "application/json")
            .json(&payload)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(format!("TTS API error: {}", error_text).into());
        }

        let result: serde_json::Value = response.json().await?;
        
        let audio_base64 = result["data"]
            .as_str()
            .ok_or("Missing audio data")?;
        
        let audio_data = base64::decode(audio_base64)?;
        
        let duration_ms = result["duration"]
            .as_u64()
            .unwrap_or(0);

        Ok(TtsResponse {
            audio_data,
            duration_ms,
        })
    }

    /// Perform chat completion
    pub async fn chat_completion(&self, request: ChatRequest) -> Result<ChatResponse, Box<dyn Error>> {
        let url = format!("{}/chat/completions", DOUBAO_CHAT_API_BASE);
        
        let payload = json!({
            "model": request.model,
            "messages": request.messages,
            "temperature": request.temperature,
            "max_tokens": request.max_tokens,
            "stream": request.stream
        });

        let response = self.client
            .post(&url)
            .header(header::AUTHORIZATION, format!("Bearer {}", self.chat_api_key))
            .header(header::CONTENT_TYPE, "application/json")
            .json(&payload)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(format!("Chat API error: {}", error_text).into());
        }

        let result: serde_json::Value = response.json().await?;
        
        let content = result["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();
        
        let finish_reason = result["choices"][0]["finish_reason"]
            .as_str()
            .unwrap_or("stop")
            .to_string();

        Ok(ChatResponse {
            content,
            finish_reason,
        })
    }

    /// Streaming chat completion
    pub async fn chat_completion_stream(
        &self,
        request: ChatRequest,
    ) -> Result<impl StreamExt<Item = Result<String, Box<dyn Error>>>, Box<dyn Error>> {
        let url = format!("{}/chat/completions", DOUBAO_CHAT_API_BASE);
        
        let mut req = request;
        req.stream = true;
        
        let payload = json!({
            "model": req.model,
            "messages": req.messages,
            "temperature": req.temperature,
            "max_tokens": req.max_tokens,
            "stream": true
        });

        let response = self.client
            .post(&url)
            .header(header::AUTHORIZATION, format!("Bearer {}", self.chat_api_key))
            .header(header::CONTENT_TYPE, "application/json")
            .json(&payload)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(format!("Chat stream API error: {}", error_text).into());
        }

        let stream = response.bytes_stream().map(|result| {
            result
                .map_err(|e| Box::new(e) as Box<dyn Error>)
                .and_then(|bytes| {
                    let text = String::from_utf8(bytes.to_vec())
                        .map_err(|e| Box::new(e) as Box<dyn Error>)?;
                    
                    // Parse SSE format
                    if text.starts_with("data: ") {
                        let json_str = text.trim_start_matches("data: ");
                        if json_str == "[DONE]" {
                            return Ok(String::new());
                        }
                        
                        let value: serde_json::Value = serde_json::from_str(json_str)?;
                        let content = value["choices"][0]["delta"]["content"]
                            .as_str()
                            .unwrap_or("")
                            .to_string();
                        Ok(content)
                    } else {
                        Ok(String::new())
                    }
                })
        });

        Ok(stream)
    }

    /// Analyze pronunciation quality
    pub async fn analyze_pronunciation(
        &self,
        audio_data: Vec<u8>,
        reference_text: &str,
        language: &str,
    ) -> Result<PronunciationAnalysis, Box<dyn Error>> {
        let url = format!("{}/pronunciation_assessment", DOUBAO_API_BASE);
        
        let audio_base64 = base64::encode(&audio_data);
        
        let payload = json!({
            "app": {
                "appid": self.app_id,
                "token": self.access_token
            },
            "user": {
                "uid": "user_001"
            },
            "audio": {
                "data": audio_base64,
                "format": "wav",
                "language": language
            },
            "request": {
                "reference_text": reference_text,
                "score_coefficients": {
                    "fluency": 1.0,
                    "pronunciation": 1.0,
                    "completeness": 1.0
                }
            }
        });

        let response = self.client
            .post(&url)
            .header(header::CONTENT_TYPE, "application/json")
            .json(&payload)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(format!("Pronunciation API error: {}", error_text).into());
        }

        let result: serde_json::Value = response.json().await?;
        
        let overall_score = result["result"]["overall_score"]
            .as_f64()
            .unwrap_or(0.0) as f32;
        
        let fluency_score = result["result"]["fluency_score"]
            .as_f64()
            .unwrap_or(0.0) as f32;
        
        let pronunciation_score = result["result"]["pronunciation_score"]
            .as_f64()
            .unwrap_or(0.0) as f32;
        
        let completeness_score = result["result"]["completeness_score"]
            .as_f64()
            .unwrap_or(0.0) as f32;

        let word_scores = result["result"]["words"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|w| {
                        Some(WordPronunciationScore {
                            word: w["word"].as_str()?.to_string(),
                            score: w["score"].as_f64()? as f32,
                            error_type: w["error_type"].as_str().map(String::from),
                            suggestion: w["suggestion"].as_str().map(String::from),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(PronunciationAnalysis {
            overall_score,
            fluency_score,
            pronunciation_score,
            completeness_score,
            word_scores,
        })
    }

    /// Generate a conversation topic based on target words
    pub async fn generate_topic(
        &self,
        target_words: &[String],
        chat_history: &[ChatMessage],
    ) -> Result<String, Box<dyn Error>> {
        let mut messages = vec![
            ChatMessage {
                role: "system".to_string(),
                content: "You are a professional English teacher. Your task is to help users speak authentic English. You should primarily speak in English, and only switch to Chinese to explain when the user indicates they cannot understand what you're saying. Generate authentic English conversation topics that naturally incorporate the target vocabulary words. Topics should be relevant to current events, work scenarios, or daily life.".to_string(),
            }
        ];

        // Add recent chat history for context
        messages.extend_from_slice(&chat_history[chat_history.len().saturating_sub(5)..]);

        let words_str = if target_words.is_empty() {
            String::from("Generate a natural conversation topic without specific word requirements.")
        } else {
            format!(
                "Generate a conversation topic that naturally uses these words: {}. Make it engaging and relevant to real-life situations.",
                target_words.join(", ")
            )
        };

        messages.push(ChatMessage {
            role: "user".to_string(),
            content: words_str,
        });

        let request = ChatRequest {
            model: "doubao-seed-1-8-251228".to_string(),
            messages,
            temperature: 0.8,
            max_tokens: 500,
            stream: false,
        };

        let response = self.chat_completion(request).await?;
        Ok(response.content)
    }

    /// Analyze user text for issues (grammar, word choice, etc.)
    pub async fn analyze_user_text(
        &self,
        user_text: &str,
    ) -> Result<Vec<TextIssue>, Box<dyn Error>> {
        let messages = vec![
            ChatMessage {
                role: "system".to_string(),
                content: "You are an English language expert. Analyze the user's English text and identify issues including: grammar errors, word choice problems, better alternatives, and suggest improvements. Return your analysis in JSON format as an array of issues, each with: {\"type\": \"grammar|word_choice|suggestion\", \"original\": \"text\", \"suggested\": \"better text\", \"description\": \"explanation\", \"severity\": \"low|medium|high\"}".to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: format!("Analyze this text: \"{}\"", user_text),
            }
        ];

        let request = ChatRequest {
            model: "doubao-seed-1-8-251228".to_string(),
            messages,
            temperature: 0.3,
            max_tokens: 1000,
            stream: false,
        };

        let response = self.chat_completion(request).await?;
        
        // Try to parse JSON response
        match serde_json::from_str::<Vec<TextIssue>>(&response.content) {
            Ok(issues) => Ok(issues),
            Err(_) => {
                // If not valid JSON, try to extract from markdown code blocks
                let content = response.content
                    .trim_start_matches("```json")
                    .trim_start_matches("```")
                    .trim_end_matches("```")
                    .trim();
                
                serde_json::from_str(content).map_err(|e| {
                    format!("Failed to parse AI response: {}", e).into()
                })
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TextIssue {
    #[serde(rename = "type")]
    pub issue_type: String,
    pub original: String,
    pub suggested: String,
    pub description: String,
    pub severity: String,
}

// Base64 encoding/decoding utilities
mod base64 {
    use std::io::{Error, ErrorKind};

    pub fn encode(data: &[u8]) -> String {
        data.iter()
            .flat_map(|&b| {
                vec![
                    b >> 2,
                    ((b & 0x03) << 4),
                ]
            })
            .collect::<Vec<u8>>()
            .chunks(3)
            .map(|chunk| {
                let mut result = [b'='; 4];
                result[0] = ENCODE_TABLE[(chunk[0] >> 2) as usize];
                result[1] = ENCODE_TABLE[((chunk.get(0).unwrap_or(&0) & 0x03) << 4 | 
                                         chunk.get(1).unwrap_or(&0) >> 4) as usize];
                if chunk.len() > 1 {
                    result[2] = ENCODE_TABLE[((chunk[1] & 0x0f) << 2 | 
                                             chunk.get(2).unwrap_or(&0) >> 6) as usize];
                }
                if chunk.len() > 2 {
                    result[3] = ENCODE_TABLE[(chunk[2] & 0x3f) as usize];
                }
                String::from_utf8_lossy(&result).to_string()
            })
            .collect()
    }

    pub fn decode(_s: &str) -> Result<Vec<u8>, Error> {
        // Simplified base64 decode - in production use base64 crate
        Err(Error::new(ErrorKind::Other, "Use base64 crate for production"))
    }

    const ENCODE_TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
}
