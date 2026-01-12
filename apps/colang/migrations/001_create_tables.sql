-- SQLite Migration: English Learning Companion Database Schema
-- Version: 001
-- Date: 2026-01-08

-- Table: issue_words
-- Stores words that the user has problems with
CREATE TABLE IF NOT EXISTS issue_words (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    word TEXT NOT NULL,
    issue_type TEXT NOT NULL CHECK(issue_type IN ('pronunciation', 'usage', 'unfamiliar', 'grammar')),
    description_en TEXT,
    description_zh TEXT,
    -- Spaced repetition fields
    last_picked_at INTEGER, -- Unix timestamp (seconds)
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    pick_count INTEGER NOT NULL DEFAULT 0,
    -- Review scheduling
    next_review_at INTEGER, -- Unix timestamp for next review
    review_interval_days INTEGER DEFAULT 1, -- Current review interval
    difficulty_level INTEGER DEFAULT 1 CHECK(difficulty_level BETWEEN 1 AND 5),
    -- Additional context
    context TEXT, -- Original sentence where the issue was found
    audio_timestamp INTEGER, -- Timestamp in audio where issue occurred
    
    UNIQUE(word, issue_type)
);

CREATE INDEX IF NOT EXISTS idx_issue_words_next_review 
ON issue_words(next_review_at) WHERE next_review_at IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_issue_words_created 
ON issue_words(created_at);

-- Table: conversations
-- Stores all conversation history
CREATE TABLE IF NOT EXISTS conversations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL, -- UUID for grouping related conversations
    speaker TEXT NOT NULL CHECK(speaker IN ('user', 'teacher')),
    use_lang TEXT NOT NULL CHECK(use_lang IN ('en', 'zh')),
    content_en TEXT NOT NULL,
    content_zh TEXT NOT NULL,
    audio_path TEXT, -- Path to audio file if available
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    duration_ms INTEGER, -- Audio duration in milliseconds
    
    -- Performance metrics
    words_per_minute REAL,
    pause_count INTEGER,
    hesitation_count INTEGER
);

CREATE INDEX IF NOT EXISTS idx_conversations_session 
ON conversations(session_id, created_at);

CREATE INDEX IF NOT EXISTS idx_conversations_created 
ON conversations(created_at);

-- Table: conversation_annotations
-- Stores annotations and issues found in conversations
CREATE TABLE IF NOT EXISTS conversation_annotations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    conversation_id INTEGER NOT NULL,
    annotation_type TEXT NOT NULL CHECK(
        annotation_type IN ('pronunciation_error', 'grammar_error', 'word_choice', 
                           'fluency_issue', 'suggestion', 'correction')
    ),
    start_position INTEGER, -- Character position in text where annotation starts
    end_position INTEGER,   -- Character position where annotation ends
    original_text TEXT,
    suggested_text TEXT,
    description_en TEXT,
    description_zh TEXT,
    severity TEXT CHECK(severity IN ('low', 'medium', 'high')) DEFAULT 'medium',
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    
    FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_annotations_conversation 
ON conversation_annotations(conversation_id);

CREATE INDEX IF NOT EXISTS idx_annotations_type 
ON conversation_annotations(annotation_type);

-- Table: word_practice_log
-- Logs each time a word is practiced
CREATE TABLE IF NOT EXISTS word_practice_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    word_id INTEGER NOT NULL,
    session_id TEXT NOT NULL,
    practiced_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    success_level INTEGER CHECK(success_level BETWEEN 1 AND 5),
    notes TEXT,

    FOREIGN KEY (word_id) REFERENCES issue_words(id) ON DELETE CASCADE,
    FOREIGN KEY (session_id) REFERENCES learning_sessions(session_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_practice_log_word 
ON word_practice_log(word_id, practiced_at);

CREATE INDEX IF NOT EXISTS idx_practice_log_session 
ON word_practice_log(session_id);

-- View: words_due_for_review
-- Helper view to find words that need review
CREATE VIEW IF NOT EXISTS words_due_for_review AS
SELECT 
    w.*,
    COALESCE(
        (SELECT COUNT(*) FROM word_practice_log 
         WHERE word_id = w.id 
         AND practiced_at >= strftime('%s', 'now', '-1 day')),
        0
    ) as today_practice_count
FROM issue_words w
WHERE 
    (w.next_review_at IS NULL OR w.next_review_at <= strftime('%s', 'now'))
    AND COALESCE(
        (SELECT COUNT(*) FROM word_practice_log 
         WHERE word_id = w.id 
         AND practiced_at >= strftime('%s', 'now', '-1 day')),
        0
    ) < 5
ORDER BY 
    CASE 
        WHEN w.next_review_at IS NULL THEN 0
        ELSE 1
    END,
    w.next_review_at ASC,
    w.difficulty_level DESC,
    w.created_at ASC
LIMIT 30;
