use std::sync::Arc;

use futures::Future;
use governor::{Quota, RateLimiter};
use nonzero_ext::nonzero;
use warp::Filter;

use crate::{
    connection_pool, rate_limit::ip_rate_limit_filter, rate_limit::path_method_limit_filter,
};

pub mod books;
pub mod delivery_methods;
pub mod subscriptions;

pub fn get_server_future(
    pool: &mobc::Pool<connection_pool::PgConnectionManager>,
) -> impl Future<Output = ()> {
    let ip_limiter = Arc::new(RateLimiter::keyed(Quota::per_second(nonzero!(5u32))));
    let ip_rate_limiter = ip_rate_limit_filter(ip_limiter);
    let api_limiter = Arc::new(RateLimiter::keyed(Quota::per_second(nonzero!(5u32))));
    let api_rate_limiter = path_method_limit_filter(api_limiter);

    let book_routes = books::get_filters(pool.clone());
    let delivery_methods_routes = delivery_methods::get_filters(pool.clone());
    let subscription_routes = subscriptions::get_filters(pool.clone());

    warp::serve(
        ip_rate_limiter
            .or(api_rate_limiter)
            .or(book_routes)
            .or(delivery_methods_routes)
            .or(subscription_routes)
            .with(warp::trace::request()),
    )
    .run(([0, 0, 0, 0], 3000))
}
