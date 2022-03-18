use mobc::Pool;
use warp::{Filter, Reply};

use crate::{connection_pool::PgConnectionManager, util::map_result};

use super::{
    register_kindle_email, register_pushover_key, validate_kindle_email, validate_pushover_key,
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
