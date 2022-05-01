use anyhow::Context;
use anyhow::Error;
use anyhow::Result;
use diesel::BelongingToDsl;
use diesel::ExpressionMethods;
use diesel::JoinOnDsl;
use diesel::QueryDsl;
use diesel::RunQueryDsl;
use futures::future::join_all;
use itertools::Itertools;
use rusoto_s3::S3Location;
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
use crate::pale;
use crate::practical_guide;
use crate::pushover;
use crate::royalroad::RoyalRoadBookKind;
use crate::schema::chapter_bodies;
use crate::schema::chapters;
use crate::schema::delivery_methods;
use crate::schema::subscriptions;
use crate::schema::unsent_chapters;
use crate::storage;
use crate::util::InstrumentedPgConnectionPool;
use crate::util::ResultExt;
use crate::wandering_inn;
use crate::{
    models::{Book, BookKind, Chapter},
    royalroad::{self},
    schema::books,
};

pub async fn check_new_chap_loop(pool: InstrumentedPgConnectionPool) -> Result<(), Error> {
    // 5 min check interval for all book.
    let mut interval = tokio::time::interval(Duration::from_secs(5 * 60));
    interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

    loop {
        interval.tick().await;
        match check_and_queue_chapters(&pool).await {
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
async fn check_and_queue_chapters(pool: &InstrumentedPgConnectionPool) -> Result<(), Error> {
    info!("Checking for new chapters");
    let book_chaps_subs = check_for_all_new_chapters(pool).await?;

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
        let conn = pool.get().await?;
        diesel::insert_into(unsent_chapters)
            .values(&new_unsent_chapters)
            .get_results(&*conn)?
    };
    info!(?inserted_unsent_chapters, "Added new unsent chapters.");

    Ok(())
}

#[tracing::instrument(
name = "Discovering new chapters for a single book.",
err,
level = "info"
skip(pool),
)]
async fn check_for_new_chapters(
    pool: InstrumentedPgConnectionPool,
    book: Book,
    subs: Vec<Subscription>,
) -> Result<(Book, Vec<Chapter>, Vec<Subscription>)> {
    let chaps = get_new_chapters(&book, &pool)
        .await
        .unwrap_or_else_log(|| Vec::with_capacity(0));
    let locations = fetch_chapter_bodies(&chaps, &book).await;
    let (chaps, locations): (Vec<_>, Vec<_>) = chaps
        .into_iter()
        .zip(locations.into_iter())
        .filter_map(|(chap, loc)| match loc {
            Ok(loc) => Some((chap, loc)),
            Err(err) => {
                tracing::error!(?err);
                None
            }
        })
        .unzip();
    let chaps: Vec<Chapter> = {
        let conn = pool.get().await?;
        diesel::insert_into(chapters::table)
            .values(chaps)
            .get_results(&*conn)?
    };
    {
        let bodies = chaps
            .iter()
            .zip(locations.iter())
            .map(|(chap, location)| ChapterBody {
                key: location.prefix.clone(),
                bucket: location.bucket_name.clone(),
                chapter_id: chap.id,
            })
            .collect_vec();
        let conn = pool.get().await?;
        diesel::insert_into(chapter_bodies::table)
            .values(&bodies)
            .execute(&*conn)?;
    }
    Ok((book, chaps, subs))
}

#[tracing::instrument(
name = "Discovering new chapters.",
err,
level = "info"
skip(pool),
)]
async fn check_for_all_new_chapters(
    pool: &InstrumentedPgConnectionPool,
) -> Result<Vec<(Book, Vec<Chapter>, Vec<Subscription>)>, Error> {
    // Fetch only books which have subscribers.
    let subscriptions = {
        let conn = pool.get().await?;
        books::table
            .inner_join(
                subscriptions::table.on(subscriptions::columns::book_id.eq(books::columns::id)),
            )
            .load::<(Book, Subscription)>(&*conn)?
            .into_iter()
            .into_group_map()
    };

    let book_chaps_subs = join_all(
        subscriptions
            .into_iter()
            .map(|(book, subs)| check_for_new_chapters(pool.clone(), book, subs)),
    )
    .await
    .into_iter()
    .filter_map(|x| match x {
        Ok(x) => Some(x),
        Err(err) => {
            tracing::error!(?err);
            None
        }
    })
    .collect();

    Ok(book_chaps_subs)
}

#[tracing::instrument(name = "Fetching a new chapter body.", err, level = "info")]
async fn fetch_chapter_body(chapter: &NewChapter, book: &Book) -> Result<S3Location> {
    let body = match &chapter.metadata {
        ChapterKind::RoyalRoad { id } => royalroad::get_chapter_body(id).await,
        ChapterKind::Pale { url } => pale::get_chapter_body(url).await,
        ChapterKind::APracticalGuideToEvil { url } => practical_guide::get_chapter_body(url).await,
        ChapterKind::TheWanderingInn { url } => wandering_inn::get_chapter_body(url).await,
    }?;
    let title = format!("{}: {}", book.name, chapter.name);
    let mobi_bytes = calibre::generate_mobi(".html", &body, &title, &title, &book.author).await?;
    storage::store_book(mobi_bytes).await
}

#[tracing::instrument(name = "Fetching all new chapter bodies.", level = "info")]
async fn fetch_chapter_bodies(chapters: &[NewChapter], book: &Book) -> Vec<Result<S3Location>> {
    return join_all(chapters.iter().map(|chap| fetch_chapter_body(chap, book))).await;
}

#[tracing::instrument(
name = "Discovering new chapters for a single book.",
err,
level = "info"
skip(pool),
)]
async fn get_new_chapters(
    book: &Book,
    pool: &InstrumentedPgConnectionPool,
) -> Result<Vec<NewChapter>, Error> {
    let rss_chapters = match book.metadata {
        BookKind::RoyalRoad(RoyalRoadBookKind { id }) => {
            royalroad::get_chapters(id, &book.id, &book.author)
                .await
                .with_context(|| "Failed to fetch new royalroad chapters.")?
        }
        BookKind::Pale => pale::get_chapters(&book.id)
            .await
            .with_context(|| "Failed to fetch new pale chapters.")?,
        BookKind::APracticalGuideToEvil => practical_guide::get_chapters(&book.id)
            .await
            .with_context(|| "Failed to fetch new practical guide to evil chapters.")?,
        BookKind::TheWanderingInn => wandering_inn::get_chapters(&book.id)
            .await
            .with_context(|| "Failed to fetch new practical guide to evil chapters.")?,
    };
    if rss_chapters.is_empty() {
        return Ok(rss_chapters);
    }
    let oldest_rss_chapter = rss_chapters
        .iter()
        .min_by(|x, y| x.published_at.cmp(&y.published_at))
        .unwrap();
    let existing_chapters = {
        use crate::schema::chapters::dsl::*;
        let conn = pool.get().await?;
        Chapter::belonging_to(book)
            .filter(published_at.ge(oldest_rss_chapter.published_at))
            .order_by(published_at.desc())
            .load::<Chapter>(&*conn)?
    }
    .into_iter()
    .map(|chap| chap.metadata)
    .collect_vec();

    Ok(rss_chapters
        .into_iter()
        .filter(|rss_chap| !existing_chapters.contains(&rss_chap.metadata))
        .collect())
}

pub async fn send_notifications_loop(pool: InstrumentedPgConnectionPool) -> Result<(), Error> {
    let mut interval = tokio::time::interval(Duration::from_secs(5));
    interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

    loop {
        interval.tick().await;
        info!("Checking for new unsent chapters.");
        let chaps: Vec<(UnsentChapter, (Chapter, ChapterBody), Book, DeliveryMethod)> = {
            let conn = pool.get().await?;
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
                .load(&*conn)
        }?;
        if chaps.is_empty() {
            continue;
        }
        info!("{} unsent chapters found", chaps.len());
        let delete_result = {
            let conn = pool.get().await?;
            diesel::delete(unsent_chapters::table).execute(&*conn)
        };
        match delete_result {
            Ok(_) => info!("Cleared unsent chapters table."),
            Err(x) => {
                error!(?x, "Failed to clear unsent chapters table.",);
                continue;
            }
        }
        let grouped_by_chapter = chaps.iter().into_group_map_by(|x| &x.1);
        for ((chapter, chapter_body), group) in grouped_by_chapter {
            let bytes = storage::fetch_book(&chapter_body.clone().into()).await?;
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
    mailgun::send_mobi_file(bytes, kindle_email, &chapter.name, &subject).await?;
    Ok(())
}
