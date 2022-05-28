use anyhow::anyhow;
use anyhow::Context;
use anyhow::Error;
use anyhow::Result;
use diesel::sql_query;
use diesel::BelongingToDsl;
use diesel::ExpressionMethods;
use diesel::JoinOnDsl;
use diesel::QueryDsl;
use diesel::RunQueryDsl;
use futures::future::join_all;
use itertools::Itertools;
use rusoto_s3::S3Location;
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::MissedTickBehavior;
use tracing::error;
use tracing::info;
use uuid::Uuid;

use crate::clients::calibre;
use crate::clients::mailgun;
use crate::clients::pushover;
use crate::models::ChapterBody;
use crate::models::ChapterKind;
use crate::models::ChapterWithUser;
use crate::models::DeliveryMethod;
use crate::models::NewChapter;
use crate::providers::pale;
use crate::providers::practical_guide;
use crate::providers::royalroad;
use crate::providers::royalroad::RoyalRoadBookKind;
use crate::providers::the_daily_grind_patreon;
use crate::providers::wandering_inn;
use crate::providers::wandering_inn_patreon;
use crate::schema::chapter_bodies;
use crate::schema::chapters;
use crate::schema::delivery_methods;
use crate::storage;
use crate::util::InstrumentedPgConnectionPool;
use crate::util::ResultExt;
use crate::{
    models::{Book, BookKind, Chapter},
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
                error!(error = ?err, "Error checking for new chapters.");
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
    let _book_chaps_subs = check_for_all_new_chapters(pool).await?;

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
) -> Result<(Book, Vec<Chapter>)> {
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
    Ok((book, chaps))
}

#[tracing::instrument(
name = "Discovering new chapters.",
err,
level = "info"
skip(pool),
)]
async fn check_for_all_new_chapters(
    pool: &InstrumentedPgConnectionPool,
) -> Result<Vec<(Book, Vec<Chapter>)>, Error> {
    // Fetch only books which have subscribers.
    let books = {
        use crate::schema::subscriptions;
        let conn = pool.get().await?;
        books::table
            .inner_join(
                subscriptions::table.on(subscriptions::columns::book_id.eq(books::columns::id)),
            )
            .select(books::all_columns)
            .load::<Book>(&*conn)?
    };

    let book_chaps = join_all(
        books
            .into_iter()
            .map(|book| check_for_new_chapters(pool.clone(), book)),
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

    Ok(book_chaps)
}

#[tracing::instrument(name = "Fetching a new chapter body.", err, level = "info")]
async fn fetch_chapter_body(chapter: &NewChapter, book: &Book) -> Result<String> {
    match &chapter.metadata {
        ChapterKind::RoyalRoad { id } => royalroad::get_chapter_body(id, book, chapter).await,
        ChapterKind::Pale { url } => pale::get_chapter_body(url, book, chapter).await,
        ChapterKind::APracticalGuideToEvil { url } => {
            practical_guide::get_chapter_body(url, book, chapter).await
        }
        ChapterKind::TheWanderingInn { url } => {
            wandering_inn::get_chapter_body(url, book, chapter).await
        }
        ChapterKind::TheWanderingInnPatreon { url, password } => {
            wandering_inn_patreon::get_chapter_body(url, password.as_deref(), book, chapter).await
        }
        ChapterKind::TheDailyGrindPatreon { html } => {
            Ok(format!("<h1>{}: {}</h1>{}", book.name, chapter.name, html))
        }
    }
}

#[tracing::instrument(name = "Fetching all new chapter bodies.", level = "info")]
async fn fetch_chapter_bodies(chapters: &[NewChapter], book: &Book) -> Vec<Result<S3Location>> {
    // Fetch all bodies as strings from the web.
    let bodies_and_chapters: Vec<(String, &NewChapter)> =
        join_all(chapters.iter().map(|chap| fetch_chapter_body(chap, book)))
            .await
            .into_iter()
            .zip(chapters.iter())
            .filter_map(|(x, chap)| match x {
                Ok(x) => Some((x, chap)),
                Err(_err) => None,
            })
            .collect();
    // Store all chapters in S3.
    let locations = join_all(
        bodies_and_chapters
            .iter()
            .map(|(body, _chapter)| storage::store_book(body.as_bytes())),
    )
    .await;
    locations
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
        BookKind::TheWanderingInnPatreon => wandering_inn_patreon::get_chapters(&book.id)
            .await
            .with_context(|| "Failed to fetch new wandering inn patreon chapters.")?,
        BookKind::TheDailyGrindPatreon => the_daily_grind_patreon::get_chapters(&book.id)
            .await
            .with_context(|| "Failed to fetch new daily grind patreon chapters.")?,
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
    let mut interval = tokio::time::interval(Duration::from_secs(30));
    interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

    loop {
        interval.tick().await;
        match send_notifications(pool.clone()).await {
            Ok(_) => {}
            Err(err) => error!({%err}, "An error occurred sending notifications."),
        };
    }
}

#[tracing::instrument(
name = "Delivering any unsent chapters",
err,
level = "info"
skip(pool),
)]
async fn send_notifications(pool: InstrumentedPgConnectionPool) -> Result<()> {
    info!("Checking for new unsent chapters.");

    let chaps: Vec<ChapterWithUser> = {
        let conn = pool.get().await?;
        let chapters_query = "
            select subs_with_timestamp.user_id, subs_with_timestamp.grouping_quantity, chapters.* from (
                select subscriptions.*, coalesce(max(chapters.published_at), TIMESTAMP '1982-05-20 22:06:05.944623+00') as last_chapter_timestamp from subscriptions
                left join chapters on chapters.id = last_chapter_id
                group by subscriptions.user_id, subscriptions.book_id) as subs_with_timestamp
            left join books on books.id = subs_with_timestamp.book_id
            left join chapters on chapters.book_id = books.id
            where chapters.published_at > subs_with_timestamp.last_chapter_timestamp
            ";
        sql_query(chapters_query).load(&*conn)?
    };

    let mut user_id_to_book_ids_to_chapters: HashMap<String, HashMap<(Uuid, i64), Vec<Chapter>>> =
        HashMap::new();
    for chap in chaps {
        let book_ids_to_chapters = user_id_to_book_ids_to_chapters
            .entry(chap.user_id)
            .or_default();
        let chap_list = book_ids_to_chapters
            .entry((chap.book_id, chap.grouping_quantity))
            .or_default();
        let new_chap = Chapter {
            id: chap.id,
            name: chap.name,
            author: chap.author,
            created_at: chap.created_at,
            published_at: chap.published_at,
            book_id: chap.book_id,
            updated_at: chap.updated_at,
            metadata: chap.metadata,
        };
        match chap_list.binary_search_by(|a| a.published_at.cmp(&new_chap.published_at)) {
            Ok(_pos) => {} // element already in vector @ `pos`
            Err(pos) => chap_list.insert(pos, new_chap),
        }
    }

    let user_ids = user_id_to_book_ids_to_chapters.keys().collect_vec();
    let book_ids = user_id_to_book_ids_to_chapters
        .values()
        .flat_map(|x| x.keys().map(|(book_id, _group_size)| book_id))
        .collect_vec();

    let user_to_delivery_method: HashMap<String, DeliveryMethod> = {
        let conn = pool.get().await?;
        delivery_methods::table
            .select(delivery_methods::all_columns)
            .filter(delivery_methods::user_id.eq_any(user_ids))
            .load::<DeliveryMethod>(&*conn)?
            .into_iter()
            .map(|x| (x.user_id.clone(), x))
            .collect()
    };

    let book_id_to_book: HashMap<Uuid, Book> = {
        let conn = pool.get().await?;
        books::table
            .select(books::all_columns)
            .filter(books::id.eq_any(book_ids))
            .load::<Book>(&*conn)?
            .into_iter()
            .map(|x| (x.id, x))
            .collect()
    };

    let delivery_errors = deliver_new_chapters(
        user_id_to_book_ids_to_chapters,
        user_to_delivery_method,
        book_id_to_book,
        pool.clone(),
    )
    .await;

    match delivery_errors.len() {
        0 => Ok(()),
        _len => Err(anyhow!("Failed to deliver some chapters to users"))
            .with_context(|| format!("{delivery_errors:#?}")),
    }
}

async fn deliver_new_chapters(
    user_id_to_book_ids_to_chapters: HashMap<String, HashMap<(Uuid, i64), Vec<Chapter>>>,
    user_to_delivery_method: HashMap<String, DeliveryMethod>,
    book_id_to_book: HashMap<Uuid, Book>,
    pool: InstrumentedPgConnectionPool,
) -> Vec<Result<()>> {
    let mut errors = Vec::new();
    for (user_id, book_id_to_chapters) in user_id_to_book_ids_to_chapters {
        let delivery_method = user_to_delivery_method.get(&user_id).unwrap();
        for ((book_id, grouping_quantity), chapters) in book_id_to_chapters {
            let book = book_id_to_book.get(&book_id).unwrap();

            let chapter_bodies: Vec<ChapterBody> = {
                let conn = match pool.get().await {
                    Ok(x) => x,
                    Err(e) => {
                        errors.push(Err(e).with_context(|| {
                            format!(
                                "Failed to acquire a database connection
                         while fetching bodies for book {}, chapters: [{}]",
                                book.name,
                                chapters.iter().map(|chap| &chap.name).join(", ")
                            )
                        }));
                        continue;
                    }
                };
                match chapter_bodies::table
                    .filter(
                        chapter_bodies::chapter_id
                            .eq_any(chapters.iter().map(|x| x.id).collect_vec()),
                    )
                    .select(chapter_bodies::all_columns)
                    .order(chapter_bodies::chapter_id.asc())
                    .load(&*conn)
                {
                    Ok(x) => x,
                    Err(e) => {
                        errors.push(Err(e).with_context(|| {
                            format!(
                                "Failed to fetch bodies for book {}, chapters: [{}]",
                                book.name,
                                chapters.iter().map(|chap| &chap.name).join(", ")
                            )
                        }));
                        continue;
                    }
                }
            };

            let chapters_with_body = chapters
                .iter()
                .zip(chapter_bodies.iter())
                .sorted_by_key(|(chap, _body)| chap.published_at)
                .collect_vec();
            if chapters_with_body.len() as i64 >= grouping_quantity {
                match send_pushover_if_enabled(delivery_method, book, &chapters).await {
                    Ok(()) => (),
                    Err(e) => {
                        errors.push(Err(e).with_context(|| {
                            format!(
                                "Failed to pushover notification to user {user_id} for book {}, chapters: [{}]",
                                book.name,
                                chapters.iter().map(|chap| &chap.name).join(", ")
                            )
                        }));
                        continue;
                    }
                };
                match send_kindle_if_enabled(delivery_method, book, &chapters_with_body).await {
                    Ok(()) => (),
                    Err(e) => {
                        errors.push(Err(e).with_context(|| {
                            format!(
                                "Failed to send kindle emails for user {user_id} for book {}, chapters: [{}]",
                                book.name,
                                chapters.iter().map(|chap| &chap.name).join(", ")
                            )
                        }));
                        continue;
                    }
                };
                match update_subscription_last_chapter_id(pool.clone(), &user_id, &chapters).await {
                    Ok(()) => (),
                    Err(e) => {
                        errors.push(Err(e).with_context(|| {
                            format!(
                                "Failed to update last_sent_chapter for user {user_id} for book {}, chapters: [{}]",
                                book.name,
                                chapters.iter().map(|chap| &chap.name).join(", ")
                            )
                        }));
                        continue;
                    }
                };
            }
        }
    }
    errors
}

async fn update_subscription_last_chapter_id(
    pool: InstrumentedPgConnectionPool,
    user_id_str: &str,
    chapters: &[Chapter],
) -> Result<()> {
    use crate::schema::subscriptions::dsl::*;
    let conn = pool.get().await?;
    diesel::update(
        subscriptions
            .filter(user_id.eq(user_id_str))
            .filter(book_id.eq(chapters[0].book_id)),
    )
    .set(last_chapter_id.eq(chapters[chapters.len() - 1].id))
    .execute(&*conn)?;
    Ok(())
}

async fn send_pushover_if_enabled(
    delivery_method: &DeliveryMethod,
    book: &Book,
    chapters: &[Chapter],
) -> Result<()> {
    if let Some(pushover_key) = delivery_method.get_pushover_key() {
        match chapters.len() {
            1 => {
                pushover::send_message(
                    pushover_key,
                    &format!(
                        "A new chapter of {} by {} has been released: {}",
                        book.name, book.author, chapters[0].name,
                    ),
                )
                .await?
            }
            x => {
                pushover::send_message(
                    pushover_key,
                    &format!(
                        "{x} new chapters of {} by {} has been released: {} through {}",
                        book.name,
                        book.author,
                        chapters[0].name,
                        chapters[x - 1].name,
                    ),
                )
                .await?
            }
        };
    }
    Ok(())
}

async fn send_kindle_if_enabled(
    delivery_method: &DeliveryMethod,
    book: &Book,
    chapters: &[(&Chapter, &ChapterBody)],
) -> Result<()> {
    let text_bytes: Vec<u8> = join_all(
        chapters
            .iter()
            .map(|(_chap, body)| S3Location {
                prefix: (*body).key.clone(),
                bucket_name: (*body).bucket.clone(),
                ..Default::default()
            })
            .map(storage::fetch_book),
    )
    .await
    .into_iter()
    .collect::<Result<Vec<Vec<u8>>>>()?
    .into_iter()
    .fold(Vec::new(), |mut acc, mut x| {
        acc.append(&mut x);
        acc
    });
    let just_chapters = chapters.iter().map(|(c, _b)| *c).collect_vec();
    let cover_title = match just_chapters.len() {
        1 => format!("{}: {}", book.name, just_chapters[0].name),
        x => format!(
            "{}: {} through {}",
            book.name,
            just_chapters[0].name,
            just_chapters[x - 1].name
        ),
    };
    let mobi_bytes = calibre::generate_mobi(
        ".html",
        &String::from_utf8(text_bytes)?,
        &cover_title,
        &book.name,
        &book.author,
    )
    .await?;
    if let Some(kindle_email) = delivery_method.get_kindle_email() {
        send_kindle(kindle_email, book, &just_chapters, &mobi_bytes).await?;
    }
    Ok(())
}

async fn send_kindle(
    kindle_email: &str,
    book: &Book,
    chapters: &[&Chapter],
    bytes: &[u8],
) -> Result<(), Error> {
    match chapters.len() {
        1 => {
            let subject = format!("New Chapter of {}: {}", book.name, chapters[0].name);
            mailgun::send_mobi_file(bytes, kindle_email, &chapters[0].name, &subject).await?;
        }
        x => {
            let subject = format!(
                "{x} New Chapters of {}: {} through {}",
                book.name,
                chapters[0].name,
                chapters[x - 1].name
            );
            mailgun::send_mobi_file(
                bytes,
                kindle_email,
                &format!("{} through {}", chapters[0].name, chapters[x - 1].name),
                &subject,
            )
            .await?;
        }
    }
    Ok(())
}
