use crate::models::Book;
use crate::models::Subscription;
use crate::schema::subscriptions;

use crate::util::{map_result, InstrumentedPgConnectionPool};
use anyhow::anyhow;
use anyhow::Result;
use diesel::{OptionalExtension, QueryDsl, RunQueryDsl};
use serde::Deserialize;
use tracing::{span, Level};
use uuid::Uuid;
use warp::{Filter, Reply};

#[derive(Debug, Deserialize, Insertable)]
#[table_name = "subscriptions"]
pub struct SubscriptionRequest {
    book_id: Uuid,
    user_id: String,
}

#[derive(Debug, Deserialize)]
pub struct ListSubscriptionsRequest {
    user_id: String,
}

#[tracing::instrument(
name = "Creating a new subscription.",
err,
level = "info"
skip(db_pool),
fields(
    request_id = %Uuid::new_v4(),
)
)]
pub async fn create_subscription(
    db_pool: InstrumentedPgConnectionPool,
    body: SubscriptionRequest,
) -> Result<Subscription> {
    let conn = db_pool.get().await?;
    let db_span = span!(Level::INFO, "Inserting subscription to db.");
    let db_result: Subscription = {
        let _a = db_span.enter();
        diesel::insert_into(subscriptions::table)
            .values(body)
            .get_result(&*conn)?
    };
    Ok(db_result)
}

#[tracing::instrument(
name = "Listing subscriptions.",
err,
level = "info"
skip(db_pool),
fields(
    request_id = %Uuid::new_v4(),
)
)]
pub async fn list_subscriptions(
    db_pool: InstrumentedPgConnectionPool,
    body: ListSubscriptionsRequest,
) -> Result<Vec<Book>> {
    let conn = db_pool.get().await?;
    let db_span = span!(Level::INFO, "Fetching subscriptions from db.");
    let db_result = {
        use crate::diesel::prelude::*;
        use crate::schema::books;
        use crate::schema::subscriptions::dsl::*;
        let _a = db_span.enter();
        subscriptions
            .filter(user_id.eq(&body.user_id))
            .inner_join(books::table.on(books::id.eq(book_id)))
            .load::<(Subscription, Book)>(&*conn)?
            .into_iter()
            .map(|(_, book)| book)
            .collect()
    };
    Ok(db_result)
}

#[tracing::instrument(
name = "Delete a subscription.",
err,
level = "info"
skip(db_pool),
fields(
    request_id = %Uuid::new_v4(),
)
)]
pub async fn delete_subscription(
    db_pool: InstrumentedPgConnectionPool,
    body: SubscriptionRequest,
) -> Result<Subscription> {
    let conn = db_pool.get().await?;
    let db_span = span!(Level::INFO, "Inserting subscription to db.");
    return {
        use crate::schema::subscriptions::dsl::*;
        let _a = db_span.enter();
        diesel::delete(subscriptions.find((&body.user_id, &body.book_id)))
            .get_result(&*conn)
            .optional()?
            .ok_or_else(|| {
                anyhow!(
                    "Subscription for body {} and book {} did not already exist.",
                    body.user_id,
                    body.book_id
                )
            })
    };
}

pub fn get_filters(
    db_pool: InstrumentedPgConnectionPool,
) -> impl Filter<Extract = impl Reply, Error = warp::Rejection> + Clone {
    let create_sub_db = db_pool.clone();
    let list_subs_db = db_pool.clone();
    let list_subs_filter = warp::get()
        .and(warp::path("subscriptions"))
        .and(warp::path::end())
        .and(warp::any().map(move || list_subs_db.clone()))
        .and(warp::query())
        .then(list_subscriptions)
        .map(map_result);
    let create_sub_filter = warp::post()
        .and(warp::path("subscriptions"))
        .and(warp::path::end())
        .and(warp::body::content_length_limit(1024))
        .and(warp::any().map(move || create_sub_db.clone()))
        .and(warp::body::json())
        .then(create_subscription)
        .map(map_result);
    let delete_sub_filter = warp::delete()
        .and(warp::path("subscriptions"))
        .and(warp::path::end())
        .and(warp::body::content_length_limit(1024))
        .and(warp::any().map(move || db_pool.clone()))
        .and(warp::body::json())
        .then(delete_subscription)
        .map(map_result);
    create_sub_filter.or(delete_sub_filter).or(list_subs_filter)
}
