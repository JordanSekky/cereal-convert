-- This file should undo anything in `up.sql`
ALTER TABLE chapters
ADD kind TEXT NOT NULL;

ALTER TABLE books
DROP COLUMN kind;
ALTER TABLE books
DROP COLUMN metadata;