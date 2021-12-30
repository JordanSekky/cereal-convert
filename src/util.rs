use derive_more::From;
use serde::Serialize;
use tracing::metadata::LevelFilter;
use tracing_subscriber::{prelude::*, Registry};

use crate::honeycomb;

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
