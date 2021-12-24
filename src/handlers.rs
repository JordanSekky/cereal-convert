use crate::aggregator::get_book_html;
use crate::chapter::BookMeta;
use crate::smtp::send_file_smtp;
use crate::storage::{fetch_book, StorageLocation};
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
use tokio::sync::Mutex;
use tracing::{info, info_span, Instrument};
use ttl_cache::TtlCache;
use uuid::Uuid;

#[derive(Serialize)]
pub struct ErrorMessage {
    pub message: String,
}

impl ErrorMessage {
    fn from(error: &Box<dyn Error>) -> ErrorMessage {
        ErrorMessage {
            message: error.as_ref().to_string().clone(),
        }
    }
}

#[derive(Deserialize, Serialize, PartialEq, Eq, Hash, Clone, Debug)]
pub struct ConvertRequestBody {
    chapters: BTreeSet<u64>,
}

#[derive(Serialize, Clone, Debug)]
pub struct ConvertRequestResponse {
    pub key: String,
    pub bucket: String,
    pub book: BookMeta,
}

#[tracing::instrument(
    name = "Caching a new RoyalRoad set of chapters",
    err,
    level = "info"
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
            warp::reply::json(&ErrorMessage::from(&err)),
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
        let location = storage::store_book(book_bytes)
            .instrument(info_span!("Storing mobi to cloud storage."))
            .await?;
        ConvertRequestResponse {
            key: location.key,
            bucket: location.bucket,
            book: aggregate.into(),
        }
    };
    db_lock.insert(body, response.clone(), Duration::from_secs(60 * 60 * 24));
    info!("Returning new response: {:?}", response);
    Ok(response)
}

#[derive(Deserialize, Serialize, PartialEq, Eq, Hash, Clone, Debug)]
pub struct MailRequestBody {
    key: String,
    bucket: String,
    book: BookMeta,
    email: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct MailRequestResponse;

#[tracing::instrument(
    name = "Mailing a set of chapters to a customer.",
    err,
    level = "info"
    skip(db),
    fields(
        request_id = %Uuid::new_v4(),
    )
)]
pub async fn smtp_handler(
    db: Arc<Mutex<TtlCache<MailRequestBody, Vec<u8>>>>,
    body: MailRequestBody,
) -> Result<impl warp::Reply, Infallible> {
    match fetch_and_mail_book(db, body).await {
        Ok(output) => Ok(warp::reply::with_status(
            warp::reply::json(&output),
            warp::http::StatusCode::OK,
        )),
        Err(err) => Ok(warp::reply::with_status(
            warp::reply::json(&ErrorMessage::from(&err)),
            warp::http::StatusCode::INTERNAL_SERVER_ERROR,
        )),
    }
}

async fn fetch_and_mail_book(
    db: Arc<Mutex<TtlCache<MailRequestBody, Vec<u8>>>>,
    body: MailRequestBody,
) -> Result<MailRequestResponse, Box<dyn Error>> {
    info!("Received request: {:?}", body);
    let db_lock = db
        .as_ref()
        .lock()
        .instrument(info_span!("Waiting for db lock."))
        .await;
    let bytes = match db_lock.get(&body) {
        Some(b) => b.clone(),
        None => {
            fetch_book(&StorageLocation {
                key: body.key,
                bucket: body.bucket,
            })
            .instrument(info_span!(
                "Cache miss. Fetching book bytes from cloud storage."
            ))
            .await?
        }
    };
    send_file_smtp(bytes, &body.email, &body.book)
        .instrument(info_span!("Sending email."))
        .await?;
    Ok(MailRequestResponse {})
}
