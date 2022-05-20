-- Your SQL goes here
ALTER TABLE subscriptions
ADD COLUMN grouping_quantity BIGINT DEFAULT 1 CHECK(grouping_quantity > 0) NOT NULL,
ADD COLUMN last_chapter_id UUID DEFAULT null,
ADD CONSTRAINT last_chapter
FOREIGN KEY (last_chapter_id)
REFERENCES chapters(id)
ON DELETE SET NULL;