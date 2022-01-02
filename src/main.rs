mod calibre;
mod connection_pool;
mod controllers;
mod honeycomb;
mod mailgun;
mod models;
mod pale;
mod pushover;
mod rate_limit;
mod royalroad;
mod schema;
mod storage;
mod tasks;
mod util;
#[macro_use]
extern crate diesel;

use derive_more::{Display, Error};
use tokio::signal;
use tracing::error;

use crate::{connection_pool::establish_connection_pool, controllers::get_server_future};
use util::configure_tracing;

#[tokio::main]
async fn main() -> Result<(), Error> {
    configure_tracing();

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

#[derive(Error, Display, Debug)]
struct Error;
