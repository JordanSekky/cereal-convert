-- Your SQL goes here
ALTER TABLE chapters ADD book_id UUID NOT NULL;

ALTER TABLE chapters
ADD CONSTRAINT parent_book
FOREIGN KEY (book_id)
REFERENCES books(id)
ON DELETE CASCADE;

ALTER TABLE subscriptions
ADD CONSTRAINT parent_book
FOREIGN KEY (book_id)
REFERENCES books(id)
ON DELETE CASCADE;