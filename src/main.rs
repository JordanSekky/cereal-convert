mod aggregator;
mod calibre;
mod chapter;
mod connection_pool;
mod controllers;
mod handlers;
mod honeycomb;
mod models;
mod royalroad;
mod schema;
mod smtp;
mod storage;
mod tasks;
mod util;
#[macro_use]
extern crate simple_error;
#[macro_use]
extern crate diesel;

use tokio::signal;
use tracing::metadata::LevelFilter;
use tracing_subscriber::{prelude::__tracing_subscriber_SubscriberExt, Registry};
use warp::Filter;

use crate::connection_pool::establish_connection_pool;

#[tokio::main]
async fn main() {
    let subscriber = Registry::default() // provide underlying span data store
        .with(LevelFilter::INFO) // filter out low-level debug tracing (eg tokio executor)
        .with(tracing_opentelemetry::layer().with_tracer(honeycomb::get_honeycomb_tracer())) // publish to honeycomb backend
        .with(tracing_subscriber::fmt::Layer::default()); // log to stdout
    tracing::subscriber::set_global_default(subscriber).unwrap();

    let pool = establish_connection_pool();

    let book_routes = controllers::books::get_filters(pool.clone());
    let delivery_methods_routes = controllers::delivery_methods::get_filters(pool.clone());

    let api_server_future = warp::serve(
        book_routes
            .or(delivery_methods_routes)
            .with(warp::trace::request()),
    )
    .run(([0, 0, 0, 0], 3000));
    let cancel = tokio::spawn(signal::ctrl_c());
    let server = tokio::spawn(api_server_future);
    let check_for_new_chapters = tokio::spawn(tasks::check_new_chap_loop(pool.clone()));
    tokio::select! {
    _ = server => 0,
    _ = check_for_new_chapters => { println!("New chapter check thread failed. Exiting"); 255}
    _ = cancel => { println!("Received exit signal, exiting."); 255}
    };
}
