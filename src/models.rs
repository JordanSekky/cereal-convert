use crate::providers::{
    pale, practical_guide,
    royalroad::{self, RoyalRoadBookKind},
    the_daily_grind_patreon, wandering_inn, wandering_inn_patreon,
};
use crate::schema::{
    books, chapter_bodies, chapters, delivery_methods, subscriptions, unsent_chapters,
};

use anyhow::Result;
use chrono::{DateTime, Utc};
use derive_more::{IsVariant, Unwrap};
use diesel::{
    sql_types::{self},
    types::{FromSql, ToSql},
    Identifiable, Queryable,
};
use rusoto_s3::S3Location;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(
    Debug,
    PartialEq,
    Serialize,
    Deserialize,
    AsExpression,
    FromSqlRow,
    Hash,
    Eq,
    Clone,
    IsVariant,
    Unwrap,
)]
#[sql_type = "sql_types::Jsonb"]
pub enum BookKind {
    RoyalRoad(RoyalRoadBookKind),
    Pale,
    APracticalGuideToEvil,
    TheWanderingInn,
    TheWanderingInnPatreon,
    TheDailyGrindPatreon,
}

impl BookKind {
    pub async fn to_new_book(&self) -> Result<NewBook> {
        match &self {
            Self::RoyalRoad(x) => Ok(royalroad::as_new_book(x).await?),
            Self::Pale => Ok(pale::get_book()),
            Self::APracticalGuideToEvil => Ok(practical_guide::get_book()),
            Self::TheWanderingInn => Ok(wandering_inn::get_book()),
            Self::TheWanderingInnPatreon => Ok(wandering_inn_patreon::get_book()),
            Self::TheDailyGrindPatreon => Ok(the_daily_grind_patreon::get_book()),
        }
    }
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

#[derive(Debug, PartialEq, Serialize, Deserialize, AsExpression, FromSqlRow, Hash, Eq)]
#[sql_type = "sql_types::Jsonb"]
pub enum ChapterKind {
    RoyalRoad {
        id: u64,
    },
    Pale {
        url: String,
    },
    APracticalGuideToEvil {
        url: String,
    },
    TheWanderingInn {
        url: String,
    },
    TheWanderingInnPatreon {
        url: String,
        password: Option<String>,
    },
    TheDailyGrindPatreon {
        html: String,
    },
}

impl<DB> ToSql<sql_types::Jsonb, DB> for ChapterKind
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

impl<DB> FromSql<sql_types::Jsonb, DB> for ChapterKind
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

#[derive(Identifiable, Queryable, PartialEq, Debug, Serialize, Hash, Eq, Clone)]
pub struct Book {
    pub id: Uuid,
    pub name: String,
    pub author: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub metadata: BookKind,
}

#[derive(Insertable, PartialEq, Debug)]
#[table_name = "chapters"]
pub struct NewChapter {
    pub name: String,
    pub author: String,
    pub book_id: Uuid,
    pub metadata: ChapterKind,
    pub published_at: DateTime<Utc>,
}

#[derive(Identifiable, Queryable, QueryableByName, PartialEq, Debug, Associations, Hash, Eq)]
#[belongs_to(Book)]
#[table_name = "chapters"]
pub struct Chapter {
    pub id: Uuid,
    pub name: String,
    pub author: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub book_id: Uuid,
    pub published_at: DateTime<Utc>,
    pub metadata: ChapterKind,
}

#[derive(Identifiable, Queryable, PartialEq, Debug, Associations, Serialize, Clone)]
#[belongs_to(Book)]
#[primary_key(user_id, book_id)]
pub struct Subscription {
    pub book_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub user_id: String,
    pub grouping_quantity: i64,
    pub last_chapter_id: Option<Uuid>,
}

#[derive(Identifiable, Queryable, PartialEq, Debug, Associations)]
#[primary_key(user_id)]
#[allow(clippy::struct_excessive_bools)]
pub struct DeliveryMethod {
    pub user_id: String,
    pub kindle_email: Option<String>,
    pub kindle_email_verified: bool,
    pub kindle_email_enabled: bool,
    pub kindle_email_verification_code_time: Option<DateTime<Utc>>,
    pub kindle_email_verification_code: Option<String>,
    pub pushover_key: Option<String>,
    pub pushover_key_verified: bool,
    pub pushover_enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub pushover_verification_code_time: Option<DateTime<Utc>>,
    pub pushover_verification_code: Option<String>,
}

impl DeliveryMethod {
    pub const fn get_pushover_key(&self) -> &Option<String> {
        if self.pushover_enabled && self.pushover_key_verified {
            &self.pushover_key
        } else {
            &None
        }
    }

    pub const fn get_kindle_email(&self) -> &Option<String> {
        if self.kindle_email_enabled && self.kindle_email_verified {
            &self.kindle_email
        } else {
            &None
        }
    }
}

#[derive(Identifiable, Queryable, PartialEq, Debug, Associations)]
#[belongs_to(Chapter)]
pub struct UnsentChapter {
    pub id: Uuid,
    pub user_id: String,
    pub chapter_id: Uuid,
    pub created_at: DateTime<Utc>,
}

#[derive(Insertable, Debug)]
#[table_name = "unsent_chapters"]
pub struct NewUnsentChapter {
    pub user_id: String,
    pub chapter_id: Uuid,
}

#[derive(Identifiable, Queryable, PartialEq, Debug, Associations, Insertable, Hash, Eq, Clone)]
#[table_name = "chapter_bodies"]
#[belongs_to(Chapter)]
#[primary_key(chapter_id)]
pub struct ChapterBody {
    pub key: String,
    pub bucket: String,
    pub chapter_id: Uuid,
}

impl From<ChapterBody> for S3Location {
    fn from(val: ChapterBody) -> Self {
        S3Location {
            prefix: val.key,
            bucket_name: val.bucket,
            ..Default::default()
        }
    }
}
