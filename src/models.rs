use crate::schema::{books, chapters, delivery_methods, subscriptions};

use chrono::{DateTime, Utc};
use diesel::{
    sql_types::{self},
    types::{FromSql, ToSql},
    Identifiable, Queryable,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, PartialEq, Serialize, Deserialize, AsExpression, FromSqlRow)]
#[sql_type = "sql_types::Jsonb"]
pub enum BookKind {
    RoyalRoad { id: u64 },
}

impl<DB> ToSql<sql_types::Jsonb, DB> for BookKind
where
    DB: diesel::backend::Backend,
    serde_json::Value: ToSql<sql_types::Jsonb, DB>,
{
    fn to_sql<W: std::io::Write>(
        &self,
        out: &mut diesel::serialize::Output<W, DB>,
    ) -> diesel::serialize::Result {
        serde_json::to_value(self)?.to_sql(out)
    }
}

impl<DB> FromSql<sql_types::Jsonb, DB> for BookKind
where
    DB: diesel::backend::Backend,
    serde_json::Value: FromSql<sql_types::Jsonb, DB>,
{
    fn from_sql(bytes: Option<&DB::RawValue>) -> diesel::deserialize::Result<Self> {
        let value = serde_json::Value::from_sql(bytes)?;
        Ok(serde_json::from_value(value)?)
    }
}

#[derive(Insertable, Debug)]
#[table_name = "books"]
pub struct NewBook {
    pub name: String,
    pub author: String,
    pub metadata: BookKind,
}

#[derive(Identifiable, Queryable, PartialEq, Debug, Serialize)]
pub struct Book {
    id: Uuid,
    name: String,
    author: String,
    created_at: DateTime<Utc>,
    modified_at: DateTime<Utc>,
    metadata: BookKind,
}

#[derive(Identifiable, Queryable, PartialEq, Debug, Associations)]
pub struct Chapter {
    id: Uuid,
    name: String,
    author: String,
    url: String,
    book_id: Uuid,
    created_at: DateTime<Utc>,
    modified_at: DateTime<Utc>,
}

#[derive(Identifiable, Queryable, PartialEq, Debug, Associations)]
#[belongs_to(Book)]
pub struct Subscription {
    id: Uuid,
    name: String,
    author: String,
    url: String,
    book_id: Uuid,
    created_at: DateTime<Utc>,
    modified_at: DateTime<Utc>,
}

#[derive(Identifiable, Queryable, PartialEq, Debug, Associations)]
#[primary_key(user_id)]
pub struct DeliveryMethod {
    pub user_id: String,
    pub kindle_email: Option<String>,
    pub kindle_email_verified: bool,
    pub kindle_email_enabled: bool,
    pub kindle_email_verification_code_time: Option<DateTime<Utc>>,
    pub kindle_email_verification_code: Option<String>,
    pub pushover_key: Option<String>,
    pub pushover_key_verified: bool,
    pub pushover_key_enabled: bool,
    pub created_at: DateTime<Utc>,
    pub modified_at: DateTime<Utc>,
}
