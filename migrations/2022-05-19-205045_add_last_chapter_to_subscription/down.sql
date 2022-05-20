-- This file should undo anything in `up.sql`
-- Your SQL goes here
ALTER TABLE subscriptions
DROP COLUMN grouping_quantity,
DROP COLUMN last_chapter_id;