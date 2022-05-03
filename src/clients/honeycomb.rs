use std::env;

use opentelemetry::sdk::trace::Tracer;
use opentelemetry_otlp::WithExportConfig;

pub fn get_honeycomb_tracer() -> Tracer {
    let mut map = tonic::metadata::MetadataMap::with_capacity(2);

    map.insert(
        "x-honeycomb-team",
        env::var("HONEYCOMB_API_KEY").unwrap().parse().unwrap(),
    );
    map.insert(
        "x-honeycomb-dataset",
        env::var("HONEYCOMB_DATASET").unwrap().parse().unwrap(),
    );
    let otlp_exporter = opentelemetry_otlp::new_exporter()
        .tonic()
        .with_endpoint("https://api.honeycomb.io")
        .with_metadata(map);
    opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(otlp_exporter)
        .install_simple()
        .unwrap()
}
