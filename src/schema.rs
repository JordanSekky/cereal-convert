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
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        book_id -> Uuid,
        published_at -> Timestamptz,
        metadata -> Jsonb,
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
        pushover_verification_code_time -> Nullable<Timestamptz>,
        pushover_verification_code -> Nullable<Text>,
    }
}

table! {
    subscriptions (user_id, book_id) {
        book_id -> Uuid,
        created_at -> Timestamptz,
        user_id -> Text,
    }
}

table! {
    unsent_chapters (id) {
        id -> Uuid,
        user_id -> Text,
        chapter_id -> Uuid,
        created_at -> Timestamptz,
    }
}

joinable!(unsent_chapters -> chapters (chapter_id));

allow_tables_to_appear_in_same_query!(
    books,
    chapters,
    delivery_methods,
    subscriptions,
    unsent_chapters,
);
