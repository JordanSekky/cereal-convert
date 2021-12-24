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
    subscriptions,
);
