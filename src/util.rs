use std::io::BufWriter;

use anyhow::{bail, Result};
use chrono::Utc;
use derive_more::From;
use mobc::Pool;
use reqwest::Url;
use serde::Serialize;
use tracing::{error, info, metadata::LevelFilter, Instrument};
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

pub async fn run_db_migrations(pool: InstrumentedPgConnectionPool) -> Result<()> {
    let conn = pool.get().await?;
    let mut buf = BufWriter::new(Vec::new());
    match embedded_migrations::run_with_output(&*conn, &mut buf) {
        Ok(_) => {
            let buf = buf.into_inner().unwrap();
            let migration_out = String::from_utf8_lossy(&buf);
            info!(%migration_out)
        }
        Err(err) => {
            bail!("Failed to run db migrations. {:?}", err);
        }
    };
    Ok(())
}

//
// Extension trait for Result types.
//

/// Extension trait for Result types.
pub trait ResultExt<T, E> {
    /// Unwraps a result, yielding the content of an [`Ok`].
    ///
    /// If its an error case, it logs the error and returns the output of the else case.
    ///
    fn unwrap_or_else_log(self, else_case: fn() -> T) -> T
    where
        E: Into<anyhow::Error>;
}

impl<T, E> ResultExt<T, E> for Result<T, E> {
    #[inline]
    #[track_caller]
    fn unwrap_or_else_log(self, else_case: fn() -> T) -> T
    where
        E: Into<anyhow::Error>,
    {
        match self {
            Ok(t) => t,
            Err(e) => {
                let e: anyhow::Error = e.into();
                tracing::error!(
                    "called `Result::unwrap_or_else_log()` on an `Err` value:\n {:?}",
                    e
                );
                else_case()
            }
        }
    }
}

pub fn parse_from_rfc2822(pub_date: &str) -> Result<chrono::DateTime<Utc>> {
    Ok(chrono::DateTime::parse_from_rfc2822(pub_date)?.with_timezone(&Utc))
}

pub fn validate_hostname(url: &str, valid_host: &str) -> Result<()> {
    let request_url = Url::parse(url)?;
    match request_url.host_str() {
        Some(host) => {
            if valid_host != host {
                bail!("Provided hostname {} is not {}.", host, valid_host);
            };
        }
        None => bail!("Url {} has no host.", request_url),
    }
    Ok(())
}

pub fn map_result(result: Result<impl Serialize>) -> impl warp::Reply {
    use warp::reply;
    match result {
        Ok(x) => reply::with_status(reply::json(&x), reqwest::StatusCode::OK),
        Err(err) => {
            error!(?err, "An uncaught error occurred.");
            reply::with_status(
                reply::json(&"An internal exception occurred."),
                reqwest::StatusCode::INTERNAL_SERVER_ERROR,
            )
        }
    }
}

#[derive(Clone)]
pub struct InstrumentedPgConnectionPool(pub Pool<PgConnectionManager>);

impl InstrumentedPgConnectionPool {
    pub async fn get(
        &self,
    ) -> Result<mobc::Connection<PgConnectionManager>, mobc::Error<diesel::ConnectionError>> {
        self.0
            .get()
            .instrument(tracing::info_span!("Fetching Database Connection"))
            .await
    }
}
