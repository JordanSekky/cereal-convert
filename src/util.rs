use std::io::BufWriter;

use anyhow::{bail, Result};
use chrono::Utc;
use derive_more::From;
use mobc::Pool;
use reqwest::Url;
use serde::Serialize;
use tracing::{info, metadata::LevelFilter};
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

pub async fn run_db_migrations(pool: Pool<PgConnectionManager>) -> Result<()> {
    let conn = match pool.get().await {
        Ok(x) => x.into_inner(),
        Err(err) => {
            bail!("Failed to acquire db connection. {:?}", err);
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
