-- Your SQL goes here
ALTER TABLE subscriptions
DROP CONSTRAINT subscriptions_pkey,
DROP COLUMN updated_at,
DROP COLUMN id,
ADD PRIMARY KEY (user_id, book_id);