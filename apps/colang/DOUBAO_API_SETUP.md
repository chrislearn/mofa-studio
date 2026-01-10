# Doubao API Setup Guide

## Required Environment Variables

To use the Doubao (Volcanic Engine) API for ASR (speech recognition) and TTS (text-to-speech), you need to configure the following environment variables:

### 1. DOUBAO_APP_ID
Your application ID (appid) from the Volcengine console. Also referred to as `X-Api-App-Key` in WebSocket API.

### 2. DOUBAO_ACCESS_TOKEN
Your access token for authentication. Also referred to as `X-Api-Access-Key` in WebSocket API.

### 3. DOUBAO_RESOURCE_ID (for TTS WebSocket API)
The resource information ID for TTS service. Available options:
- **豆包语音合成模型1.0**:
  - `seed-tts-1.0` or `volc.service_type.10029` (字符版)
  - `seed-tts-1.0-concurr` or `volc.service_type.10048` (并发版)
- **豆包语音合成模型2.0**:
  - `seed-tts-2.0` (字符版) - **推荐默认值**
- **声音复刻**:
  - `seed-icl-1.0` (声音复刻1.0字符版)
  - `seed-icl-1.0-concurr` (声音复刻1.0并发版)
  - `seed-icl-2.0` (声音复刻2.0字符版)

### 4. DOUBAO_CLUSTER (for ASR HTTP API)
The business cluster identifier for ASR service. Use `volcano_asr` or the appropriate cluster for your service.

### 5. DOUBAO_API_KEY (for LLM services)
Your API key for Doubao AI model services (used in english-teacher node).

### 6. VOICE_TYPE
The voice/speaker ID to use. Examples:
- `zh_female_cancan_mars_bigtts` - 中文女声 (默认)
- `zh_male_qingxin_mars_bigtts` - 中文男声
- See [大模型音色列表](https://www.volcengine.com/docs/6561/1257544) for more options

## How to Get These Values

According to the [Volcengine Console FAQ](https://www.volcengine.com/docs/6561/196768#q1):
1. Log in to the Volcengine console
2. Navigate to your Doubao Voice/Speech service
3. Find your application settings
4. Copy the `appid`, `token`, and `cluster`/`resource_id` values

## API Versions

### TTS (Text-to-Speech)
**Current Implementation**: WebSocket Bidirectional Streaming API (双向流式 WebSocket)
- **URL**: `wss://openspeech.bytedance.com/api/v3/tts/bidirection`
- **Authentication**: Via WebSocket headers (`X-Api-App-Key`, `X-Api-Access-Key`, `X-Api-Resource-Id`)
- **Protocol**: Binary WebSocket protocol with events
- **Documentation**: https://www.volcengine.com/docs/6561/1329505

**Benefits of WebSocket API**:
- Better latency for streaming responses
- Supports connection reuse (multiple sessions per connection)
- Handles sentence segmentation automatically
- More suitable for real-time conversation scenarios

### ASR (Speech Recognition)
**Current Implementation**: HTTP API
- **URL**: `https://openspeech.bytedance.com/api/v1/asr`
- **Authentication**: Via request body (`app.cluster` required)
- **Documentation**: https://www.volcengine.com/docs/6561/79820

## Setting Environment Variables

### Windows (PowerShell)
```powershell
$env:DOUBAO_APP_ID="your_app_id"
$env:DOUBAO_ACCESS_TOKEN="your_access_token"
$env:DOUBAO_RESOURCE_ID="seed-tts-2.0"  # For TTS WebSocket
$env:DOUBAO_CLUSTER="volcano_asr"       # For ASR HTTP
$env:DOUBAO_API_KEY="your_api_key"
$env:VOICE_TYPE="zh_female_cancan_mars_bigtts"
```

### Linux/macOS (Bash)
```bash
export DOUBAO_APP_ID="your_app_id"
export DOUBAO_ACCESS_TOKEN="your_access_token"
export DOUBAO_RESOURCE_ID="seed-tts-2.0"
export DOUBAO_CLUSTER="volcano_asr"
export DOUBAO_API_KEY="your_api_key"
export VOICE_TYPE="zh_female_cancan_mars_bigtts"
```

### In .env file (recommended for development)
Create a `.env` file in the project root:
```env
DOUBAO_APP_ID=your_app_id
DOUBAO_ACCESS_TOKEN=your_access_token
DOUBAO_RESOURCE_ID=seed-tts-2.0
DOUBAO_CLUSTER=volcano_asr
DOUBAO_API_KEY=your_api_key
VOICE_TYPE=zh_female_cancan_mars_bigtts
```

## API Documentation References

### TTS (Text-to-Speech)
- **双向流式 WebSocket API**: https://www.volcengine.com/docs/6561/1329505 **(当前使用)**
- 单向流式 WebSocket API: https://www.volcengine.com/docs/6561/1719100
- HTTP 接口 (非流式): https://www.volcengine.com/docs/6561/79820
- 大模型音色列表: https://www.volcengine.com/docs/6561/1257544

### ASR (Speech Recognition)
- HTTP 接口: https://www.volcengine.com/docs/6561/79820
- 参数说明: https://www.volcengine.com/docs/6561/79823

### General
- Console FAQ: https://www.volcengine.com/docs/6561/196768

## Common Errors

### TTS WebSocket Errors

#### Error: Connection refused or timeout
Check that:
1. `DOUBAO_APP_ID` is correct
2. `DOUBAO_ACCESS_TOKEN` is correct and valid
3. Your network allows WebSocket connections to `openspeech.bytedance.com`

#### Error: Authentication failed
Verify that:
1. Your access token is valid and not expired
2. The app_id matches your console configuration
3. You have proper permissions for the TTS service

#### Error: Invalid resource_id
Make sure `DOUBAO_RESOURCE_ID` is one of the valid values listed above and that you have subscribed to that service tier.

### ASR HTTP Errors

#### Error: "Missing required: app.cluster"
This error occurs when the `DOUBAO_CLUSTER` environment variable is not set for ASR. Make sure to set it to `volcano_asr` or your specific cluster value.

#### Error: "authenticate request: load grant: requested grant not found"
This indicates authentication failure. Check that:
1. `DOUBAO_APP_ID` is correct
2. `DOUBAO_ACCESS_TOKEN` is correct and valid

## Current Configuration in learning.yml

The dataflow configuration uses these environment variables with sensible defaults:

```yaml
# For ASR (doubao-asr node) - HTTP API
env:
  DOUBAO_APP_ID: ${DOUBAO_APP_ID:-}
  DOUBAO_ACCESS_TOKEN: ${DOUBAO_ACCESS_TOKEN:-}
  DOUBAO_CLUSTER: ${DOUBAO_CLUSTER:-volcano_asr}

# For TTS (doubao-tts node) - WebSocket API
env:
  DOUBAO_APP_ID: ${DOUBAO_APP_ID:-}
  DOUBAO_ACCESS_TOKEN: ${DOUBAO_ACCESS_TOKEN:-}
  DOUBAO_RESOURCE_ID: ${DOUBAO_RESOURCE_ID:-seed-tts-2.0}
  VOICE_TYPE: ${VOICE_TYPE:-zh_female_cancan_mars_bigtts}
```

## Migration Notes

The TTS implementation has been migrated from HTTP API to WebSocket API for better performance:

**Old (HTTP API)**:
- Required: `DOUBAO_CLUSTER` (e.g., `volcano_tts`)
- Single request-response per connection
- Base64-encoded audio in JSON response

**New (WebSocket API)**:
- Required: `DOUBAO_RESOURCE_ID` (e.g., `seed-tts-2.0`)
- Connection reuse with multiple sessions
- Binary streaming audio frames
- Better latency and throughput

If you need to revert to the HTTP API, check git history for the previous implementation.
