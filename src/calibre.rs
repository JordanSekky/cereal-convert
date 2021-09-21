use std::fs;
use std::path::PathBuf;
use std::process::Command;

use crate::chapter::AggregateBook;
use std::error::Error;

pub fn convert_to_mobi(book: &AggregateBook) -> Result<PathBuf, Box<dyn Error>> {
    fs::write(format!("/tmp/{}.html", book.title), &book.body)?;
    let output = Command::new("ebook-convert")
        .arg(format!("/tmp/{}.html", book.title))
        .arg(format!("/tmp/{}.mobi", book.title))
        .arg("--filter-css")
        .arg(r#""font-family,color,background""#)
        .arg("--authors")
        .arg(format!(r#""{}""#, book.author))
        .arg("--title")
        .arg(format!(r#""{}""#, book.title))
        //     .arg(r#"--cover cover.jpg"#)
        .arg("--output-profile")
        .arg("kindle_oasis")
        .output()?;

    println!("{}", String::from_utf8(output.stdout)?);
    println!("{}", String::from_utf8(output.stderr)?);
    println!("{}", output.status);
    Ok(PathBuf::from(format!("/tmp/{}.mobi", book.title)))
}
