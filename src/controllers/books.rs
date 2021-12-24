use crate::diesel::ExpressionMethods;
use crate::{
    connection_pool::PgConnectionManager,
    models::{Book, BookKind, NewBook},
};
use std::convert::Infallible;

use crate::{handlers::ErrorMessage, royalroad::RoyalRoadBook};
use diesel::{QueryDsl, RunQueryDsl};
use mobc::Pool;
use serde::Deserialize;
use tracing::{span, Instrument, Level};
use uuid::Uuid;

use crate::schema::books::dsl::*;

#[derive(Debug, Deserialize)]
pub struct CreateBookRequest {
    url: String,
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
) -> Result<impl warp::Reply, Infallible> {
    let book_id = RoyalRoadBook::royalroad_book_id(&body.url);
    if let Err(err) = book_id {
        return Ok(warp::reply::with_status(
            warp::reply::json(&ErrorMessage {
                message: err.to_string(),
            }),
            warp::http::StatusCode::NOT_FOUND,
        ));
    }
    let conn = db_pool
        .get()
        .instrument(tracing::info_span!("Acquiring a DB Connection."))
        .await;
    if conn.is_err() {
        return Ok(warp::reply::with_status(
            warp::reply::json(&ErrorMessage {
                message: String::from("Failed to get db connection"),
            }),
            warp::http::StatusCode::INTERNAL_SERVER_ERROR,
        ));
    };
    let book_id = book_id.unwrap();
    let conn = conn.unwrap().into_inner();
    let db_check_span = span!(Level::INFO, "Checking if book already exists in db.");
    let existing_book: Result<Book, _> = {
        let _a = db_check_span.enter();
        books
            .filter(metadata.eq(BookKind::RoyalRoad { id: book_id }))
            .first(&conn)
    };
    if let Ok(existing_book) = existing_book {
        return Ok(warp::reply::with_status(
            warp::reply::json(&existing_book),
            warp::http::StatusCode::OK,
        ));
    }
    let book = RoyalRoadBook::from_book_id(book_id).await;
    match book {
        Ok(book) => {
            let db_insert_span = span!(Level::INFO, "Inserting item into DB");
            let db_result: Result<Book, _> = {
                let _a = db_insert_span.enter();
                diesel::insert_into(books)
                    .values::<NewBook>(book.into())
                    .get_result(&conn)
            };
            match db_result {
                Ok(new_book) => Ok(warp::reply::with_status(
                    warp::reply::json(&new_book),
                    warp::http::StatusCode::OK,
                )),
                Err(err) => Ok(warp::reply::with_status(
                    warp::reply::json(&ErrorMessage {
                        message: err.to_string(),
                    }),
                    warp::http::StatusCode::INTERNAL_SERVER_ERROR,
                )),
            }
        }
        Err(err) => Ok(warp::reply::with_status(
            warp::reply::json(&ErrorMessage {
                message: err.to_string(),
            }),
            warp::http::StatusCode::NOT_FOUND,
        )),
    }
}
