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
#[macro_use]
extern crate simple_error;
#[macro_use]
extern crate diesel;
use std::sync::Arc;

use tokio::signal;
use tokio::sync::Mutex;
use tracing::metadata::LevelFilter;
use tracing_subscriber::{prelude::__tracing_subscriber_SubscriberExt, Registry};
use ttl_cache::TtlCache;
use warp::Filter;

use crate::{
    connection_pool::establish_connection_pool,
    handlers::{
        royalroad_handler, smtp_handler, ConvertRequestBody, ConvertRequestResponse,
        MailRequestBody,
    },
};

#[tokio::main]
async fn main() {
    let subscriber = Registry::default() // provide underlying span data store
        .with(LevelFilter::INFO) // filter out low-level debug tracing (eg tokio executor)
        .with(tracing_opentelemetry::layer().with_tracer(honeycomb::get_honeycomb_tracer())) // publish to honeycomb backend
        .with(tracing_subscriber::fmt::Layer::default()); // log to stdout
    tracing::subscriber::set_global_default(subscriber).unwrap();

    let storage_location_cache: Arc<Mutex<TtlCache<ConvertRequestBody, ConvertRequestResponse>>> =
        Arc::new(Mutex::new(TtlCache::new(1000)));

    let book_bytes_cache: Arc<Mutex<TtlCache<MailRequestBody, Vec<u8>>>> =
        Arc::new(Mutex::new(TtlCache::new(1000)));

    let royalroad = warp::post()
        .and(warp::path("royalroad"))
        .and(warp::post())
        .and(warp::body::content_length_limit(1024 * 16))
        .and(warp::any().map(move || storage_location_cache.clone()))
        .and(warp::body::json())
        .and_then(royalroad_handler);

    let mail = warp::post()
        .and(warp::path("mail"))
        .and(warp::post())
        .and(warp::body::content_length_limit(1024 * 16))
        .and(warp::any().map(move || book_bytes_cache.clone()))
        .and(warp::body::json())
        .and_then(smtp_handler);

    let pool = establish_connection_pool();

    let create_book = warp::post()
        .and(warp::path("books"))
        .and(warp::post())
        .and(warp::body::content_length_limit(1024))
        .and(warp::any().map(move || pool.clone()))
        .and(warp::body::json())
        .and_then(controllers::books::create_book);

    let server = warp::serve(
        royalroad
            .or(mail)
            .or(create_book)
            .with(warp::trace::request()),
    )
    .run(([0, 0, 0, 0], 3000));
    let cancel = signal::ctrl_c();
    tokio::select! {
    _ = server => 0,
    _ = cancel => { println!("Received exit signal, exiting."); 255}
    };
}
