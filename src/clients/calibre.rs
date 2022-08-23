use anyhow::{bail, Context, Result};
use rand::Rng;
use std::fs;
use tokio::process::Command;
use tracing::info;
use uuid::Uuid;

#[tracing::instrument(
name = "Converting to mobi",
err,
level = "info"
skip(body),
fields(
    request_id = %Uuid::new_v4(),
)
)]
pub async fn generate_epub(
    input_extension: &str,
    body: &str,
    cover_title: &str,
    book_title: &str,
    author: &str,
) -> Result<Vec<u8>> {
    let file_name: String = rand::thread_rng()
        .sample_iter(rand::distributions::Alphanumeric)
        .take(30)
        .map(char::from)
        .collect();
    let in_path = format!("/tmp/{}.{}", file_name, input_extension);
    let out_path = format!("/tmp/{}.epub", file_name);
    fs::write(&in_path, body)?;
    let output = Command::new("ebook-convert")
        .arg(&in_path)
        .arg(&out_path)
        .arg("--filter-css")
        .arg(r#""font-family,color,background""#)
        .arg("--authors")
        .arg(author)
        .arg("--title")
        .arg(cover_title)
        .arg("--series")
        .arg(book_title)
        .arg("--output-profile")
        .arg("kindle_oasis")
        .output()
        .await
        .with_context(|| "Failed to spawn ebook-convert. Perhaps calibre is not installed?")?;
    info!(
        stdout = ?String::from_utf8_lossy(&output.stdout),
        stderr = ?String::from_utf8_lossy(&output.stderr),
        status_code = ?output.status
    );
    if !output.status.success() {
        bail!("Calibre conversion failed with status {:?}", output.status);
    }
    let bytes = fs::read(&out_path)?;
    fs::remove_file(&in_path)?;
    fs::remove_file(&out_path)?;
    Ok(bytes)
}

pub async fn generate_kindle_email_validation_epub(code: &str) -> Result<Vec<u8>> {
    let body = format!("Thank you for using cereal. To validate your kindle email address, please input the following code: {}", code);
    let title = "Cereal Kindle Email Validation Book";

    return generate_epub("txt", &body, title, title, "Cereal").await;
}
