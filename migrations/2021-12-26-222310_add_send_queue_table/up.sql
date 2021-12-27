-- Your SQL goes here
CREATE TABLE unsent_chapters(
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id TEXT NOT NULL,
  chapter_id uuid NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

ALTER TABLE unsent_chapters
ADD CONSTRAINT parent_chapter
FOREIGN KEY (chapter_id)
REFERENCES chapters(id)
ON DELETE CASCADE;