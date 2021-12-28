-- This file should undo anything in `up.sql`
-- Your SQL goes here
ALTER TABLE subscriptions
DROP CONSTRAINT subscriptions_pkey,
ADD updated_at timestamptz NOT NULL DEFAULT NOW(),
ADD id uuid NOT NULL DEFAULT gen_random_uuid(),
ADD PRIMARY KEY (user_id, book_id);