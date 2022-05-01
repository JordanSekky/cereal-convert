use crate::diesel::ExpressionMethods;
use crate::models::{Book, BookKind, NewBook};
use crate::util::{map_result, InstrumentedPgConnectionPool};

use crate::{pale, practical_guide, royalroad, wandering_inn};
use anyhow::{bail, Result};
use diesel::{QueryDsl, RunQueryDsl};
use serde::Deserialize;
use uuid::Uuid;
use warp::{Filter, Reply};

use crate::schema::books::dsl::*;

fn get_book_metadata(url: &str) -> Result<BookKind> {
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
    bail!("Failed to parse url {} into book metadata", url);
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
pub async fn get_book(book_id: Uuid, db_pool: InstrumentedPgConnectionPool) -> Result<Book> {
    let conn = db_pool.get().await?;

    let book: Book = books.find(book_id).first(&*conn)?;
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
    db_pool: InstrumentedPgConnectionPool,
    body: CreateBookRequest,
) -> Result<Book> {
    let book_kind = get_book_metadata(&body.url)?;
    let conn = db_pool.get().await?;
    let existing_book: Result<Book, _> = books.filter(metadata.eq(&book_kind)).first(&*conn);
    if let Ok(existing_book) = existing_book {
        return Ok(existing_book);
    }
    let book = book_kind.to_new_book().await?;
    let db_result: Book = diesel::insert_into(books)
        .values::<NewBook>(book)
        .get_result(&*conn)?;
    Ok(db_result)
}

pub fn get_filters(
    db_pool: &InstrumentedPgConnectionPool,
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
    let get_book_db = db_pool.clone();
    let get_book_filter = warp::get()
        .and(warp::path("books"))
        .and(warp::path::param())
        .and(warp::path::end())
        .and(warp::any().map(move || get_book_db.clone()))
        .then(get_book)
        .map(map_result);
    create_book_filter.or(get_book_filter)
}
