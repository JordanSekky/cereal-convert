use diesel::BelongingToDsl;
use diesel::ExpressionMethods;
use diesel::JoinOnDsl;
use diesel::OptionalExtension;
use diesel::QueryDsl;
use diesel::RunQueryDsl;
use itertools::Itertools;
use mobc::Pool;
use std::collections::HashMap;
use std::fmt::Display;
use std::time::Duration;
use tokio::time::MissedTickBehavior;
use tracing::error;
use tracing::info;
use uuid::Uuid;

use crate::models::DeliveryMethod;
use crate::models::NewChapter;
use crate::models::NewUnsentChapter;
use crate::models::UnsentChapter;
use crate::pushover;
use crate::schema::unsent_chapters;
use crate::{
    connection_pool::PgConnectionManager,
    models::{Book, BookKind, Chapter},
    royalroad::{self},
    schema::books,
};

pub async fn check_new_chap_loop(pool: Pool<PgConnectionManager>) -> Result<(), Error> {
    // 5 min check interval for all book.
    let mut interval = tokio::time::interval(Duration::from_secs(5 * 60));
    interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

    loop {
        interval.tick().await;
        match check_and_queue_chapters(pool.clone()).await {
            Ok(_) => {}
            Err(err) => {
                error!(error = ?err, "Error checking for new chapters.")
            }
        }
    }
}

#[tracing::instrument(
name = "Discovering and queueing new chapters.",
err,
level = "info"
skip(pool),
)]
async fn check_and_queue_chapters(pool: Pool<PgConnectionManager>) -> Result<(), Error> {
    info!("Checking for new chapters");
    let conn = pool.get().await?.into_inner();

    let new_chapters: Vec<Chapter> = check_for_new_chapters(pool.clone()).await?;
    if new_chapters.is_empty() {
        info!("No new chapters found.");
        return Ok(());
    }
    let new_book_ids = new_chapters
        .iter()
        .map(|chap| chap.book_id)
        .unique()
        .collect_vec();
    let subscribers = get_subscribers_for_books(new_book_ids, pool.clone()).await?;
    if subscribers.is_empty() {
        info!("No subscribers found for new chapter.");
        return Ok(());
    }
    let chapters_grouped_by_book = new_chapters
        .into_iter()
        .into_group_map_by(|chap| chap.book_id);

    let new_unsent_chapters: Vec<NewUnsentChapter> = chapters_grouped_by_book
        .into_iter()
        .filter_map(|(book_id, chapters)| match subscribers.get(&book_id) {
            Some(book_subs) => Some((chapters, book_subs)),
            None => None,
        })
        .flat_map(|(chapters, book_subs)| {
            book_subs
                .iter()
                .cartesian_product(chapters.iter())
                .map(|(user_id, chapter)| NewUnsentChapter {
                    chapter_id: chapter.id,
                    user_id: user_id.clone(),
                })
                .collect_vec()
        })
        .collect_vec();

    let inserted_unsent_chapters: Vec<UnsentChapter> = {
        use crate::schema::unsent_chapters::dsl::*;
        diesel::insert_into(unsent_chapters)
            .values(&new_unsent_chapters)
            .get_results(&conn)?
    };
    info!(?inserted_unsent_chapters, "Added new unsent chapters.");

    Ok(())
}

#[tracing::instrument(
name = "Discovering new chapters.",
err,
level = "info"
skip(pool),
)]
async fn check_for_new_chapters(pool: Pool<PgConnectionManager>) -> Result<Vec<Chapter>, Error> {
    let conn = pool.get().await?.into_inner();
    let books_to_check: Vec<Book> = books::table.load(&conn)?;
    use crate::schema::chapters::dsl::*;
    let mut new_chapters: Vec<NewChapter> = Vec::new();
    for book in books_to_check {
        match book.metadata {
            BookKind::RoyalRoad { id: check_book_id } => {
                let newest_chapter_publish_time = Chapter::belonging_to(&book)
                    .order_by(published_at.desc())
                    .first::<Chapter>(&conn)
                    .optional()?
                    .map(|x| x.published_at)
                    .unwrap_or(chrono::MIN_DATETIME);
                info!(
                    "Looking for chapters newer than {} for book {:?}",
                    newest_chapter_publish_time, book
                );
                let rss_chapters: Vec<NewChapter> =
                    royalroad::get_chapters(check_book_id, book.id, &book.author)
                        .await
                        .or(Err(Error::NewChapterFetch(
                            "Failed to fetch new royalroad chapters.".into(),
                        )))?;
                new_chapters.append(
                    &mut (rss_chapters
                        .into_iter()
                        .filter(|rss_chap| rss_chap.published_at > newest_chapter_publish_time)
                        .collect()),
                );
            }
        }
    }
    Ok(diesel::insert_into(chapters)
        .values(&new_chapters)
        .get_results(&conn)?)
}

async fn get_subscribers_for_books(
    book_ids: Vec<Uuid>,
    pool: Pool<PgConnectionManager>,
) -> Result<HashMap<Uuid, Vec<String>>, Error> {
    let conn = pool.get().await?.into_inner();
    use crate::schema::subscriptions::dsl::*;
    info!(books = ?book_ids, "Fetching subscribers for books");
    Ok(subscriptions
        .select((book_id, user_id))
        .filter(book_id.eq_any(book_ids))
        .get_results::<(Uuid, String)>(&conn)
        .optional()?
        .unwrap_or_else(|| Vec::with_capacity(0))
        .into_iter()
        .into_group_map())
}

pub async fn send_notifications_loop(pool: Pool<PgConnectionManager>) -> Result<(), Error> {
    let mut interval = tokio::time::interval(Duration::from_secs(5));
    interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let conn = pool.get().await?.into_inner();

    loop {
        interval.tick().await;
        info!("Checking for new unsent chapters.");
        let chaps: Vec<(UnsentChapter, Chapter, Book, DeliveryMethod)> = {
            use crate::schema::chapters;
            use crate::schema::delivery_methods;
            unsent_chapters::table
                .inner_join(chapters::table.on(unsent_chapters::chapter_id.eq(chapters::id)))
                .inner_join(books::table.on(chapters::book_id.eq(books::id)))
                .inner_join(
                    delivery_methods::table
                        .on(unsent_chapters::user_id.eq(delivery_methods::user_id)),
                )
                .load(&conn)
        }?;
        info!("{} unsent chapters found", chaps.len());
        let delete_result = diesel::delete(unsent_chapters::table).execute(&conn);
        match delete_result {
            Ok(_) => info!("Cleared unsent chapters table."),
            Err(x) => {
                error!(?x, "Failed to clear unsent chapters table.",);
                continue;
            }
        }
        for (_, chapter, book, delivery_method) in chaps.iter() {
            if (&delivery_method).pushover_enabled
                && (&delivery_method).pushover_key_verified
                && (&delivery_method).pushover_key.is_some()
            {
                let notification = pushover::send_message(
                    (&delivery_method).pushover_key.clone().unwrap().as_str(),
                    &format!(
                        "A new chapter of {} by {} has been released: {}",
                        book.name, book.author, chapter.name,
                    ),
                )
                .await;
                match notification {
                    Ok(_) => {}
                    Err(x) => error!(
                        ?x,
                        "Failed to deliver notification for chapter {:?} and delivery_method {:?}",
                        chapter,
                        delivery_method
                    ),
                }
            }
        }
    }
}

#[derive(Debug)]
pub enum Error {
    EstablishConnection(mobc::Error<diesel::ConnectionError>),
    QueryResult(diesel::result::Error),
    NewChapterFetch(String),
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{:?}", self))
    }
}

impl std::error::Error for Error {}

impl From<mobc::Error<diesel::ConnectionError>> for Error {
    fn from(x: mobc::Error<diesel::ConnectionError>) -> Self {
        Error::EstablishConnection(x)
    }
}

impl From<diesel::result::Error> for Error {
    fn from(x: diesel::result::Error) -> Self {
        Error::QueryResult(x)
    }
}
