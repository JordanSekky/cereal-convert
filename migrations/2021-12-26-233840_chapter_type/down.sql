-- This file should undo anything in `up.sql`
ALTER TABLE chapters
DROP COLUMN metadata,
ADD url TEXT NOT NULL;