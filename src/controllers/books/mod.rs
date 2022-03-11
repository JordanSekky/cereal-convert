mod errors;
use crate::diesel::ExpressionMethods;
use crate::{
    connection_pool::PgConnectionManager,
    models::{Book, BookKind, NewBook},
};
pub use errors::Error;

use crate::util::ErrorMessage;
use crate::{pale, practical_guide, royalroad, wandering_inn};
use diesel::{QueryDsl, RunQueryDsl};
use mobc::Pool;
use serde::{Deserialize, Serialize};
use tracing::{error, span, Instrument, Level};
use uuid::Uuid;
use warp::http::StatusCode;
use warp::{reply, Filter, Reply};

use crate::schema::books::dsl::*;

fn get_book_metadata(url: &str) -> Result<BookKind, Error> {
    if let Ok(x) = royalroad::try_parse_url(url) {
        return Ok(BookKind::RoyalRoad(x));
    }
    if let Ok(()) = pale::try_parse_url(url) {
        return Ok(BookKind::Pale);
    }
    if let Ok(()) = practical_guide::try_parse_url(url) {
        return Ok(BookKind::APracticalGuideToEvil);
    }
    if let Ok(()) = wandering_inn::try_parse_url(url) {
        return Ok(BookKind::TheWanderingInn);
    }
    Err(Error::MetadataParse(
        "Failed to parse url into book metadata.".into(),
    ))
}

#[derive(Debug, Deserialize)]
pub struct CreateBookRequest {
    url: String,
}

#[tracing::instrument(
name = "Get a book.",
err,
level = "info"
skip(db_pool),
fields(
    request_id = %Uuid::new_v4(),
)
)]
pub async fn get_book(book_id: Uuid, db_pool: Pool<PgConnectionManager>) -> Result<Book, Error> {
    let conn = db_pool
        .get()
        .instrument(tracing::info_span!("Acquiring a DB Connection."))
        .await?;
    let conn = conn.into_inner();

    let db_check_span = span!(Level::INFO, "Fetching book from db.");
    let book: Book = {
        let _a = db_check_span.enter();
        books.find(book_id).first(&conn)?
    };
    Ok(book)
}

#[tracing::instrument(
name = "Creating a new book.",
err,
level = "info"
skip(db_pool),
fields(
    request_id = %Uuid::new_v4(),
)
)]
pub async fn create_book(
    db_pool: Pool<PgConnectionManager>,
    body: CreateBookRequest,
) -> Result<Book, Error> {
    let book_kind = get_book_metadata(&body.url)?;
    let conn = db_pool
        .get()
        .instrument(tracing::info_span!("Acquiring a DB Connection."))
        .await?
        .into_inner();
    let db_check_span = span!(Level::INFO, "Checking if book already exists in db.");
    let existing_book: Result<Book, _> = {
        let _a = db_check_span.enter();
        books.filter(metadata.eq(&book_kind)).first(&conn)
    };
    if let Ok(existing_book) = existing_book {
        return Ok(existing_book);
    }
    let book = book_kind.to_new_book().await?;
    let db_insert_span = span!(Level::INFO, "Inserting item into DB");
    let db_result: Book = {
        let _a = db_insert_span.enter();
        diesel::insert_into(books)
            .values::<NewBook>(book)
            .get_result(&conn)?
    };
    Ok(db_result)
}

pub fn get_filters(
    db_pool: Pool<PgConnectionManager>,
) -> impl Filter<Extract = impl Reply, Error = warp::Rejection> + Clone {
    let create_book_db = db_pool.clone();
    let create_book_filter = warp::post()
        .and(warp::path("books"))
        .and(warp::path::end())
        .and(warp::body::content_length_limit(1024))
        .and(warp::any().map(move || create_book_db.clone()))
        .and(warp::body::json())
        .then(create_book)
        .map(map_result);
    let get_book_filter = warp::get()
        .and(warp::path("books"))
        .and(warp::path::param())
        .and(warp::path::end())
        .and(warp::any().map(move || db_pool.clone()))
        .then(get_book)
        .map(map_result);
    create_book_filter.or(get_book_filter)
}

fn map_result(result: Result<impl Serialize, Error>) -> impl Reply {
    match result {
        Ok(x) => reply::with_status(reply::json(&x), StatusCode::OK),
        Err(err) => {
            let internal_server_error: (StatusCode, ErrorMessage) = (
                StatusCode::INTERNAL_SERVER_ERROR,
                "An internal exception occurred.".into(),
            );
            let (status, body) = match err {
                Error::EstablishConnection(_) => internal_server_error,
                Error::QueryResult(_) => internal_server_error,
                Error::RoyalRoad(royalroad::Error::UrlParse(_)) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Provide a valid url.".into(),
                ),
                Error::RoyalRoad(royalroad::Error::Url(_)) => (
                    StatusCode::BAD_REQUEST,
                    "Provide a url to a royalroad book.".into(),
                ),
                Error::RoyalRoad(royalroad::Error::WebParse(_)) => internal_server_error,
                Error::RoyalRoad(royalroad::Error::Reqwest(_)) => internal_server_error,
                Error::RoyalRoad(_) => internal_server_error,
                Error::MetadataParse(_) => internal_server_error,
                Error::GatherBookMetadata(_) => internal_server_error,
            };
            error!(
                "Returning error body: {}, StatusCode: {}, Source: {}",
                serde_json::to_string(&body).expect("Failed to serialize outgoing message body."),
                status,
                err
            );
            reply::with_status(reply::json(&body), status)
        }
    }
}
