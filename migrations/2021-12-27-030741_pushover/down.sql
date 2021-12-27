-- This file should undo anything in `up.sql`
ALTER TABLE delivery_methods
DROP COLUMN pushover_verification_code_time,
DROP COLUMN pushover_verification_code;