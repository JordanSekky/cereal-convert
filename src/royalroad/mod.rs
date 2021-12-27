extern crate futures;
extern crate reqwest;
extern crate url;

use crate::chapter::Book;
use crate::chapter::Chapter;
use crate::models::BookKind;
use crate::models::ChapterKind;
use crate::models::NewBook;
use crate::models::NewChapter;

use chrono::Utc;
pub use error::Error;
use futures::future::try_join_all;
use scraper::{Html, Selector};
use std::collections::BTreeSet;
use url::Url;
use uuid::Uuid;

mod error;

#[derive(Debug, PartialEq, Clone)]
pub struct RoyalRoadBook {
    pub title: String,
    pub author: String,
    pub royalroad_id: u64,
}

impl From<RoyalRoadBook> for NewBook {
    fn from(b: RoyalRoadBook) -> Self {
        return Self {
            name: b.title,
            author: b.author,
            metadata: BookKind::RoyalRoad { id: b.royalroad_id },
        };
    }
}

impl RoyalRoadBook {
    pub fn royalroad_book_id(request_url: &str) -> Result<u64, Error> {
        let request_url = Url::parse(request_url)?;
        let valid_hosts = vec!["www.royalroad.com", "royalroad.com"];
        if request_url.host_str().is_none()
            || !valid_hosts
                .iter()
                .any(|v| *v == request_url.host_str().unwrap())
        {
            return Err(Error::UrlError(String::from(
                "Provided hostname is not www.royalroad.com or royalroad.com.",
            )));
        }
        let path_segments = request_url.path_segments();
        let mut path_segments = match path_segments {
            None => return Err(Error::UrlError(String::from("No path provided."))),
            Some(segments) => segments,
        };

        let path_start = path_segments.next();
        if path_start != Some("fiction") {
            return Err(Error::UrlError(String::from(
                "Url does not correspond to a book.",
            )));
        }
        let royalroad_id: Option<u64> = path_segments.next().and_then(|id| id.parse().ok());
        if royalroad_id.is_none() {
            return Err(Error::UrlError("Book id not valid.".into()));
        }
        return Ok(royalroad_id.unwrap());
    }

    pub async fn from_book_id(book_id: u64) -> Result<RoyalRoadBook, Error> {
        return fetch_book_meta(book_id).await;
    }
}

#[tracing::instrument(
name = "Fetching Book Metadata",
err,
level = "info"
fields(
    request_id = %Uuid::new_v4(),
)
)]
async fn fetch_book_meta(book_id: u64) -> Result<RoyalRoadBook, Error> {
    let link = format!("https://royalroad.com/fiction/{}", book_id);
    let html = reqwest::get(&link).await?.text().await?;
    let doc = Html::parse_document(&html);
    let title_selector = Selector::parse("div.fic-header h1").unwrap();
    let author_selector = Selector::parse("div.fic-header h4 span[property=name]").unwrap();

    let title = doc
        .select(&title_selector)
        .next()
        .ok_or_else(|| {
            Error::WebParseError("Failed to find title element on royalroad page.".into())
        })?
        .text()
        .fold(String::new(), |a, b| a + b)
        .trim()
        .to_string();

    if title.is_empty() {
        return Err(Error::WebParseError(
            "Empty title element on royalroad page.".into(),
        ));
    }

    let author = doc
        .select(&author_selector)
        .next()
        .ok_or_else(|| {
            Error::WebParseError("Failed to find author element on royalroad page.".into())
        })?
        .text()
        .fold(String::new(), |a, b| a + b)
        .trim()
        .to_string();
    if author.is_empty() {
        return Err(Error::WebParseError(
            "Empty author element on royalroad page.".into(),
        ));
    }
    Ok(RoyalRoadBook {
        title,
        author,
        royalroad_id: book_id,
    })
}

pub async fn download_book(
    chapter_ids: &BTreeSet<u64>,
) -> Result<Book, Box<dyn std::error::Error>> {
    if chapter_ids.is_empty() {
        bail!("Expected chapter ids, but received an empty set.")
    }
    let chapter_futures: Vec<_> = chapter_ids.iter().map(|id| get_chapter(id)).collect();

    let chapters = try_join_all(chapter_futures).await?;
    let title = chapters[0].title.clone();
    let title_is_same = chapters
        .iter()
        .map(|chapter| &chapter.title)
        .all(|chapter_title| chapter_title == &title);
    let author = chapters[0].author.clone();
    let author_is_same = chapters
        .iter()
        .map(|chapter| &chapter.author)
        .all(|chapter_author| chapter_author == &author);
    if !(author_is_same && title_is_same) {
        bail!("Provided chapter ids do not belong to the same novel.");
    }

    let chapters: Vec<Chapter> = chapters.into_iter().map(|chap| chap.chapter).collect();

    Ok(Book {
        title: title,
        author: author,
        chapters: chapters,
    })
}

struct ChapterWithMeta {
    chapter: Chapter,
    author: String,
    title: String,
}

async fn get_chapter(chapter_id: &u64) -> Result<ChapterWithMeta, Box<dyn std::error::Error>> {
    let link = format!("https://www.royalroad.com/fiction/chapter/{}", chapter_id);
    let res = reqwest::get(&link).await?.text().await?;
    let doc = Html::parse_document(&res);
    let chapter_body_selector = Selector::parse("div.chapter-inner").unwrap();
    let chapter_title_selector = Selector::parse("div.fic-header h1").unwrap();
    let book_title_selector = Selector::parse("div.fic-header h2").unwrap();
    let chapter_author_selector = Selector::parse("div.fic-header h3").unwrap();

    let body = doc
        .select(&chapter_body_selector)
        .next()
        .ok_or_else(|| simple_error!(&format!("Failed to find body in {}", link)))?
        .html();
    let title = doc
        .select(&chapter_title_selector)
        .next()
        .ok_or_else(|| simple_error!(&format!("Failed to find title in {}", link)))?
        .text()
        .fold(String::new(), |a, b| a + b)
        .trim()
        .to_string();

    if title.is_empty() {
        bail!("Title text was empty.")
    }

    let book_title = doc
        .select(&book_title_selector)
        .next()
        .ok_or_else(|| simple_error!(&format!("Failed to find book title in {}", link)))?
        .text()
        .fold(String::new(), |a, b| a + b)
        .trim()
        .to_string();

    if book_title.is_empty() {
        bail!("Title text was empty.")
    }
    let author = doc
        .select(&chapter_author_selector)
        .next()
        .ok_or_else(|| simple_error!(&format!("Failed to find author in {}", link)))?
        .text()
        .fold(String::new(), |a, b| a + b)
        .trim()
        .to_string();
    if author.is_empty() {
        bail!("Author text was empty.")
    }
    Ok(ChapterWithMeta {
        chapter: Chapter { body, title },
        author: author,
        title: book_title,
    })
}

async fn get_chapter_body(chapter_id: &u64) -> Result<String, Error> {
    let link = format!("https://www.royalroad.com/fiction/chapter/{}", chapter_id);
    let res = reqwest::get(&link).await?.text().await?;
    let doc = Html::parse_document(&res);
    let chapter_body_selector = Selector::parse("div.chapter-inner").unwrap();

    let body = doc
        .select(&chapter_body_selector)
        .next()
        .ok_or_else(|| Error::WebParseError(format!("Failed to find body in {}", link)))?
        .html();
    Ok(body)
}

pub async fn get_chapters(
    book_id: u64,
    book_uuid: Uuid,
    author: &str,
) -> Result<Vec<NewChapter>, Error> {
    let content = reqwest::get(format!("https://www.royalroad.com/syndication/{}", book_id))
        .await?
        .bytes()
        .await?;
    let channel = rss::Channel::read_from(&content[..])?;
    channel
        .items()
        .iter()
        .map(|item| {
            Ok(NewChapter {
                book_id: book_uuid,
                metadata: ChapterKind::RoyalRoad {
                    id: get_chapter_id_from_link(item.link())?,
                },
                author: author.into(),
                name: item
                    .title()
                    .ok_or(Error::RssContentsError(
                        "No valid royalroad chapter title in RSS Item.".into(),
                    ))?
                    .into(),
                published_at: get_published_at(item.pub_date())?,
            })
        })
        .collect()
}

fn get_chapter_id_from_link(link: Option<&str>) -> Result<u64, Error> {
    link.and_then(|link| {
        link.rsplit_once("/")
            .map(|(_left, right)| right)
            .and_then(|x| x.parse().ok())
    })
    .ok_or(Error::RssContentsError(
        "No valid royalroad chapter link in RSS Item.".into(),
    ))
}

fn get_published_at(pub_date: Option<&str>) -> Result<chrono::DateTime<Utc>, Error> {
    match pub_date {
        Some(datestamp) => match chrono::DateTime::parse_from_rfc2822(datestamp) {
            Ok(date) => Ok(date.with_timezone(&Utc)),
            Err(_) => Err(Error::RssContentsError(
                "No valid published date in RSS Item".into(),
            )),
        },
        None => Err(Error::RssContentsError(
            "No valid published date in RSS Item".into(),
        )),
    }
}
