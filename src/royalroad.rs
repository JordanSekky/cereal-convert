extern crate futures;
extern crate reqwest;

use crate::chapter::Book;
use crate::chapter::Chapter;
use crate::configuration::RoyalRoadConfiguration;

use feed_rs::parser;
use futures::future::try_join_all;
use lazy_static::lazy_static;
use regex::Regex;
use scraper::{Html, Selector};
use std::collections::HashSet;
use std::error::Error;
use std::fs;
use std::io::Write;

pub async fn download(config: &RoyalRoadConfiguration) -> Result<Vec<Book>, Box<dyn Error>> {
    let books: Vec<_> = config
        .ids
        .iter()
        .map(|book_id| download_book(&book_id))
        .collect();
    Ok(try_join_all(books)
        .await?
        .into_iter()
        .filter_map(|download_result| match download_result {
            BookDownloadResult::NewChapters(book) => Some(book),
            BookDownloadResult::NoNewChapters() => None,
        })
        .collect())
}

enum BookDownloadResult {
    NewChapters(Book),
    NoNewChapters(),
}

async fn download_book(book_id: &u32) -> Result<BookDownloadResult, Box<dyn Error>> {
    let chapter_ids = get_chapter_ids(book_id).await?;
    let new_chapter_ids: Vec<u32> = chapter_ids
        .into_iter()
        .filter(|id| !chapter_seen_before(id))
        .collect();

    if new_chapter_ids.is_empty() {
        return Ok(BookDownloadResult::NoNewChapters());
    }
    let chapter_futures: Vec<_> = new_chapter_ids.iter().map(|id| get_chapter(id)).collect();

    let chapters = try_join_all(chapter_futures).await?;
    let title = chapters
        .iter()
        .next()
        .expect("No chapters found for book.")
        .title
        .clone();
    let author = chapters
        .iter()
        .next()
        .expect("No chapters found for book.")
        .author
        .clone();
    let chapters: Vec<Chapter> = chapters.into_iter().map(|chap| chap.chapter).collect();

    mark_new_chapters(&new_chapter_ids)?;
    Ok(BookDownloadResult::NewChapters(Book {
        title: title,
        author: author,
        chapters: chapters,
    }))
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

fn chapter_seen_before(id: &u32) -> bool {
    lazy_static! {
        static ref SEEN_IDS: HashSet<u32> = String::from_utf8_lossy(&fs::read("royalroad_seen_chapter_ids.txt").unwrap_or(b"".to_vec()))
            .lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty())
            .map(|id_string| id_string.parse().expect(&format!("Chapter id {} in file royalroad_seen_chapter_ids.txt could not be parsed as an unsigned int.", id_string)))
            .collect();
    }
    SEEN_IDS.contains(id)
}

fn mark_new_chapters(ids: &Vec<u32>) -> Result<(), Box<dyn Error>> {
    let mut file = fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open("royalroad_seen_chapter_ids.txt")?;
    for id in ids {
        writeln!(file, "{}\n", id)?;
    }
    Ok(())
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
        .expect(&format!("Failed to find body in {}", link))
        .html();
    let title = doc
        .select(&chapter_title_selector)
        .next()
        .expect(&format!("Failed to find title in {}", link))
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
        .expect(&format!("Failed to find book title in {}", link))
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
        .expect(&format!("Failed to find author in {}", link))
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
