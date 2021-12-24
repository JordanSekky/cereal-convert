-- This file should undo anything in `up.sql`
ALTER TABLE chapters DROP COLUMN book_id CASCADE;

ALTER TABLE subscriptions
DROP constraint parent_book;