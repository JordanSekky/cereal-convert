mod aggregator;
mod calibre;
mod chapter;
mod handlers;
mod royalroad;
mod smtp;
mod storage;
#[macro_use]
extern crate simple_error;
extern crate pretty_env_logger;
#[macro_use]
extern crate log;
use std::env;
use std::sync::Arc;

use tokio::signal;
use tokio::sync::Mutex;
use ttl_cache::TtlCache;
use warp::Filter;

use crate::handlers::{royalroad_handler, ConvertRequestBody, ConvertRequestResponse};

#[tokio::main]
async fn main() {
    if env::var_os("RUST_LOG").is_none() {
        // Set `RUST_LOG=todos=debug` to see debug logs,
        // this only shows access logs.
        env::set_var("RUST_LOG", "info,royalroad=info");
    }
    pretty_env_logger::init();

    let storage_location_cache: Arc<Mutex<TtlCache<ConvertRequestBody, ConvertRequestResponse>>> =
        Arc::new(Mutex::new(TtlCache::new(1000)));

    let royalroad = warp::post()
        .and(warp::path("royalroad"))
        .and(warp::post())
        .and(warp::body::content_length_limit(1024 * 16))
        .and(warp::any().map(move || storage_location_cache.clone()))
        .and(warp::body::json())
        .and_then(royalroad_handler)
        .with(warp::log("royalroad"));

    let server = warp::serve(royalroad).run(([0, 0, 0, 0], 80));
    let cancel = signal::ctrl_c();
    tokio::select! {
      _ = server => 0,
      _ = cancel => { println!("Received exit signal, exiting."); 255}
    };
}
