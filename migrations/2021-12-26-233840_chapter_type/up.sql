-- Your SQL goes here
ALTER TABLE chapters
DROP COLUMN url,
ADD published_at timestamptz NOT NULL DEFAULT NOW(),
ADD metadata JSONB NOT NULL;