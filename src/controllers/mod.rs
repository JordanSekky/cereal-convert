use futures::Future;
use warp::Filter;

use crate::connection_pool;

pub mod books;
pub mod delivery_methods;
pub mod subscriptions;

pub fn get_server_future(
    pool: &mobc::Pool<connection_pool::PgConnectionManager>,
) -> impl Future<Output = ()> {
    let book_routes = books::get_filters(pool.clone());
    let delivery_methods_routes = delivery_methods::get_filters(pool.clone());
    let subscription_routes = subscriptions::get_filters(pool.clone());
    let api_server_future = warp::serve(
        book_routes
            .or(delivery_methods_routes)
            .or(subscription_routes)
            .with(warp::trace::request()),
    )
    .run(([0, 0, 0, 0], 3000));
    api_server_future
}
