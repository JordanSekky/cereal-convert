use crate::schema::subscriptions;
use crate::{connection_pool::PgConnectionManager, models::Subscription};

use crate::util::ErrorMessage;
use diesel::{OptionalExtension, QueryDsl, RunQueryDsl};
use mobc::Pool;
use serde::{Deserialize, Serialize};
use tracing::{error, span, Instrument, Level};
use uuid::Uuid;
use warp::http::StatusCode;
use warp::{reply, Filter, Reply};

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
) -> Result<Subscription, Error> {
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
) -> Result<Subscription, Error> {
    let conn = db_pool
        .get()
        .instrument(tracing::info_span!("Acquiring a DB Connection."))
        .await?
        .into_inner();
    let db_span = span!(Level::INFO, "Inserting subscription to db.");
    let db_result: Option<Subscription> = {
        use crate::schema::subscriptions::dsl::*;
        let _a = db_span.enter();
        diesel::delete(subscriptions.find((body.user_id, body.book_id)))
            .get_result(&conn)
            .optional()?
    };
    match db_result {
        None => Err(Error::NotFound),
        Some(x) => Ok(x),
    }
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

fn map_result(result: Result<impl Serialize, Error>) -> impl Reply {
    match result {
        Ok(x) => reply::with_status(reply::json(&x), StatusCode::OK),
        Err(err) => {
            let internal_server_error = (
                StatusCode::INTERNAL_SERVER_ERROR,
                ErrorMessage {
                    message: String::from("An internal exception occurred."),
                },
            );
            let (status, body) = match err {
                Error::EstablishConnection(_) => internal_server_error,
                Error::QueryResult(_) => internal_server_error,
                Error::NotFound => (
                    StatusCode::NOT_FOUND,
                    ErrorMessage {
                        message: "Not found.".into(),
                    },
                ),
            };
            error!(
                "Returning error body: {}, StatusCode: {}, Source: {:?}",
                serde_json::to_string(&body).expect("Failed to serialize outgoing message body."),
                status,
                err
            );
            reply::with_status(reply::json(&body), status)
        }
    }
}

use std::fmt::Display;

#[derive(Debug)]
pub enum Error {
    EstablishConnection(mobc::Error<diesel::ConnectionError>),
    QueryResult(diesel::result::Error),
    NotFound,
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{:?}", self))
    }
}

impl std::error::Error for Error {}

impl From<mobc::Error<diesel::ConnectionError>> for Error {
    fn from(x: mobc::Error<diesel::ConnectionError>) -> Self {
        Error::EstablishConnection(x)
    }
}

impl From<diesel::result::Error> for Error {
    fn from(x: diesel::result::Error) -> Self {
        Error::QueryResult(x)
    }
}
