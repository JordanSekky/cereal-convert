use rand::Rng;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tracing::info;

use crate::chapter::AggregateBook;
use std::error::Error;

pub fn convert_to_mobi(book: &AggregateBook) -> Result<PathBuf, Box<dyn Error>> {
    let file_name: String = rand::thread_rng()
        .sample_iter(rand::distributions::Alphanumeric)
        .take(30)
        .map(char::from)
        .collect();
    fs::write(format!("/tmp/{}.html", file_name), &book.body)?;
    let file_title = match book.chapter_titles.len() {
        1 => book.chapter_titles[0].to_string(),
        _ => book.chapter_titles[0].to_string() + " - " + &book.chapter_titles.last().unwrap(),
    };
    let cover_gen_output = Command::new("calibre-debug")
        .arg("-c")
        .arg(format!("from calibre.ebooks.covers import *; open('/tmp/cover.jpg', 'wb').write(create_cover('{}', ['{}']))", file_title, book.author))
        .output()?;
    info!("{}", String::from_utf8(cover_gen_output.stdout.clone())?);
    info!("{}", String::from_utf8(cover_gen_output.stderr.clone())?);
    info!("{}", cover_gen_output.status);
    if !cover_gen_output.status.success() {
        bail!("Cover generation failed to complete successsfully.");
    };
    let output = Command::new("ebook-convert")
        .arg(format!("/tmp/{}.html", file_name))
        .arg(format!("/tmp/{}.mobi", file_name))
        .arg("--filter-css")
        .arg(r#""font-family,color,background""#)
        .arg("--authors")
        .arg(format!(r#"{}"#, book.author))
        .arg("--title")
        .arg(format!(r#"{}: {}"#, book.title, file_title))
        .arg(r#"--cover"#)
        .arg(r#"/tmp/cover.jpg"#)
        .arg("--output-profile")
        .arg("kindle_oasis")
        .output()?;
    info!("{}", String::from_utf8(output.stdout.clone())?);
    info!("{}", String::from_utf8(output.stderr.clone())?);
    info!("{}", output.status);
    if !output.status.success() {
        bail!("MOBI generation failed to complete successsfully.");
    }
    Ok(PathBuf::from(format!("/tmp/{}.mobi", file_name)))
}
