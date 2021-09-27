extern crate futures;
extern crate reqwest;

use crate::chapter::Book;
use crate::chapter::Chapter;

use feed_rs::parser;
use futures::future::try_join_all;
use lazy_static::lazy_static;
use regex::Regex;
use scraper::{Html, Selector};
use std::collections::BTreeSet;
use std::error::Error;

pub async fn download_book(chapter_ids: &BTreeSet<u32>) -> Result<Book, Box<dyn Error>> {
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

async fn get_chapter_ids(book_id: &u32) -> Result<Vec<u32>, Box<dyn Error>> {
    let res = reqwest::get(format!("https://www.royalroad.com/syndication/{}", book_id)).await?;
    let feed = res.text().await?;
    let feed = parser::parse(feed.as_bytes())?;
    feed.entries
        .into_iter()
        .flat_map(|entry| entry.links)
        .map(|link| extract_chapter_id_from_link(&link.href))
        .filter(|chapter_id| !chapter_id.is_err())
        .rev()
        .collect()
}

fn extract_chapter_id_from_link(link: &str) -> Result<u32, Box<dyn Error>> {
    lazy_static! {
        static ref RE: Regex =
            Regex::new(r"https://www.royalroad.com/fiction/chapter/(\d+)").unwrap();
    }
    for cap in RE.captures_iter(link) {
        return Ok(cap[1].parse()?);
    }
    bail!("No chapter id found in link.")
}

struct ChapterWithMeta {
    chapter: Chapter,
    author: String,
    title: String,
}

async fn get_chapter(chapter_id: &u32) -> Result<ChapterWithMeta, Box<dyn Error>> {
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
