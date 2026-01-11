-- Sample seed data for English Learning Companion
-- Run this to populate database with common problematic words for English learners

-- Common pronunciation challenges
INSERT OR IGNORE INTO issue_words (word, issue_type, description_en, created_at, pick_count, review_interval_days, difficulty_level, context)
VALUES 
    ('through', 'pronunciation', 'Often mispronounced as "throw" or "true"', strftime('%s', 'now'), 0, 1, 3, 'I walked through the park.'),
    ('comfortable', 'pronunciation', 'Many syllables, often shortened incorrectly', strftime('%s', 'now'), 0, 1, 3, 'This chair is very comfortable.'),
    ('throughout', 'pronunciation', 'Challenging th sound at beginning', strftime('%s', 'now'), 0, 1, 3, 'It rained throughout the day.'),
    ('schedule', 'pronunciation', 'Different pronunciation in US (sked-jool) vs UK (shed-yool)', strftime('%s', 'now'), 0, 1, 2, 'What is your schedule today?'),
    ('colleague', 'pronunciation', 'Silent ue at the end', strftime('%s', 'now'), 0, 1, 2, 'My colleague helped me with the project.'),
    ('comfortable', 'pronunciation', 'Often reduced to "comf-ter-ble"', strftime('%s', 'now'), 0, 1, 3, 'Make yourself comfortable.'),
    ('successful', 'pronunciation', 'Multiple syllables with stress pattern', strftime('%s', 'now'), 0, 1, 2, 'She is very successful.'),
    ('particularly', 'pronunciation', 'Long word with specific stress pattern', strftime('%s', 'now'), 0, 1, 3, 'I particularly enjoy reading.');

-- Common grammar/usage issues
INSERT OR IGNORE INTO issue_words (word, issue_type, description_en, created_at, pick_count, review_interval_days, difficulty_level, context)
VALUES
    ('affect', 'usage', 'Often confused with "effect"', strftime('%s', 'now'), 0, 1, 3, 'This will affect our plans.'),
    ('effect', 'usage', 'Often confused with "affect"', strftime('%s', 'now'), 0, 1, 3, 'The effect was immediate.'),
    ('their', 'usage', 'Confused with "there" and "they''re"', strftime('%s', 'now'), 0, 1, 2, 'It is their responsibility.'),
    ('its', 'usage', 'Confused with "it''s" (it is)', strftime('%s', 'now'), 0, 1, 2, 'The dog wagged its tail.'),
    ('accept', 'usage', 'Confused with "except"', strftime('%s', 'now'), 0, 1, 2, 'I accept your offer.'),
    ('advice', 'usage', 'Noun - confused with "advise" (verb)', strftime('%s', 'now'), 0, 1, 2, 'Can you give me some advice?'),
    ('advise', 'usage', 'Verb - confused with "advice" (noun)', strftime('%s', 'now'), 0, 1, 2, 'I advise you to wait.'),
    ('lose', 'usage', 'Often misspelled as "loose"', strftime('%s', 'now'), 0, 1, 2, 'Don''t lose your keys.');

-- Advanced vocabulary for workplace
INSERT OR IGNORE INTO issue_words (word, issue_type, description_en, created_at, pick_count, review_interval_days, difficulty_level, context)
VALUES
    ('accomplish', 'unfamiliar', 'Useful for discussing achievements', strftime('%s', 'now'), 0, 1, 2, 'I want to accomplish this goal.'),
    ('determine', 'unfamiliar', 'Important for decision-making contexts', strftime('%s', 'now'), 0, 1, 2, 'We need to determine the best approach.'),
    ('approach', 'unfamiliar', 'Common in professional settings', strftime('%s', 'now'), 0, 1, 2, 'What is your approach to this problem?'),
    ('confident', 'unfamiliar', 'Expressing certainty and self-assurance', strftime('%s', 'now'), 0, 1, 1, 'I feel confident about the presentation.'),
    ('persuade', 'unfamiliar', 'Convincing others', strftime('%s', 'now'), 0, 1, 3, 'She tried to persuade me to join.'),
    ('efficient', 'unfamiliar', 'Describing productivity', strftime('%s', 'now'), 0, 1, 2, 'This is an efficient method.'),
    ('collaborate', 'unfamiliar', 'Working together', strftime('%s', 'now'), 0, 1, 2, 'Let''s collaborate on this project.'),
    ('implement', 'unfamiliar', 'Putting plans into action', strftime('%s', 'now'), 0, 1, 2, 'We will implement the new policy.');

-- Common conversational words
INSERT OR IGNORE INTO issue_words (word, issue_type, description_en, created_at, pick_count, review_interval_days, difficulty_level, context)
VALUES
    ('although', 'unfamiliar', 'Expressing contrast', strftime('%s', 'now'), 0, 1, 2, 'Although it rained, we went out.'),
    ('however', 'unfamiliar', 'Contrasting ideas', strftime('%s', 'now'), 0, 1, 2, 'I wanted to go; however, I was busy.'),
    ('furthermore', 'unfamiliar', 'Adding information', strftime('%s', 'now'), 0, 1, 3, 'The plan is good. Furthermore, it is affordable.'),
    ('meanwhile', 'unfamiliar', 'Describing simultaneous events', strftime('%s', 'now'), 0, 1, 2, 'I cooked dinner. Meanwhile, she set the table.'),
    ('opportunity', 'unfamiliar', 'Chance or possibility', strftime('%s', 'now'), 0, 1, 2, 'This is a great opportunity.'),
    ('challenge', 'unfamiliar', 'Difficult task', strftime('%s', 'now'), 0, 1, 1, 'This is a real challenge for me.'),
    ('experience', 'unfamiliar', 'Knowledge from practice', strftime('%s', 'now'), 0, 1, 1, 'I have experience with this tool.'),
    ('significant', 'unfamiliar', 'Important or meaningful', strftime('%s', 'now'), 0, 1, 2, 'This is a significant achievement.');

-- Grammar-specific challenges
INSERT OR IGNORE INTO issue_words (word, issue_type, description_en, created_at, pick_count, review_interval_days, difficulty_level, context)
VALUES
    ('have been', 'grammar', 'Present perfect continuous tense', strftime('%s', 'now'), 0, 1, 3, 'I have been working here for two years.'),
    ('would have', 'grammar', 'Past conditional', strftime('%s', 'now'), 0, 1, 4, 'I would have called if I had known.'),
    ('had been', 'grammar', 'Past perfect continuous', strftime('%s', 'now'), 0, 1, 4, 'She had been waiting for hours.'),
    ('ought to', 'grammar', 'Modal verb for advice/obligation', strftime('%s', 'now'), 0, 1, 3, 'You ought to see a doctor.'),
    ('used to', 'grammar', 'Past habits', strftime('%s', 'now'), 0, 1, 2, 'I used to live in New York.'),
    ('get used to', 'grammar', 'Becoming accustomed', strftime('%s', 'now'), 0, 1, 3, 'I am getting used to the weather here.');

-- Verify insertion
SELECT 'Total words inserted:', COUNT(*) FROM issue_words;
SELECT 'By issue type:', issue_type, COUNT(*) 
FROM issue_words 
GROUP BY issue_type 
ORDER BY COUNT(*) DESC;
