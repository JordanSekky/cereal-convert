use crate::diesel::ExpressionMethods;
use crate::utils::ResponseError;
use crate::{
    connection_pool::PgConnectionManager,
    models::{Book, BookKind, NewBook},
};

use crate::{handlers::ErrorMessage, royalroad::RoyalRoadBook};
use diesel::{QueryDsl, RunQueryDsl};
use mobc::Pool;
use serde::{Deserialize, Serialize};
use tracing::{span, Instrument, Level};
use uuid::Uuid;
use warp::http::StatusCode;
use warp::{reply, Filter, Reply};

use crate::schema::books::dsl::*;

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
pub async fn get_book(
    book_id: Uuid,
    db_pool: Pool<PgConnectionManager>,
) -> Result<Book, ResponseError> {
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
    return Ok(book);
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
) -> Result<Book, ResponseError> {
    let book_id = RoyalRoadBook::royalroad_book_id(&body.url)?;
    let conn = db_pool
        .get()
        .instrument(tracing::info_span!("Acquiring a DB Connection."))
        .await?
        .into_inner();
    let db_check_span = span!(Level::INFO, "Checking if book already exists in db.");
    let existing_book: Result<Book, _> = {
        let _a = db_check_span.enter();
        books
            .filter(metadata.eq(BookKind::RoyalRoad { id: book_id }))
            .first(&conn)
    };
    if let Ok(existing_book) = existing_book {
        return Ok(existing_book);
    }
    let book = RoyalRoadBook::from_book_id(book_id).await?;
    let db_insert_span = span!(Level::INFO, "Inserting item into DB");
    let db_result: Book = {
        let _a = db_insert_span.enter();
        diesel::insert_into(books)
            .values::<NewBook>(book.into())
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

fn map_result(result: Result<impl Serialize, ResponseError>) -> impl Reply {
    match result {
        Ok(x) => reply::with_status(reply::json(&x), StatusCode::OK),
        Err(err) => {
            let (status, body) = match err {
                ResponseError::EstablishConnection(_) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    ErrorMessage {
                        message: String::from("An internal exception occurred."),
                    },
                ),
                ResponseError::QueryResult(_) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    ErrorMessage {
                        message: String::from("An internal exception occurred."),
                    },
                ),
                ResponseError::UrlParseError(_) => (
                    StatusCode::BAD_REQUEST,
                    ErrorMessage {
                        message: String::from("Provide a valid URL."),
                    },
                ),
                ResponseError::ReqwestError(_) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    ErrorMessage {
                        message: String::from("An internal exception occurred."),
                    },
                ),
                ResponseError::RoyalRoadError { message } => {
                    (StatusCode::INTERNAL_SERVER_ERROR, ErrorMessage { message })
                }
                ResponseError::RoyalRoadUrlError { message } => {
                    (StatusCode::BAD_REQUEST, ErrorMessage { message })
                }
            };

            return reply::with_status(reply::json(&body), status);
        }
    }
}
