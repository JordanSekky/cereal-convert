use chrono::Utc;
use itertools::Itertools;
use reqwest::Url;
use scraper::{Html, Selector};
use uuid::Uuid;

use crate::models::{BookKind, ChapterKind, NewBook, NewChapter};

pub fn get_book() -> NewBook {
    NewBook {
        name: "Pale".into(),
        author: "Wildbow".into(),
        metadata: BookKind::Pale,
    }
}

pub async fn get_chapters(book_uuid: &Uuid) -> Result<Vec<NewChapter>, Error> {
    let content = reqwest::get("https://palewebserial.wordpress.com/feed/")
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
                metadata: ChapterKind::Pale {
                    url: item
                        .link()
                        .ok_or_else(|| Error::RssContents("No chapter link in RSS item.".into()))?
                        .into(),
                },
                author: "Wildbow".into(),
                name: item
                    .title()
                    .ok_or_else(|| {
                        Error::RssContents("No valid pale chapter title in RSS Item.".into())
                    })?
                    .into(),
                published_at: parse_from_rfc2822(item.pub_date())?,
            })
        })
        .collect()
}

fn parse_from_rfc2822(pub_date: Option<&str>) -> Result<chrono::DateTime<Utc>, Error> {
    match pub_date {
        Some(datestamp) => match chrono::DateTime::parse_from_rfc2822(datestamp) {
            Ok(date) => Ok(date.with_timezone(&Utc)),
            Err(_) => Err(Error::RssContents(
                "No valid published date in RSS Item".into(),
            )),
        },
        None => Err(Error::RssContents(
            "No valid published date in RSS Item".into(),
        )),
    }
}

pub async fn get_chapter_body(link: &str) -> Result<String, Error> {
    let res = reqwest::get(link).await?.text().await?;
    let doc = Html::parse_document(&res);
    let chapter_body_elem_selector = Selector::parse("div.entry-content > *").unwrap();

    let body = doc
        .select(&chapter_body_elem_selector)
        .filter(|x| x.value().id() != Some("jp-post-flair"))
        .filter(|x| !x.text().any(|t| t == "Next Chapter"))
        .filter(|x| !x.text().any(|t| t == "Previous Chapter"))
        .map(|x| x.html())
        .join("\n");
    if body.trim().is_empty() {
        return Err(Error::WebParse("Failed to find chapter body.".into()));
    }
    Ok(body)
}
use derive_more::{Display, Error, From};

#[derive(Debug, Display, From, Error)]
#[display(fmt = "Pale Error: {}")]
pub enum Error {
    UrlParse(url::ParseError),
    Reqwest(reqwest::Error),
    Rss(rss::Error),
    #[from(ignore)]
    #[display(fmt = "WebParseError: {}", "_0")]
    WebParse(#[error(not(source))] String),
    #[from(ignore)]
    #[display(fmt = "RssContentsError: {}", "_0")]
    RssContents(#[error(not(source))] String),
    #[from(ignore)]
    #[display(fmt = "UrlError: {}", "_0")]
    Url(#[error(not(source))] String),
}

pub fn try_parse_url(url: &str) -> Result<(), Error> {
    let request_url = Url::parse(url)?;
    let valid_host = "palewebserial.wordpress.com";
    match request_url.host_str() {
        Some(host) => {
            if valid_host != host {
                return Err(Error::Url(String::from(
                    "Provided hostname is not palewebserial.wordpress.com.",
                )));
            };
        }
        None => return Err(Error::Url("Url has no host.".into())),
    }
    Ok(())
}
