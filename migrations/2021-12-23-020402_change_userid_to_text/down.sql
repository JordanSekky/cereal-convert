-- This file should undo anything in `up.sql`
ALTER TABLE subscriptions
DROP COLUMN user_id,
ADD user_id Uuid NOT NULL;