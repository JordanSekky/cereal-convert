use rand::Rng;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use crate::chapter::AggregateBook;
use std::error::Error;

pub fn convert_to_mobi(book: &AggregateBook) -> Result<PathBuf, Box<dyn Error>> {
    let file_name: String = rand::thread_rng()
        .sample_iter(rand::distributions::Alphanumeric)
        .take(30)
        .map(char::from)
        .collect();
    fs::write(format!("/tmp/{}.html", file_name), &book.body)?;
    let output = Command::new("ebook-convert")
        .arg(format!("/tmp/{}.html", file_name))
        .arg(format!("/tmp/{}.mobi", file_name))
        .arg("--filter-css")
        .arg(r#""font-family,color,background""#)
        .arg("--authors")
        .arg(format!(r#""{}""#, book.author))
        .arg("--title")
        .arg(format!(r#""{}""#, book.title))
        // TODO Generate Cover too!
        //     .arg(r#"--cover cover.jpg"#)
        .arg("--output-profile")
        .arg("kindle_oasis")
        .output()?;

    println!("{}", String::from_utf8(output.stdout)?);
    println!("{}", String::from_utf8(output.stderr)?);
    println!("{}", output.status);
    Ok(PathBuf::from(format!("/tmp/{}.mobi", file_name)))
}
