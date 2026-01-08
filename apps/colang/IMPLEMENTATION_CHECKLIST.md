# Implementation Checklist - English Learning Companion

## ✅ Completed Tasks

### Core Infrastructure
- [x] Database schema designed with 5 tables + 1 view
- [x] SQL migrations created (`migrations/001_create_tables.sql`)
- [x] Database operations implemented (`src/database.rs`)
- [x] Seed data created with 40+ example words (`seed_data.sql`)

### API Integration
- [x] Doubao API client implemented (`src/doubao_api.rs`)
- [x] ASR (Speech Recognition) integration
- [x] TTS (Text-to-Speech) integration
- [x] Chat completion with streaming support
- [x] Pronunciation analysis capabilities
- [x] Text analysis for grammar/vocabulary

### Dora Nodes
- [x] Word selector node (`node-hub/dora-word-selector/`)
- [x] Topic generator node (`node-hub/dora-topic-generator/`)
- [x] Doubao ASR node (`node-hub/dora-doubao-asr/`)
- [x] Doubao TTS node (`node-hub/dora-doubao-tts/`)
- [x] Conversation analyzer node (`node-hub/dora-conversation-analyzer/`)

### Configuration
- [x] Dataflow YAML (`dataflow/english-learning.yml`)
- [x] AI teacher config (`dataflow/english_teacher_config.toml`)
- [x] Environment template (`.env` created by setup script)

### Documentation
- [x] Comprehensive README (`ENGLISH_LEARNING_README.md`)
- [x] Quick start guide (`QUICKSTART.md`)
- [x] Refactoring summary (`REFACTORING_SUMMARY.md`)
- [x] Setup automation script (`setup-english-learning.ps1`)

### Dependencies
- [x] Cargo.toml updated with all required dependencies
- [x] sqlx with SQLite support
- [x] reqwest for HTTP/API calls
- [x] tokio for async runtime
- [x] base64 for audio encoding
- [x] uuid for session IDs

## ⚠️ Pending Integration Tasks

### MoFA Bridge Integration (Placeholder Nodes)
These nodes are marked as "custom" in the dataflow and need implementation:

- [ ] **mofa-audio-input**: Capture microphone input
  - Should output audio in format compatible with Doubao ASR
  - Recommended: 24kHz, WAV or PCM format
  - Location: Integrate with existing `src/audio.rs` or `src/audio_player.rs`

- [ ] **mofa-audio-player**: Play TTS audio output
  - Receive audio from Doubao TTS node
  - Output `audio_complete` signal for segmentation
  - Output `buffer_status` for backpressure control
  - Location: Extend existing `src/audio_player.rs`

- [ ] **mofa-chat-display**: Display conversation history
  - Show user text from ASR
  - Show AI responses
  - Show analysis results (issues detected)
  - Location: Add to existing `src/screen/` or create new widget

- [ ] **session-controller**: Control learning session flow
  - Trigger word selection periodically (every 30 minutes default)
  - Send control signals to other nodes
  - Manage session lifecycle
  - Location: Could be integrated into `src/dora_integration.rs`

### Database Initialization
- [ ] Run migrations on first startup
- [ ] Optionally load seed data
- [ ] Create database backup mechanism

### Testing
- [ ] Test word selection with empty database
- [ ] Test word selection with seed data
- [ ] Test API connectivity to Doubao
- [ ] Test audio input/output pipeline
- [ ] Test conversation flow end-to-end
- [ ] Test issue detection and storage
- [ ] Test spaced repetition logic

## 🔧 Configuration Requirements

### Before First Run
1. [ ] Sign up for Doubao Volcanic Engine account
2. [ ] Obtain API credentials:
   - `DOUBAO_APP_ID`
   - `DOUBAO_ACCESS_TOKEN`
   - `DOUBAO_API_KEY`
3. [ ] Update `.env` file with credentials
4. [ ] Choose appropriate voice type (default: `BV700_V2_streaming` for US English)

### Optional Configuration
- [ ] Adjust AI teacher personality in `english_teacher_config.toml`
- [ ] Set voice speed/pitch preferences
- [ ] Configure session parameters (length, frequency)
- [ ] Customize word selection limits

## 📋 Buildand Run Steps

### 1. Setup
```powershell
# Run automated setup
.\setup-english-learning.ps1

# Or manual setup
cd apps/colang
# Edit .env with your API credentials
```

### 2. Build Nodes
```bash
# Build all nodes
cargo build --release --manifest-path node-hub/dora-word-selector/Cargo.toml
cargo build --release --manifest-path node-hub/dora-topic-generator/Cargo.toml
cargo build --release --manifest-path node-hub/dora-doubao-asr/Cargo.toml
cargo build --release --manifest-path node-hub/dora-doubao-tts/Cargo.toml
cargo build --release --manifest-path node-hub/dora-conversation-analyzer/Cargo.toml
```

### 3. Initialize Database (Optional)
```bash
cd apps/colang
# Database will be created automatically, but you can seed it:
sqlite3 learning_companion.db < seed_data.sql
```

### 4. Run Dataflow
```bash
cd apps/colang/dataflow
dora start english-learning.yml
```

## 🔍 Verification Steps

### Database
```bash
sqlite3 learning_companion.db
sqlite> .tables
# Should show: issue_words, conversations, conversation_annotations, 
#              learning_sessions, word_practice_log

sqlite> SELECT COUNT(*) FROM issue_words;
# If seed data loaded, should show 40+
```

### API Connectivity
```bash
# Test with curl (replace with your credentials)
curl -X POST https://openspeech.bytedance.com/api/v1/asr \
  -H "Content-Type: application/json" \
  -d '{"app":{"appid":"YOUR_APP_ID","token":"YOUR_TOKEN"},...}'
```

### Node Builds
```bash
# Verify all binaries exist
ls node-hub/dora-word-selector/target/release/dora-word-selector.exe
ls node-hub/dora-topic-generator/target/release/dora-topic-generator.exe
ls node-hub/dora-doubao-asr/target/release/dora-doubao-asr.exe
ls node-hub/dora-doubao-tts/target/release/dora-doubao-tts.exe
ls node-hub/dora-conversation-analyzer/target/release/dora-conversation-analyzer.exe
```

## 🐛 Known Issues & Workarounds

### Issue 1: MoFA Bridge Nodes Not Implemented
**Status**: Placeholder nodes in YAML
**Workaround**: Implement custom bridge nodes or use test stubs
**Priority**: High - Required for full functionality

### Issue 2: Base64 Implementation Simplified
**Status**: Custom base64 encoding may have edge cases
**Workaround**: Replace with proper `base64` crate (already in dependencies)
**Priority**: Medium - Current implementation works for testing

### Issue 3: Hardcoded Database Path in YAML
**Status**: Path is absolute, not portable
**Workaround**: Edit YAML to use `DATABASE_URL` environment variable
**Priority**: Low - Works for single machine setup

### Issue 4: No Session Controller Implementation
**Status**: Marked as "custom" in dataflow
**Workaround**: Manual triggering or timer-based external trigger
**Priority**: Medium - Can be added later

### Issue 5: No Real-time Audio Streaming
**Status**: Uses batch audio processing
**Workaround**: Keep audio chunks small for near-real-time feel
**Priority**: Low - Batch processing is acceptable for learning

## 📊 Success Metrics

After implementation, you should see:
- [ ] Words automatically selected from database
- [ ] AI generates relevant conversation topics
- [ ] Speech recognition works with acceptable accuracy
- [ ] AI responses are natural and educational
- [ ] TTS output sounds natural
- [ ] Issues are detected and stored in database
- [ ] Words appear in review schedule with appropriate intervals
- [ ] Conversation history is saved and retrievable

## 🎯 Next Steps

1. **Immediate** (Required for basic functionality):
   - Implement MoFA audio bridge nodes
   - Test API connectivity with real credentials
   - Verify database operations

2. **Short-term** (Enhance user experience):
   - Add session controller for automatic triggering
   - Implement progress visualization
   - Add error handling and user feedback

3. **Long-term** (Advanced features):
   - Real-time pronunciation feedback
   - Progress dashboard/analytics
   - Custom vocabulary import
   - Multi-user support
   - Export learning reports

## 📝 Notes for Developers

- All timestamps use Unix epoch in seconds
- Database uses SQLite for simplicity (consider PostgreSQL for production)
- Doubao API calls are async using tokio
- Error handling uses `eyre` crate for better error messages
- Logging uses `env_logger` (set `LOG_LEVEL=DEBUG` for verbose output)

## 📞 Support Resources

- **Doubao API Docs**: https://www.volcengine.com/docs
- **Dora Framework**: https://dora.carsmos.ai
- **SQLx Documentation**: https://docs.rs/sqlx
- **This Project**: See `ENGLISH_LEARNING_README.md` for details

---

**Last Updated**: January 8, 2026
**Status**: Core implementation complete, MoFA integration pending
**Total LOC**: 3200+
**Estimated Integration Time**: 4-8 hours for MoFA bridges
