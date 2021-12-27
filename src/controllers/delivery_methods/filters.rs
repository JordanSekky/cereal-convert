use mobc::Pool;
use serde::Serialize;
use tracing::error;
use warp::{http::StatusCode, reply, Filter, Reply};

use crate::{connection_pool::PgConnectionManager, util::ErrorMessage};

use super::{register_kindle_email, validate_kindle_email, Error};

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
    let validate_email_filter = warp::post()
        .and(warp::path("delivery_methods"))
        .and(warp::path("kindle"))
        .and(warp::path("validate"))
        .and(warp::path::end())
        .and(warp::body::content_length_limit(1024))
        .and(warp::body::json())
        .and(warp::any().map(move || db_pool.clone()))
        .then(validate_kindle_email)
        .map(map_result);
    register_email_filter.or(validate_email_filter)
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
            let (status, body) = match &err {
                Error::EstablishConnection(_) => internal_server_error,
                Error::QueryResult(_) => internal_server_error,
                Error::Validation(x) => (
                    StatusCode::BAD_REQUEST,
                    ErrorMessage {
                        message: format!("Validation Error: {}", x),
                    },
                ),
                Error::ValidationConversion(_) => internal_server_error,
                Error::ValidationDelivery(_) => internal_server_error,
                Error::EmailParseError => (
                    StatusCode::BAD_REQUEST,
                    ErrorMessage {
                        message: "Failed to parse kindle email.".into(),
                    },
                ),
                Error::NotKindleEmailError => (
                    StatusCode::BAD_REQUEST,
                    ErrorMessage {
                        message: "Email address must be a kindle.com address.".into(),
                    },
                ),
            };

            error!(
                "Returning error body: {}, StatusCode: {}, Source: {:?}",
                serde_json::to_string(&body).expect("Failed to serialize outgoing message body."),
                status,
                &err
            );
            return reply::with_status(reply::json(&body), status);
        }
    }
}
