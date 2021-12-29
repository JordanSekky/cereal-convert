-- Your SQL goes here
CREATE TABLE chapter_bodies(
    key TEXT NOT NULL,
    bucket TEXT NOT NULL,
    chapter_id Uuid PRIMARY KEY,
    CONSTRAINT fk_chapter_id FOREIGN KEY(chapter_id) REFERENCES chapters(id) ON DELETE CASCADE
)