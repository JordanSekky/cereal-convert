-- Your SQL goes here
ALTER TABLE delivery_methods
ADD pushover_verification_code_time timestamptz,
ADD pushover_verification_code TEXT;