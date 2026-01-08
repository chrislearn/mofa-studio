# Quick Start Guide - English Learning Companion

## Quick Setup (5 minutes)

### 1. Get Doubao API Credentials

Visit [Volcanic Engine](https://www.volcengine.com) and sign up for an account. You'll need:
- `DOUBAO_APP_ID`
- `DOUBAO_ACCESS_TOKEN` 
- `DOUBAO_API_KEY`

### 2. Configure Environment

Edit `apps/colang/.env`:
```bash
DOUBAO_APP_ID=your_actual_app_id
DOUBAO_ACCESS_TOKEN=your_actual_token
DOUBAO_API_KEY=your_actual_api_key
```

### 3. Build & Run

**Windows PowerShell:**
```powershell
.\setup-english-learning.ps1
```

**Manual build:**
```bash
cd apps/colang/dataflow
dora start english-learning.yml
```

## Usage

### Starting a Session

1. The system automatically selects 20-30 words for review
2. AI generates a conversation topic using those words
3. Start speaking in English
4. AI responds and continues the conversation

### During Conversation

- **Speak naturally** - the system records and transcribes your speech
- **AI responds** in English with natural pronunciation
- **Issues are detected** automatically:
  - Pronunciation problems (low confidence words)
  - Grammar errors
  - Better word choices
  - Vocabulary gaps

### After Session

All data is saved to database:
- Conversation history
- Detected issues
- Words needing practice
- Next review schedule

## Example Conversation Flow

```
System: Selecting words for review...
        Words: confident, persuade, accomplish, determine, approach...
        Topic: "Discussing career goals and professional development"

AI: Hi! Today I'd like to talk about your career goals. 
    What are you hoping to accomplish in your current role?

User: [Speaks] I want to improve my skills and maybe get promotion.

System: [Analyzing...]
        - Detected: Missing article "a" before "promotion"
        - Suggestion: "get a promotion"
        - Added "improve" to practice list (usage)

AI: That's great! What specific skills do you feel most confident about 
    right now? And which ones would you like to approach differently?

User: [Continues conversation...]
```

## Database Query Examples

Check your progress:

```sql
-- View all problematic words
SELECT word, issue_type, difficulty_level, pick_count 
FROM issue_words 
ORDER BY difficulty_level DESC;

-- Recent conversations
SELECT speaker, content_text, created_at 
FROM conversations 
WHERE session_id = 'latest_session_id'
ORDER BY created_at;

-- Words due for review today
SELECT * FROM words_due_for_review;

-- Learning statistics
SELECT 
    COUNT(*) as total_sessions,
    AVG(total_exchanges) as avg_exchanges
FROM learning_sessions;
```

## Troubleshooting

### "Database migration failed"
- Delete `learning_companion.db` and restart

### "ASR API error"
- Check `DOUBAO_APP_ID` and `DOUBAO_ACCESS_TOKEN`
- Verify API quota hasn't been exceeded

### "No audio output"
- Check speaker connection
- Verify `VOICE_TYPE` setting
- Check TTS API credentials

### "No words selected"
- Database is empty - words are added as you practice
- First session will have no target words (still works!)

## Tips for Effective Learning

1. **Practice regularly** - Daily 15-30 minute sessions
2. **Speak naturally** - Don't worry about mistakes
3. **Review patterns** - Check database to see your common issues
4. **Vary topics** - Edit `english_teacher_config.toml` for different scenarios
5. **Be patient** - Spaced repetition takes time but is very effective

## Configuration Tips

### Make AI more/less strict

Edit `english_teacher_config.toml`:
```toml
[feedback]
correction_threshold = "low"  # low, medium, high
```

### Change voice speed

Edit `dataflow/english-learning.yml`:
```yaml
env:
  SPEED_RATIO: 1.2  # Faster speech
```

### Adjust session length

```toml
[conversation]
min_exchanges = 10  # Longer sessions
max_exchanges = 50
```

## What's Next?

- Practice daily for best results
- Check `ENGLISH_LEARNING_README.md` for detailed documentation
- Monitor your progress in the database
- Customize the AI teacher personality to your preferences

Happy learning! 🎓
