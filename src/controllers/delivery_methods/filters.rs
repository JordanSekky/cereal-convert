use mobc::Pool;
use serde::Serialize;
use tracing::error;
use warp::{http::StatusCode, reply, Filter, Reply};

use crate::{connection_pool::PgConnectionManager, util::ErrorMessage};

use super::{
    register_kindle_email, register_pushover_key, validate_kindle_email, validate_pushover_key,
    Error,
};

pub fn get_filters(
    db_pool: Pool<PgConnectionManager>,
) -> impl Filter<Extract = impl Reply, Error = warp::Rejection> + Clone {
    let add_db = db_pool.clone();
    let register_email_filter = warp::post()
        .and(warp::path("delivery_methods"))
        .and(warp::path("kindle"))
        .and(warp::path::end())
        .and(warp::body::content_length_limit(1024))
        .and(warp::body::json())
        .and(warp::any().map(move || add_db.clone()))
        .then(register_kindle_email)
        .map(map_result);
    let validate_db = db_pool.clone();
    let validate_email_filter = warp::post()
        .and(warp::path("delivery_methods"))
        .and(warp::path("kindle"))
        .and(warp::path("validate"))
        .and(warp::path::end())
        .and(warp::body::content_length_limit(1024))
        .and(warp::body::json())
        .and(warp::any().map(move || validate_db.clone()))
        .then(validate_kindle_email)
        .map(map_result);
    let add_pool_db = db_pool.clone();
    let register_pushover_filter = warp::post()
        .and(warp::path("delivery_methods"))
        .and(warp::path("pushover"))
        .and(warp::path::end())
        .and(warp::body::content_length_limit(1024))
        .and(warp::body::json())
        .and(warp::any().map(move || add_pool_db.clone()))
        .then(register_pushover_key)
        .map(map_result);
    let validate_pushover_filter = warp::post()
        .and(warp::path("delivery_methods"))
        .and(warp::path("pushover"))
        .and(warp::path("validate"))
        .and(warp::path::end())
        .and(warp::body::content_length_limit(1024))
        .and(warp::body::json())
        .and(warp::any().map(move || db_pool.clone()))
        .then(validate_pushover_key)
        .map(map_result);
    register_email_filter
        .or(validate_email_filter)
        .or(register_pushover_filter)
        .or(validate_pushover_filter)
}

fn map_result(result: Result<impl Serialize, Error>) -> impl Reply {
    match result {
        Ok(x) => reply::with_status(reply::json(&x), StatusCode::OK),
        Err(err) => {
            let internal_server_error: (StatusCode, ErrorMessage) = (
                StatusCode::INTERNAL_SERVER_ERROR,
                "An internal exception occurred.".into(),
            );
            let (status, body) = match &err {
                Error::EstablishConnection(_) => internal_server_error,
                Error::QueryResult(_) => internal_server_error,
                Error::ValidationConversion(_) => internal_server_error,
                Error::ValidationDelivery(_) => internal_server_error,
                Error::Validation(_) => internal_server_error,
                Error::EmailParse(_) => (
                    StatusCode::BAD_REQUEST,
                    "Failed to parse kindle email.".into(),
                ),
                Error::NotKindleEmail => (
                    StatusCode::BAD_REQUEST,
                    "Email address must be a kindle.com address.".into(),
                ),
                Error::NoPushoverKey => (StatusCode::BAD_REQUEST, "No pushover key found.".into()),
                Error::PushoverDelivery(_) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to send pushover notification.".into(),
                ),
            };

            error!(
                "Returning error body: {}, StatusCode: {}, Source: {}",
                serde_json::to_string(&body).expect("Failed to serialize outgoing message body."),
                status,
                &err
            );
            reply::with_status(reply::json(&body), status)
        }
    }
}
