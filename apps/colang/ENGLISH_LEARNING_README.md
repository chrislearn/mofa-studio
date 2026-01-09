# English Learning Companion - Architecture & Setup

## Overview

This is an AI-powered English learning application that uses **Doubao (豆包) Volcanic Engine APIs** for speech recognition, text-to-speech, and conversational AI. The system implements **spaced repetition learning** by tracking problematic words and systematically reintroducing them in conversations.

## Core Features

### 1. **Spaced Repetition Learning**
- Tracks words the user has difficulty with (pronunciation, usage, grammar)
- Selects 20-30 words for each session based on review schedule
- Limits word practice to 2-5 times per day to avoid overload
- Implements increasing intervals: 1 → 2 → 4 → 7 → 14 → 30 days

### 2. **Conversation Flow**
1. **Word Selection**: System selects words due for review from database
2. **Topic Generation**: AI generates a conversation topic incorporating target words
3. **Real-time Chat**: User speaks → ASR → AI responds → TTS → Audio output
4. **Issue Detection**: Analyzes user speech for pronunciation, grammar, vocabulary issues
5. **Database Storage**: All conversations and issues saved for future review

### 3. **AI-Powered Analysis**
- **Pronunciation**: Detects low-confidence words from ASR
- **Grammar**: Identifies grammatical errors
- **Vocabulary**: Finds word choice improvements
- **Fluency**: Tracks hesitations and pauses

## Architecture

```
┌─────────────────┐
│ Word Selector   │  Selects 20-30 words from database
└────────┬────────┘
         │
         v
┌─────────────────┐
│ Topic Generator │  Generates conversation topic
└────────┬────────┘
         │
         v
┌─────────────────┐     ┌──────────────┐
│ User Speaks     │────>│ Doubao ASR   │  Speech → Text
└─────────────────┘     └──────┬───────┘
                               │
                               v
                        ┌──────────────┐
                        │ AI Chat      │  Generates response
                        └──────┬───────┘
                               │
                               v
                        ┌──────────────┐
                        │ Doubao TTS   │  Text → Speech
                        └──────┬───────┘
                               │
                               v
                        ┌──────────────┐
                        │ Audio Output │
                        └──────────────┘

                        ┌──────────────┐
Parallel Analysis:      │ Conversation │  Analyzes issues
                        │ Analyzer     │  Saves to DB
                        └──────────────┘
```

## Database Schema

### Tables

1. **issue_words**: Stores problematic words with spaced repetition metadata
   - `word`, `issue_type`, `issue_description`
   - `last_picked_at`, `next_review_at`, `review_interval_days`
   - `difficulty_level`, `pick_count`

2. **conversations**: All conversation history
   - `session_id`, `speaker` (user/ai), `content_text`
   - `created_at`, `audio_path`, `duration_ms`
   - Performance metrics: `words_per_minute`, `pause_count`, `hesitation_count`

3. **conversation_annotations**: Issues found in conversations
   - `conversation_id`, `annotation_type`, `severity`
   - `original_text`, `suggested_text`, `description`

4. **learning_sessions**: Session metadata
   - `session_id`, `topic`, `target_words`
   - `started_at`, `ended_at`, `total_exchanges`

5. **word_practice_log**: Tracks each word practice event
   - `word_id`, `session_id`, `practiced_at`, `success_level`

## Setup Instructions

### Prerequisites

1. **Doubao (豆包) Volcanic Engine Account**
   - Sign up at [https://www.volcengine.com](https://www.volcengine.com)
   - Get API credentials:
     - `DOUBAO_APP_ID`
     - `DOUBAO_ACCESS_TOKEN`
     - `DOUBAO_API_KEY`

2. **Rust Toolchain**
   ```bash
   rustup update stable
   ```

3. **Dora Framework**
   ```bash
   cargo install dora-cli
   ```

### Environment Variables

Create a `.env` file in the `apps/colang` directory:

```bash
# Doubao API Credentials
DOUBAO_APP_ID=your_app_id_here
DOUBAO_ACCESS_TOKEN=your_access_token_here
DOUBAO_API_KEY=your_api_key_here

# Database
DATABASE_URL=sqlite:///d:/Works/chrislearn/mofa-studio/apps/colang/learning_companion.db

# Logging
LOG_LEVEL=INFO
```

### Build & Run

1. **Build all nodes**:
   ```bash
   cd apps/colang
   
   # Build word selector
   cargo build --release --manifest-path ../../rust-nodes/dora-word-selector/Cargo.toml
   
   # Build topic generator
   cargo build --release --manifest-path ../../rust-nodes/dora-topic-generator/Cargo.toml
   
   # Build Doubao ASR
   cargo build --release --manifest-path ../../rust-nodes/dora-doubao-asr/Cargo.toml
   
   # Build Doubao TTS
   cargo build --release --manifest-path ../../rust-nodes/dora-doubao-tts/Cargo.toml
   
   # Build conversation analyzer
   cargo build --release --manifest-path ../../rust-nodes/dora-conversation-analyzer/Cargo.toml
   ```

2. **Initialize database**:
   ```bash
   # Database will be created automatically on first run
   # Migrations are in apps/colang/migrations/
   ```

3. **Run dataflow**:
   ```bash
   cd apps/colang/dataflow
   dora start english-learning.yml
   ```

## Configuration

### AI Teacher Personality

Edit `dataflow/english_teacher_config.toml` to customize:
- Teaching style and tone
- Correction timing and threshold
- Conversation topics
- Session parameters

### Voice Settings

In the dataflow YAML, adjust:
- `VOICE_TYPE`: Choose Doubao voice (e.g., `BV700_V2_streaming` for US English)
- `SPEED_RATIO`: Speech speed (0.5 - 2.0)
- `LANGUAGE`: Recognition language (`en` for English)

### Word Selection

Environment variables:
- `MIN_WORDS`: Minimum words per session (default: 20)
- `MAX_WORDS`: Maximum words per session (default: 30)
- Daily limit is hardcoded to 5 practices per word

## Doubao API Integration

### Speech Recognition (ASR)
- **Endpoint**: `https://openspeech.bytedance.com/api/v1/asr`
- **Features**: Word-level timing, confidence scores
- **Supported formats**: WAV, MP3, PCM
- **Languages**: English, Chinese, Auto-detect

### Text-to-Speech (TTS)
- **Endpoint**: `https://openspeech.bytedance.com/api/v1/tts`
- **Voices**: Multiple English voices (US, UK, Australian)
- **Controls**: Speed, pitch, volume
- **Output**: 24kHz WAV audio

### Chat Completion
- **Endpoint**: `https://ark.cn-beijing.volces.com/api/v3/chat/completions`
- **Model**: `doubao-pro-32k` (32k context window)
- **Features**: Streaming responses, JSON mode
- **Temperature**: Configurable (0.7 default)

## Development Notes

### Adding New Issue Types

1. Update `IssueType` enum in `src/database.rs`
2. Add to CHECK constraint in `migrations/001_create_tables.sql`
3. Update analyzer logic in `dora-conversation-analyzer`

### Customizing Spaced Repetition

Modify `update_word_after_practice()` in `src/database.rs`:
- Change interval progression (currently: 1→2→4→7→14→30 days)
- Adjust difficulty scaling
- Modify success/failure criteria

### Extending Analysis

Add new analysis types in `dora-conversation-analyzer`:
- Fluency metrics (WPM, pauses)
- Discourse markers
- Sentence complexity
- Topic coherence

## Troubleshooting

### Database Issues
```bash
# Reset database
rm learning_companion.db
# Will be recreated on next run
```

### API Errors
- Check API credentials in `.env`
- Verify API quota/limits
- Check network connectivity to Volcanic Engine

### Audio Problems
- Ensure audio format matches ASR expectations (24kHz recommended)
- Check microphone permissions
- Verify TTS output format compatibility

## Future Enhancements

- [ ] Real-time pronunciation feedback
- [ ] Grammar rule explanations
- [ ] Progress dashboard
- [ ] Custom vocabulary lists
- [ ] Multiple difficulty levels
- [ ] Conversation topics customization
- [ ] Export learning reports

## References

- **Doubao Documentation**: [https://www.volcengine.com/docs](https://www.volcengine.com/docs)
- **Dora Framework**: [https://dora.carsmos.ai](https://dora.carsmos.ai)
- **Spaced Repetition**: [https://en.wikipedia.org/wiki/Spaced_repetition](https://en.wikipedia.org/wiki/Spaced_repetition)
