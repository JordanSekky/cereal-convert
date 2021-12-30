mod calibre;
mod connection_pool;
mod controllers;
mod honeycomb;
mod mailgun;
mod models;
mod pushover;
mod royalroad;
mod schema;
mod storage;
mod tasks;
mod util;
#[macro_use]
extern crate diesel;

use derive_more::{Display, Error};
use futures::Future;
use tokio::signal;
use tracing::{error, metadata::LevelFilter};
use tracing_subscriber::{prelude::__tracing_subscriber_SubscriberExt, Registry};
use warp::Filter;

use crate::connection_pool::establish_connection_pool;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let subscriber = Registry::default() // provide underlying span data store
        .with(LevelFilter::INFO) // filter out low-level debug tracing (eg tokio executor)
        .with(tracing_opentelemetry::layer().with_tracer(honeycomb::get_honeycomb_tracer())) // publish to honeycomb backend
        .with(tracing_subscriber::fmt::Layer::default()); // log to stdout
    tracing::subscriber::set_global_default(subscriber).unwrap();

    let pool = establish_connection_pool();

    let cancel = tokio::spawn(signal::ctrl_c());
    tokio::pin!(cancel);
    let mut server = Box::pin(tokio::spawn(get_server_future(&pool)));
    let mut check_for_new_chapters =
        Box::pin(tokio::spawn(tasks::check_new_chap_loop(pool.clone())));
    let mut send_notification =
        Box::pin(tokio::spawn(tasks::send_notifications_loop(pool.clone())));

    loop {
        tokio::select! {
        x = &mut server => {
            error!("API server thread failed. Restarting the thread.");
            match x {
                Ok(_) => error!("New chapter check returned OK. This should not be possible."),
                Err(err) => error!(?err, "New chapter check has paniced. This should not be possible."),
            };
            server.set(tokio::spawn(get_server_future(&pool)));

        },
        x = &mut check_for_new_chapters => {
            error!("New chapter check thread failed. Restarting the thread.");
            match x {
                Ok(_) => error!("New chapter check returned OK. This should not be possible."),
                Err(err) => error!(?err, "New chapter check has paniced. This should not be possible."),
            };
            check_for_new_chapters.set(tokio::spawn(tasks::check_new_chap_loop(pool.clone())));

        }
        x = &mut send_notification => {
            error!("Chapter notification thread failed. Restarting the thread.");
            match x {
                Ok(_) => error!("Chapter notification thread returned OK. This should not be possible."),
                Err(err) => error!(?err, "Chapter notification thread returned has paniced. This should not be possible."),
            };
            send_notification.set(tokio::spawn(tasks::send_notifications_loop(pool.clone())));
        }
        _ = &mut cancel => { println!("Received exit signal, exiting."); break}
        }
    }
    Ok(())
}

fn get_server_future(
    pool: &mobc::Pool<connection_pool::PgConnectionManager>,
) -> impl Future<Output = ()> {
    let book_routes = controllers::books::get_filters(pool.clone());
    let delivery_methods_routes = controllers::delivery_methods::get_filters(pool.clone());
    let subscription_routes = controllers::subscriptions::get_filters(pool.clone());
    let api_server_future = warp::serve(
        book_routes
            .or(delivery_methods_routes)
            .or(subscription_routes)
            .with(warp::trace::request()),
    )
    .run(([0, 0, 0, 0], 3000));
    api_server_future
}

#[derive(Error, Display, Debug)]
struct Error;
