use crate::schema::subscriptions;
use crate::{connection_pool::PgConnectionManager, models::Subscription};

use crate::util::map_result;
use anyhow::anyhow;
use anyhow::Result;
use diesel::{OptionalExtension, QueryDsl, RunQueryDsl};
use mobc::Pool;
use serde::Deserialize;
use tracing::{span, Instrument, Level};
use uuid::Uuid;
use warp::{Filter, Reply};

#[derive(Debug, Deserialize, Insertable)]
#[table_name = "subscriptions"]
pub struct SubscriptionRequest {
    book_id: Uuid,
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
    db_pool: Pool<PgConnectionManager>,
    body: SubscriptionRequest,
) -> Result<Subscription> {
    let conn = db_pool
        .get()
        .instrument(tracing::info_span!("Acquiring a DB Connection."))
        .await?
        .into_inner();
    let db_span = span!(Level::INFO, "Inserting subscription to db.");
    let db_result: Subscription = {
        let _a = db_span.enter();
        diesel::insert_into(subscriptions::table)
            .values(body)
            .get_result(&conn)?
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
    db_pool: Pool<PgConnectionManager>,
    body: SubscriptionRequest,
) -> Result<Subscription> {
    let conn = db_pool
        .get()
        .instrument(tracing::info_span!("Acquiring a DB Connection."))
        .await?
        .into_inner();
    let db_span = span!(Level::INFO, "Inserting subscription to db.");
    return {
        use crate::schema::subscriptions::dsl::*;
        let _a = db_span.enter();
        diesel::delete(subscriptions.find((&body.user_id, &body.book_id)))
            .get_result(&conn)
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
    db_pool: Pool<PgConnectionManager>,
) -> impl Filter<Extract = impl Reply, Error = warp::Rejection> + Clone {
    let create_sub_db = db_pool.clone();
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
    create_sub_filter.or(delete_sub_filter)
}
