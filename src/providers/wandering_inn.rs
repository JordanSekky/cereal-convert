use anyhow::anyhow;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use itertools::Itertools;
use scraper::{Html, Selector};
use uuid::Uuid;

use crate::models::{BookKind, ChapterKind, NewBook, NewChapter};
use crate::util::parse_from_rfc2822;
use crate::util::validate_hostname;

pub fn get_book() -> NewBook {
    NewBook {
        name: "The Wandering Inn".into(),
        author: "Pirateaba".into(),
        metadata: BookKind::TheWanderingInn,
    }
}

pub async fn get_chapters(book_uuid: &Uuid) -> Result<Vec<NewChapter>> {
    let content = reqwest::get("https://wanderinginn.com/feed/")
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
                metadata: ChapterKind::TheWanderingInn {
                    url: item
                        .link()
                        .ok_or_else(|| anyhow!("No chapter link in RSS item. Item {:?}", &item))?
                        .into(),
                },
                author: "Pirateaba".into(),
                name: item
                    .title()
                    .ok_or_else(|| anyhow!("No chapter title in RSS item. Item {:?}", &item))?
                    .into(),
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

pub async fn get_chapter_body(link: &str) -> Result<String> {
    let res = reqwest::get(link).await?.text().await?;
    let doc = Html::parse_document(&res);
    let chapter_body_elem_selector = Selector::parse("div.entry-content > *").unwrap();

    let body = doc
        .select(&chapter_body_elem_selector)
        .filter(|x| !x.text().any(|t| t == "Next Chapter"))
        .filter(|x| !x.text().any(|t| t == "Previous Chapter"))
        .map(|x| x.html())
        .join("\n");
    if body.trim().is_empty() {
        bail!("Failed to find chapter body.");
    }
    Ok(body)
}

pub fn try_parse_url(url: &str) -> Result<()> {
    let valid_host = "wanderinginn.com";
    validate_hostname(url, valid_host)
}
