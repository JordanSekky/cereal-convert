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
use ttl_cache::TtlCache;

#[derive(Deserialize, Serialize, PartialEq, Eq, Hash, Clone)]
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
    let mut db_lock = db.as_ref().lock().await;
    if let Some(response) = db_lock.get(&body) {
        info!("Cache hit! Returning previous response.");
        return Ok(response.clone());
    }
    info!("Cache miss! Fetching chapters from royalroad.");
    let book = royalroad::download_book(&body.chapters).await?;
    let aggregate = get_book_html(&book);
    info!("Converting chapters to mobi.");
    let converted_book_path = calibre::convert_to_mobi(&aggregate)?;
    info!("Uploading mobi file to cloud storage.");
    let mut book_bytes = Vec::new();
    File::open(converted_book_path)?.read_to_end(&mut book_bytes)?;
    let response: ConvertRequestResponse = storage::store_book(book_bytes).await?.into();
    db_lock.insert(body, response.clone(), Duration::from_secs(60 * 60 * 24));
    info!("Returning new response: {:?}", response);
    Ok(response)
}
