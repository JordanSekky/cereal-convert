table! {
    books (id) {
        id -> Uuid,
        name -> Text,
        author -> Text,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        metadata -> Jsonb,
    }
}

table! {
    chapters (id) {
        id -> Uuid,
        name -> Text,
        author -> Text,
        url -> Text,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        book_id -> Uuid,
    }
}

table! {
    delivery_methods (user_id) {
        user_id -> Text,
        kindle_email -> Nullable<Text>,
        kindle_email_verified -> Bool,
        kindle_email_enabled -> Bool,
        kindle_email_verification_code_time -> Nullable<Timestamptz>,
        kindle_email_verification_code -> Nullable<Text>,
        pushover_key -> Nullable<Text>,
        pushover_key_verified -> Bool,
        pushover_enabled -> Bool,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

table! {
    subscriptions (id) {
        id -> Uuid,
        book_id -> Uuid,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        user_id -> Text,
    }
}

allow_tables_to_appear_in_same_query!(
    books,
    chapters,
    delivery_methods,
    subscriptions,
);
