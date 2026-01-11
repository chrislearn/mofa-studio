-- SQLite Migration: Add Chinese description fields
-- Version: 002
-- Date: 2026-01-11

-- Rename issue_description to issue_description_en and add issue_description_zh in issue_words table
ALTER TABLE issue_words RENAME COLUMN issue_description TO issue_description_en;
ALTER TABLE issue_words ADD COLUMN issue_description_zh TEXT;

-- Rename description to description_en and add description_zh in conversation_annotations table
ALTER TABLE conversation_annotations RENAME COLUMN description TO description_en;
ALTER TABLE conversation_annotations ADD COLUMN description_zh TEXT;
