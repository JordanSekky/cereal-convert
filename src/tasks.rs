use derive_more::Display;
use derive_more::Error;
use derive_more::From;
use diesel::BelongingToDsl;
use diesel::ExpressionMethods;
use diesel::JoinOnDsl;
use diesel::OptionalExtension;
use diesel::QueryDsl;
use diesel::RunQueryDsl;
use itertools::Itertools;
use mobc::Pool;
use std::time::Duration;
use tokio::time::MissedTickBehavior;
use tracing::error;
use tracing::info;

use crate::calibre;
use crate::mailgun;
use crate::models::ChapterBody;
use crate::models::ChapterKind;
use crate::models::DeliveryMethod;
use crate::models::NewChapter;
use crate::models::NewUnsentChapter;
use crate::models::Subscription;
use crate::models::UnsentChapter;
use crate::pushover;
use crate::schema::chapter_bodies;
use crate::schema::chapters;
use crate::schema::delivery_methods;
use crate::schema::subscriptions;
use crate::schema::unsent_chapters;
use crate::storage;
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
    let book_chaps_subs = check_for_new_chapters(pool.clone()).await?;

    let new_unsent_chapters: Vec<NewUnsentChapter> = book_chaps_subs
        .into_iter()
        .flat_map(|(book, chapters, subs)| {
            info!("Queueing new chapter notifications for book {:?}", book);
            subs.iter()
                .cartesian_product(chapters.iter())
                .map(|(sub, chapter)| NewUnsentChapter {
                    chapter_id: chapter.id,
                    user_id: sub.user_id.clone(),
                })
                .collect_vec()
        })
        .collect_vec();

    let inserted_unsent_chapters: Vec<UnsentChapter> = {
        use crate::schema::unsent_chapters::dsl::*;
        let conn = pool.get().await?.into_inner();
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
async fn check_for_new_chapters(
    pool: Pool<PgConnectionManager>,
) -> Result<Vec<(Book, Vec<Chapter>, Vec<Subscription>)>, Error> {
    // Fetch only books which have subscribers.
    let subscriptions = {
        let conn = pool.get().await?.into_inner();
        books::table
            .inner_join(
                subscriptions::table.on(subscriptions::columns::book_id.eq(books::columns::id)),
            )
            .load::<(Book, Subscription)>(&conn)?
            .into_iter()
            .into_group_map()
    };

    let mut book_chaps_subs = Vec::with_capacity(subscriptions.len());
    for (book, subs) in subscriptions.into_iter() {
        let chaps = get_new_chapters(&book, pool.clone()).await?;
        let chaps: Vec<Chapter> = {
            let conn = pool.get().await?.into_inner();
            diesel::insert_into(chapters::table)
                .values(chaps)
                .get_results(&conn)?
        };
        let locations = fetch_chapter_bodies(&chaps, &book).await?;
        {
            let conn = pool.get().await?.into_inner();
            diesel::insert_into(chapter_bodies::table)
                .values(&locations)
                .execute(&conn)?;
        }
        book_chaps_subs.insert(book_chaps_subs.len(), (book, chaps, subs));
    }

    Ok(book_chaps_subs)
}

async fn fetch_chapter_bodies(
    chapters: &[Chapter],
    book: &Book,
) -> Result<Vec<ChapterBody>, Error> {
    let mut out = Vec::with_capacity(chapters.len());
    for chapter in chapters {
        match chapter.metadata {
            ChapterKind::RoyalRoad { id } => {
                let body = royalroad::get_chapter_body(&id).await?;
                let title = format!("{}: {}", book.name, chapter.name);
                let mobi_bytes =
                    calibre::generate_mobi(".html", &body, &title, &title, &book.author).await?;
                let location = storage::store_book(mobi_bytes, &chapter.id).await?;
                out.insert(out.len(), location);
            }
        }
    }
    Ok(out)
}

async fn get_new_chapters(
    book: &Book,
    pool: Pool<PgConnectionManager>,
) -> Result<Vec<NewChapter>, Error> {
    match book.metadata {
        BookKind::RoyalRoad { id: check_book_id } => {
            use crate::schema::chapters::dsl::*;
            let newest_chapter_publish_time = {
                let conn = pool.get().await?.into_inner();
                Chapter::belonging_to(book)
                    .order_by(published_at.desc())
                    .first::<Chapter>(&conn)
                    .optional()?
                    .map(|x| x.published_at)
                    .unwrap_or(chrono::MIN_DATETIME)
            };
            info!(
                "Looking for chapters newer than {} for book {:?}",
                newest_chapter_publish_time, book
            );
            let rss_chapters: Vec<NewChapter> =
                royalroad::get_chapters(check_book_id, &book.id, &book.author)
                    .await
                    .or(Err(Error::NewChapterFetch(
                        "Failed to fetch new royalroad chapters.".into(),
                    )))?;

            Ok(rss_chapters
                .into_iter()
                .filter(|rss_chap| rss_chap.published_at > newest_chapter_publish_time)
                .collect())
        }
    }
}

pub async fn send_notifications_loop(pool: Pool<PgConnectionManager>) -> Result<(), Error> {
    let mut interval = tokio::time::interval(Duration::from_secs(5));
    interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let conn = pool.get().await?.into_inner();

    loop {
        interval.tick().await;
        info!("Checking for new unsent chapters.");
        let chaps: Vec<(UnsentChapter, (Chapter, ChapterBody), Book, DeliveryMethod)> = {
            unsent_chapters::table
                .inner_join(
                    chapters::table
                        .inner_join(chapter_bodies::table)
                        .on(unsent_chapters::chapter_id.eq(chapters::id)),
                )
                .inner_join(books::table.on(chapters::book_id.eq(books::id)))
                .inner_join(
                    delivery_methods::table
                        .on(unsent_chapters::user_id.eq(delivery_methods::user_id)),
                )
                .load(&conn)
        }?;
        if chaps.is_empty() {
            continue;
        }
        info!("{} unsent chapters found", chaps.len());
        let delete_result = diesel::delete(unsent_chapters::table).execute(&conn);
        match delete_result {
            Ok(_) => info!("Cleared unsent chapters table."),
            Err(x) => {
                error!(?x, "Failed to clear unsent chapters table.",);
                continue;
            }
        }
        let grouped_by_chapter = chaps.iter().into_group_map_by(|x| &x.1);
        for ((chapter, chapter_body), group) in grouped_by_chapter {
            let bytes = storage::fetch_book(chapter_body).await?;
            for (_, _, book, delivery_method) in group {
                send_pushover_if_enabled(delivery_method, book, chapter).await;
                send_kindle_if_enabled(delivery_method, book, chapter, &bytes).await;
            }
        }
    }
}

async fn send_pushover_if_enabled(
    delivery_method: &DeliveryMethod,
    book: &Book,
    chapter: &Chapter,
) {
    if let Some(pushover_key) = delivery_method.get_pushover_key() {
        let notification = pushover::send_message(
            pushover_key,
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

async fn send_kindle_if_enabled(
    delivery_method: &DeliveryMethod,
    book: &Book,
    chapter: &Chapter,
    bytes: &[u8],
) {
    if let Some(kindle_email) = delivery_method.get_kindle_email() {
        let notification = send_kindle(kindle_email, book, chapter, bytes).await;
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

async fn send_kindle(
    kindle_email: &str,
    book: &Book,
    chapter: &Chapter,
    bytes: &[u8],
) -> Result<(), Error> {
    let subject = format!("New Chapter of {}: {}", book.name, chapter.name);
    mailgun::send_mobi_file(&bytes, kindle_email, &chapter.name, &subject).await?;
    Ok(())
}

#[derive(Debug, Display, From, Error)]
#[display(fmt = "Tasks Error: {}")]
pub enum Error {
    #[display(fmt = "EstablishConnection: {}", "_0")]
    EstablishConnection(mobc::Error<diesel::ConnectionError>),
    #[display(fmt = "QueryResult: {}", "_0")]
    QueryResult(diesel::result::Error),
    #[display(fmt = "NewChapterFetch: {}", "_0")]
    NewChapterFetch(#[error(not(source))] String),
    #[display(fmt = "RoyalRoad: {}", "_0")]
    RoyalRoad(royalroad::Error),
    #[display(fmt = "Calibre: {}", "_0")]
    Calibre(calibre::Error),
    #[display(fmt = "Mailgun: {}", "_0")]
    Mailgun(mailgun::Error),
    #[display(fmt = "Storage: {}", "_0")]
    Storage(storage::Error),
}
