-- Your SQL goes here

CREATE TABLE delivery_methods (
    user_id TEXT PRIMARY KEY NOT NULL,
    kindle_email TEXT,
    kindle_email_verified BOOLEAN NOT NULL DEFAULT false,
    kindle_email_enabled BOOLEAN NOT NULL DEFAULT false,
    kindle_email_verification_code_time timestamptz,
    kindle_email_verification_code TEXT,


    pushover_key TEXT,
    pushover_key_verified BOOLEAN NOT NULL DEFAULT false,
    pushover_enabled BOOLEAN NOT NULL DEFAULT false,

    created_at timestamptz NOT NULL DEFAULT NOW(),
    updated_at timestamptz NOT NULL DEFAULT NOW()
);

SELECT diesel_manage_updated_at('delivery_methods');
SELECT diesel_manage_updated_at('books');
SELECT diesel_manage_updated_at('subscriptions');
SELECT diesel_manage_updated_at('chapters');