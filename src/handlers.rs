use crate::aggregator::get_book_html;
use crate::{calibre, royalroad, storage};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::convert::Infallible;
use std::error::Error;
use std::fs::File;
use std::hash::Hash;
use std::io::Read;
use std::sync::Arc;
use std::time::Duration;
use storage::StorageLocation;
use tokio::sync::Mutex;
use tracing::{info, info_span, span, Instrument, Level};
use ttl_cache::TtlCache;
use uuid::Uuid;

#[derive(Deserialize, Serialize, PartialEq, Eq, Hash, Clone, Debug)]
pub struct ConvertRequestBody {
    chapters: BTreeSet<u32>,
}

#[derive(Serialize, Clone, Debug)]
pub struct ConvertRequestResponse {
    pub key: String,
    pub bucket: String,
}

impl From<StorageLocation> for ConvertRequestResponse {
    fn from(storage_location: StorageLocation) -> Self {
        ConvertRequestResponse {
            key: storage_location.key,
            bucket: storage_location.bucket,
        }
    }
}

#[tracing::instrument(
    name = "Caching a new RoyalRoad set of chapters",
    err,
    level = "warn"
    skip(db),
    fields(
        request_id = %Uuid::new_v4(),
    )
)]
pub async fn royalroad_handler(
    db: Arc<Mutex<TtlCache<ConvertRequestBody, ConvertRequestResponse>>>,
    body: ConvertRequestBody,
) -> Result<impl warp::Reply, Infallible> {
    match convert_and_store_book(db, body).await {
        Ok(output) => Ok(warp::reply::with_status(
            warp::reply::json(&output),
            warp::http::StatusCode::OK,
        )),
        Err(err) => Ok(warp::reply::with_status(
            warp::reply::json(&err.as_ref().to_string()),
            warp::http::StatusCode::INTERNAL_SERVER_ERROR,
        )),
    }
}

async fn convert_and_store_book(
    db: Arc<Mutex<TtlCache<ConvertRequestBody, ConvertRequestResponse>>>,
    body: ConvertRequestBody,
) -> Result<ConvertRequestResponse, Box<dyn Error>> {
    info!("Received chapters: {:?}", body.chapters);
    let mut db_lock = db
        .as_ref()
        .lock()
        .instrument(info_span!("Waiting for db lock."))
        .await;
    if let Some(response) = db_lock.get(&body) {
        return Ok(response.clone());
    }
    info!("Cache miss! Fetching chapters from royalroad.");
    let book = royalroad::download_book(&body.chapters)
        .instrument(info_span!("Fetching chapters from royalroad."))
        .await?;
    let aggregate = {
        let span = info_span!("Aggregating chapter contents.");
        let _guard = span.enter();
        get_book_html(&book)
    };

    let converted_book_path = {
        let span = &info_span!("Converting chapter contents to mobi.");
        let _guard = span.enter();
        calibre::convert_to_mobi(&aggregate)?
    };
    info!("Uploading mobi file to cloud storage.");
    let response: ConvertRequestResponse = {
        let mut book_bytes = Vec::new();
        File::open(converted_book_path)?.read_to_end(&mut book_bytes)?;
        storage::store_book(book_bytes)
            .instrument(info_span!("Storing mobi to cloud storage."))
            .await?
            .into()
    };
    db_lock.insert(body, response.clone(), Duration::from_secs(60 * 60 * 24));
    info!("Returning new response: {:?}", response);
    Ok(response)
}
