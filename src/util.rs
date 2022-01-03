use std::io::BufWriter;

use derive_more::{Display, Error, From};
use mobc::Pool;
use serde::Serialize;
use tracing::{error, info, metadata::LevelFilter};
use tracing_subscriber::{prelude::*, Registry};

use crate::{connection_pool::PgConnectionManager, embedded_migrations, honeycomb};

#[derive(Serialize, From)]
pub struct ErrorMessage {
    pub message: String,
}

impl From<&str> for ErrorMessage {
    fn from(x: &str) -> Self {
        x.to_owned().into()
    }
}

pub fn configure_tracing() {
    let subscriber = Registry::default() // provide underlying span data store
        .with(LevelFilter::INFO) // filter out low-level debug tracing (eg tokio executor)
        .with(tracing_opentelemetry::layer().with_tracer(honeycomb::get_honeycomb_tracer())) // publish to honeycomb backend
        .with(tracing_subscriber::fmt::Layer::default());
    tracing::subscriber::set_global_default(subscriber).unwrap();
}

pub async fn run_db_migrations(pool: Pool<PgConnectionManager>) -> Result<(), Error> {
    let conn = match pool.get().await {
        Ok(x) => x.into_inner(),
        Err(err) => {
            error!(?err, "Failed to acquire db connection.");
            return Err(Error);
        }
    };
    let mut buf = BufWriter::new(Vec::new());
    match embedded_migrations::run_with_output(&conn, &mut buf) {
        Ok(_) => {
            let buf = buf.into_inner().unwrap();
            let migration_out = String::from_utf8_lossy(&buf);
            info!(%migration_out)
        }
        Err(err) => {
            error!(?err, "Failed to run db migrations.");
            return Err(Error);
        }
    };
    return Ok(());
}

#[derive(Error, Display, Debug)]
pub struct Error;
