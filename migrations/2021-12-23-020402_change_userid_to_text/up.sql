-- Your SQL goes here
ALTER TABLE subscriptions
DROP COLUMN user_id,
ADD user_id TEXT NOT NULL;