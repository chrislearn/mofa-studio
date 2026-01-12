// Database models and operations for English Learning Companion

use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqlitePool, FromRow, Row};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct IssueWord {
    pub id: Option<i64>,
    pub word: String,
    pub issue_type: IssueType,
    pub description_en: Option<String>,
    pub description_zh: Option<String>,
    pub last_picked_at: Option<i64>,
    pub created_at: i64,
    pub pick_count: i64,
    pub next_review_at: Option<i64>,
    pub review_interval_days: i64,
    pub difficulty_level: i64,
    pub context: Option<String>,
    pub audio_timestamp: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum IssueType {
    Pronunciation,
    Usage,
    Unfamiliar,
    Grammar,
}

impl ToString for IssueType {
    fn to_string(&self) -> String {
        match self {
            IssueType::Pronunciation => "pronunciation".to_string(),
            IssueType::Usage => "usage".to_string(),
            IssueType::Unfamiliar => "unfamiliar".to_string(),
            IssueType::Grammar => "grammar".to_string(),
        }
    }
}

impl std::str::FromStr for IssueType {
    type Err = String;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pronunciation" => Ok(IssueType::Pronunciation),
            "usage" => Ok(IssueType::Usage),
            "unfamiliar" => Ok(IssueType::Unfamiliar),
            "grammar" => Ok(IssueType::Grammar),
            _ => Err(format!("Invalid issue type: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Conversation {
    pub id: Option<i64>,
    pub session_id: String,
    pub speaker: Speaker,
    pub use_lang: UseLang,
    pub content_en: String,
    pub content_zh: String,
    pub audio_path: Option<String>,
    pub created_at: i64,
    pub duration_ms: Option<i64>,
    pub words_per_minute: Option<f64>,
    pub pause_count: Option<i64>,
    pub hesitation_count: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Speaker {
    User,
    Teacher,
}

impl ToString for Speaker {
    fn to_string(&self) -> String {
        match self {
            Speaker::User => "user".to_string(),
            Speaker::Teacher => "teacher".to_string(),
        }
    }
}

impl std::str::FromStr for Speaker {
    type Err = String;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "user" => Ok(Speaker::User),
            "teacher" => Ok(Speaker::Teacher),
            _ => Err(format!("Invalid speaker: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum UseLang {
    En,
    Zh,
    Mix,
}

impl ToString for UseLang {
    fn to_string(&self) -> String {
        match self {
            UseLang::En => "en".to_string(),
            UseLang::Zh => "zh".to_string(),
            UseLang::Mix => "mix".to_string(),
        }
    }
}

impl std::str::FromStr for UseLang {
    type Err = String;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "en" => Ok(UseLang::En),
            "zh" => Ok(UseLang::Zh),
            "mix" => Ok(UseLang::Mix),
            _ => Err(format!("Invalid use_lang: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ConversationAnnotation {
    pub id: Option<i64>,
    pub conversation_id: i64,
    pub annotation_type: AnnotationType,
    pub start_position: Option<i64>,
    pub end_position: Option<i64>,
    pub original_text: Option<String>,
    pub suggested_text: Option<String>,
    pub description_en: Option<String>,
    pub description_zh: Option<String>,
    pub severity: Severity,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AnnotationType {
    PronunciationError,
    GrammarError,
    WordChoice,
    FluencyIssue,
    Suggestion,
    Correction,
}

impl ToString for AnnotationType {
    fn to_string(&self) -> String {
        match self {
            AnnotationType::PronunciationError => "pronunciation_error".to_string(),
            AnnotationType::GrammarError => "grammar_error".to_string(),
            AnnotationType::WordChoice => "word_choice".to_string(),
            AnnotationType::FluencyIssue => "fluency_issue".to_string(),
            AnnotationType::Suggestion => "suggestion".to_string(),
            AnnotationType::Correction => "correction".to_string(),
        }
    }
}

impl std::str::FromStr for AnnotationType {
    type Err = String;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pronunciation_error" => Ok(AnnotationType::PronunciationError),
            "grammar_error" => Ok(AnnotationType::GrammarError),
            "word_choice" => Ok(AnnotationType::WordChoice),
            "fluency_issue" => Ok(AnnotationType::FluencyIssue),
            "suggestion" => Ok(AnnotationType::Suggestion),
            "correction" => Ok(AnnotationType::Correction),
            _ => Err(format!("Invalid annotation type: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Low,
    Medium,
    High,
}

impl ToString for Severity {
    fn to_string(&self) -> String {
        match self {
            Severity::Low => "low".to_string(),
            Severity::Medium => "medium".to_string(),
            Severity::High => "high".to_string(),
        }
    }
}

impl std::str::FromStr for Severity {
    type Err = String;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "low" => Ok(Severity::Low),
            "medium" => Ok(Severity::Medium),
            "high" => Ok(Severity::High),
            _ => Err(format!("Invalid severity: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LearningSession {
    pub id: Option<i64>,
    pub session_id: String,
    pub topic: String,
    pub target_words: Option<String>, // JSON array
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub total_exchanges: i64,
    pub user_satisfaction: Option<i64>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct WordPracticeLog {
    pub id: Option<i64>,
    pub word_id: i64,
    pub session_id: String,
    pub practiced_at: i64,
    pub success_level: Option<i64>,
    pub notes: Option<String>,
}

/// Database manager for English Learning Companion
pub struct Database {
    pool: SqlitePool,
}

impl Database {
    /// Create a new database connection pool
    pub async fn new(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = SqlitePool::connect(database_url).await?;
        Ok(Self { pool })
    }

    /// Run migrations
    pub async fn migrate(&self) -> Result<(), sqlx::Error> {
        sqlx::migrate!("./migrations").run(&self.pool).await?;
        Ok(())
    }

    /// Get current Unix timestamp
    fn now() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
    }

    // ============ IssueWord Operations ============

    /// Insert a new issue word
    pub async fn insert_issue_word(&self, word: &IssueWord) -> Result<i64, sqlx::Error> {
        let result = sqlx::query(
            r#"
            INSERT INTO issue_words (
                word, issue_type, description_en, description_zh, created_at, pick_count,
                review_interval_days, difficulty_level, context, audio_timestamp
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(word, issue_type) DO UPDATE SET
                description_en = excluded.description_en,
                description_zh = excluded.description_zh,
                context = excluded.context,
                audio_timestamp = excluded.audio_timestamp
            "#
        )
        .bind(&word.word)
        .bind(word.issue_type.to_string())
        .bind(&word.description_en)
        .bind(&word.description_zh)
        .bind(word.created_at)
        .bind(word.pick_count)
        .bind(word.review_interval_days)
        .bind(word.difficulty_level)
        .bind(&word.context)
        .bind(word.audio_timestamp)
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    /// Get words due for review (max 30, respecting daily frequency limit)
    pub async fn get_words_for_review(&self, limit: i64) -> Result<Vec<IssueWord>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT 
                w.id, w.word, w.issue_type, w.description_en, w.description_zh, w.last_picked_at,
                w.created_at, w.pick_count, w.next_review_at, w.review_interval_days,
                w.difficulty_level, w.context, w.audio_timestamp,
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
        .bind(Self::now() - 86400) // 24 hours ago
        .bind(Self::now())
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let mut words = Vec::new();
        for row in rows {
            words.push(IssueWord {
                id: row.get("id"),
                word: row.get("word"),
                issue_type: row.get::<String, _>("issue_type").parse().unwrap(),
                description_en: row.get("description_en"),
                description_zh: row.get("description_zh"),
                last_picked_at: row.get("last_picked_at"),
                created_at: row.get("created_at"),
                pick_count: row.get("pick_count"),
                next_review_at: row.get("next_review_at"),
                review_interval_days: row.get("review_interval_days"),
                difficulty_level: row.get("difficulty_level"),
                context: row.get("context"),
                audio_timestamp: row.get("audio_timestamp"),
            });
        }

        Ok(words)
    }

    /// Update word after practice (implements spaced repetition)
    pub async fn update_word_after_practice(
        &self,
        word_id: i64,
        success: bool,
    ) -> Result<(), sqlx::Error> {
        let (new_interval, new_difficulty) = if success {
            // Increase interval on success (1 -> 2 -> 4 -> 7 -> 14 -> 30 days)
            let row = sqlx::query(
                "SELECT review_interval_days, difficulty_level FROM issue_words WHERE id = ?"
            )
            .bind(word_id)
            .fetch_one(&self.pool)
            .await?;
            
            let interval: i64 = row.get("review_interval_days");
            let diff: i64 = row.get("difficulty_level");
            
            let new_interval = match interval {
                1 => 2,
                2 => 4,
                4 => 7,
                7 => 14,
                14 => 30,
                _ => 30,
            };
            let new_diff = (diff - 1).max(1);
            (new_interval, new_diff)
        } else {
            // Reset interval on failure, increase difficulty
            let diff = sqlx::query_scalar::<_, i64>(
                "SELECT difficulty_level FROM issue_words WHERE id = ?"
            )
            .bind(word_id)
            .fetch_one(&self.pool)
            .await?;
            
            (1, (diff + 1).min(5))
        };

        let next_review = Self::now() + (new_interval * 86400);

        sqlx::query(
            r#"
            UPDATE issue_words 
            SET last_picked_at = ?,
                pick_count = pick_count + 1,
                next_review_at = ?,
                review_interval_days = ?,
                difficulty_level = ?
            WHERE id = ?
            "#
        )
        .bind(Self::now())
        .bind(next_review)
        .bind(new_interval)
        .bind(new_difficulty)
        .bind(word_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    // ============ Conversation Operations ============

    /// Insert a new conversation entry
    pub async fn insert_conversation(&self, conv: &Conversation) -> Result<i64, sqlx::Error> {
        let result = sqlx::query(
            r#"
            INSERT INTO conversations (
                session_id, speaker, use_lang, content_en, content_zh, audio_path, created_at,
                duration_ms, words_per_minute, pause_count, hesitation_count
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(&conv.session_id)
        .bind(conv.speaker.to_string())
        .bind(conv.use_lang.to_string())
        .bind(&conv.content_en)
        .bind(&conv.content_zh)
        .bind(&conv.audio_path)
        .bind(conv.created_at)
        .bind(conv.duration_ms)
        .bind(conv.words_per_minute)
        .bind(conv.pause_count)
        .bind(conv.hesitation_count)
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    /// Get conversation history for a session
    pub async fn get_conversation_history(
        &self,
        session_id: &str,
        limit: i64,
    ) -> Result<Vec<Conversation>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT * FROM conversations 
            WHERE session_id = ?
            ORDER BY created_at DESC
            LIMIT ?
            "#
        )
        .bind(session_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let mut conversations = Vec::new();
        for row in rows {
            conversations.push(Conversation {
                id: row.get("id"),
                session_id: row.get("session_id"),
                speaker: row.get::<String, _>("speaker").parse().unwrap(),
                use_lang: row.get::<String, _>("use_lang").parse().unwrap(),
                content_en: row.get("content_en"),
                content_zh: row.get("content_zh"),
                audio_path: row.get("audio_path"),
                created_at: row.get("created_at"),
                duration_ms: row.get("duration_ms"),
                words_per_minute: row.get("words_per_minute"),
                pause_count: row.get("pause_count"),
                hesitation_count: row.get("hesitation_count"),
            });
        }

        Ok(conversations)
    }

    // ============ Annotation Operations ============

    /// Insert a conversation annotation
    pub async fn insert_annotation(
        &self,
        annotation: &ConversationAnnotation,
    ) -> Result<i64, sqlx::Error> {
        let result = sqlx::query(
            r#"
            INSERT INTO conversation_annotations (
                conversation_id, annotation_type, start_position, end_position,
                original_text, suggested_text, description_en, description_zh, severity, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(annotation.conversation_id)
        .bind(annotation.annotation_type.to_string())
        .bind(annotation.start_position)
        .bind(annotation.end_position)
        .bind(&annotation.original_text)
        .bind(&annotation.suggested_text)
        .bind(&annotation.description_en)
        .bind(&annotation.description_zh)
        .bind(annotation.severity.to_string())
        .bind(annotation.created_at)
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    /// Get annotations for a conversation
    pub async fn get_annotations(
        &self,
        conversation_id: i64,
    ) -> Result<Vec<ConversationAnnotation>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT * FROM conversation_annotations WHERE conversation_id = ?"
        )
        .bind(conversation_id)
        .fetch_all(&self.pool)
        .await?;

        let mut annotations = Vec::new();
        for row in rows {
            annotations.push(ConversationAnnotation {
                id: row.get("id"),
                conversation_id: row.get("conversation_id"),
                annotation_type: row.get::<String, _>("annotation_type").parse().unwrap(),
                start_position: row.get("start_position"),
                end_position: row.get("end_position"),
                original_text: row.get("original_text"),
                suggested_text: row.get("suggested_text"),
                description_en: row.get("description_en"),
                description_zh: row.get("description_zh"),
                severity: row.get::<String, _>("severity").parse().unwrap(),
                created_at: row.get("created_at"),
            });
        }

        Ok(annotations)
    }

    // ============ Learning Session Operations ============

    /// Create a new learning session
    pub async fn create_session(&self, session: &LearningSession) -> Result<i64, sqlx::Error> {
        let result = sqlx::query(
            r#"
            INSERT INTO learning_sessions (
                session_id, topic, target_words, started_at, total_exchanges
            ) VALUES (?, ?, ?, ?, ?)
            "#
        )
        .bind(&session.session_id)
        .bind(&session.topic)
        .bind(&session.target_words)
        .bind(session.started_at)
        .bind(session.total_exchanges)
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    /// Update session on completion
    pub async fn end_session(
        &self,
        session_id: &str,
        satisfaction: Option<i64>,
        notes: Option<String>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE learning_sessions 
            SET ended_at = ?, user_satisfaction = ?, notes = ?
            WHERE session_id = ?
            "#
        )
        .bind(Self::now())
        .bind(satisfaction)
        .bind(notes)
        .bind(session_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    // ============ Word Practice Log Operations ============

    /// Log a word practice
    pub async fn log_word_practice(
        &self,
        log: &WordPracticeLog,
    ) -> Result<i64, sqlx::Error> {
        let result = sqlx::query(
            r#"
            INSERT INTO word_practice_log (
                word_id, session_id, practiced_at, success_level, notes
            ) VALUES (?, ?, ?, ?, ?)
            "#
        )
        .bind(log.word_id)
        .bind(&log.session_id)
        .bind(log.practiced_at)
        .bind(log.success_level)
        .bind(&log.notes)
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }
}
