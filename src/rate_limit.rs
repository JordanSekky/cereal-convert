use std::{net::SocketAddr, sync::Arc};

use governor::{clock, state::keyed::DefaultKeyedStateStore, RateLimiter};
use reqwest::{Method, StatusCode};
use warp::{
    filters::BoxedFilter,
    path::Peek,
    reply::{Json, WithStatus},
    Filter, Rejection, Reply,
};

use crate::util::ErrorMessage;

pub fn ip_rate_limit_filter(
    limiter: Arc<
        RateLimiter<
            Option<SocketAddr>,
            DefaultKeyedStateStore<Option<SocketAddr>>,
            clock::DefaultClock,
        >,
    >,
) -> BoxedFilter<(impl Reply,)> {
    warp::addr::remote()
        .and(warp::any().map(move || limiter.clone()))
        .and_then(check_ip_limiter)
        .boxed()
}

async fn check_ip_limiter(
    ip: Option<SocketAddr>,
    limiter: Arc<
        RateLimiter<
            Option<SocketAddr>,
            DefaultKeyedStateStore<Option<SocketAddr>>,
            clock::DefaultClock,
        >,
    >,
) -> Result<WithStatus<Json>, Rejection> {
    let rate_limit_reply = warp::reply::with_status(
        warp::reply::json(&ErrorMessage {
            message: "IP Rate Limit".into(),
        }),
        StatusCode::TOO_MANY_REQUESTS,
    );
    let response = limiter.check_key(&ip);
    match response {
        Ok(_) => Err(warp::reject()),
        Err(_) => Ok(rate_limit_reply),
    }
}

type PathLimiter = Arc<
    RateLimiter<(String, Method), DefaultKeyedStateStore<(String, Method)>, clock::DefaultClock>,
>;

pub fn path_method_limit_filter(limiter: PathLimiter) -> BoxedFilter<(impl Reply,)> {
    warp::path::peek()
        .and(warp::method())
        .and(warp::any().map(move || limiter.clone()))
        .and_then(check_path_limiter)
        .boxed()
}

async fn check_path_limiter(
    path: Peek,
    method: Method,
    limiter: PathLimiter,
) -> Result<WithStatus<Json>, Rejection> {
    let rate_limit_reply = warp::reply::with_status(
        warp::reply::json(&ErrorMessage {
            message: "API Rate Limit".into(),
        }),
        StatusCode::TOO_MANY_REQUESTS,
    );
    let response = limiter.check_key(&(path.as_str().into(), method));
    match response {
        Ok(_) => Err(warp::reject()),
        Err(_) => Ok(rate_limit_reply),
    }
}
