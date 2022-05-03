extern crate futures;
extern crate reqwest;
extern crate url;

use crate::models::BookKind;
use crate::models::ChapterKind;
use crate::models::NewBook;
use crate::models::NewChapter;

use anyhow::anyhow;
use anyhow::bail;
use anyhow::Context;
use chrono::Utc;
use rss::Item;
use scraper::{Html, Selector};
use serde::Deserialize;
use serde::Serialize;
use url::Url;
use uuid::Uuid;

use anyhow::Result;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Hash)]
pub struct RoyalRoadBookKind {
    pub id: u64,
}

pub fn try_parse_url(request_url: &str) -> Result<RoyalRoadBookKind> {
    let request_url = Url::parse(request_url)?;
    let valid_hosts = vec!["www.royalroad.com", "royalroad.com"];
    if request_url.host_str().is_none()
        || !valid_hosts
            .iter()
            .any(|v| *v == request_url.host_str().unwrap())
    {
        bail!(
            "Provided hostname {} is not www.royalroad.com or royalroad.com.",
            request_url
        );
    }
    let mut path_segments = request_url
        .path_segments()
        .ok_or_else(|| anyhow!("No path provided"))?;

    let path_start = path_segments.next();
    if path_start != Some("fiction") {
        bail!("Url {} is not a royalroad book.", request_url);
    }
    let royalroad_id: Option<u64> = path_segments.next().and_then(|id| id.parse().ok());
    if royalroad_id.is_none() {
        bail!("Book id in url {} not valid.", request_url);
    }
    Ok(RoyalRoadBookKind {
        id: royalroad_id.unwrap(),
    })
}

#[tracing::instrument(
name = "Fetching Book Metadata",
err,
level = "info"
fields(
    request_id = %Uuid::new_v4(),
)
)]
pub async fn as_new_book(book_meta: &RoyalRoadBookKind) -> Result<NewBook> {
    return fetch_book_meta(book_meta).await;
}

async fn fetch_book_meta(book_meta: &RoyalRoadBookKind) -> Result<NewBook> {
    let link = format!("https://royalroad.com/fiction/{}", book_meta.id);
    let html = reqwest::get(&link).await?.text().await?;
    let doc = Html::parse_document(&html);
    let title_selector = Selector::parse("div.fic-header h1").unwrap();
    let author_selector = Selector::parse("div.fic-header h4 span[property=name]").unwrap();

    let title = doc
        .select(&title_selector)
        .next()
        .ok_or_else(|| anyhow!("Failed to find title element on royalroad page."))?
        .text()
        .fold(String::new(), |a, b| a + b)
        .trim()
        .to_string();

    if title.is_empty() {
        bail!("Empty title element on royalroad page.");
    }

    let author = doc
        .select(&author_selector)
        .next()
        .ok_or_else(|| anyhow!("Failed to find author element on royalroad page."))?
        .text()
        .fold(String::new(), |a, b| a + b)
        .trim()
        .to_string();
    if author.is_empty() {
        bail!("Empty author element on royalroad page.");
    }
    Ok(NewBook {
        name: title,
        author,
        metadata: BookKind::RoyalRoad(book_meta.clone()),
    })
}

pub async fn get_chapter_body(chapter_id: &u64) -> Result<String> {
    let link = format!("https://www.royalroad.com/fiction/chapter/{}", chapter_id);
    let res = reqwest::get(&link).await?.text().await?;
    let doc = Html::parse_document(&res);
    let chapter_body_selector = Selector::parse("div.chapter-inner").unwrap();

    let body = doc
        .select(&chapter_body_selector)
        .next()
        .ok_or_else(|| anyhow!("Failed to find body in {}", link))?
        .html();
    Ok(body)
}

pub async fn get_chapters(book_id: u64, book_uuid: &Uuid, author: &str) -> Result<Vec<NewChapter>> {
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
                book_id: *book_uuid,
                metadata: ChapterKind::RoyalRoad {
                    id: get_chapter_id_from_link(item.link())?,
                },
                author: author.into(),
                name: get_chapter_title_from_rss(item, channel.title())?,
                published_at: parse_from_rfc2822(
                    item.pub_date()
                        .ok_or_else(|| anyhow!("No publish date in RSS item. Item {:?}", &item))?,
                )
                .with_context(|| {
                    format!("Failed to parse publish date in RSS item. Item {:?}", &item)
                })?,
            })
        })
        .collect()
}

fn get_chapter_title_from_rss(item: &Item, channel_title: &str) -> Result<String> {
    let rss_item_title = item
        .title()
        .ok_or_else(|| anyhow!("No valid royalroad chapter title in RSS Item."))?;
    if let Some((_book_title, chapter_title)) =
        rss_item_title.split_once(&format!("{} - ", channel_title))
    {
        return Ok(chapter_title.trim().into());
    }
    Ok(rss_item_title.into())
}

fn get_chapter_id_from_link(link: Option<&str>) -> Result<u64> {
    link.and_then(|link| {
        link.rsplit_once('/')
            .map(|(_left, right)| right)
            .and_then(|x| x.parse().ok())
    })
    .ok_or_else(|| anyhow!("No valid royalroad chapter link in RSS Item."))
}

fn parse_from_rfc2822(pub_date: &str) -> Result<chrono::DateTime<Utc>> {
    Ok(chrono::DateTime::parse_from_rfc2822(pub_date)?.with_timezone(&Utc))
}
