-- Your SQL goes here
ALTER TABLE chapters
DROP COLUMN kind;

ALTER TABLE books
ADD kind TEXT NOT NULL;
ALTER TABLE books
ADD metadata JSONB NOT NULL;