-- This file should undo anything in `up.sql`
ALTER TABLE books
ADD kind TEXT NOT NULL;