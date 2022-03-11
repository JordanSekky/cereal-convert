mod errors;
use rand::Rng;
use std::fs;
use tokio::process::Command;
use tracing::info;
use uuid::Uuid;

pub use self::errors::Error;

#[tracing::instrument(
name = "Converting to mobi",
err,
level = "info"
skip(body),
fields(
    request_id = %Uuid::new_v4(),
)
)]
pub async fn generate_mobi(
    input_extension: &str,
    body: &str,
    cover_title: &str,
    book_title: &str,
    author: &str,
) -> Result<Vec<u8>, errors::Error> {
    let file_name: String = rand::thread_rng()
        .sample_iter(rand::distributions::Alphanumeric)
        .take(30)
        .map(char::from)
        .collect();
    let in_path = format!("/tmp/{}.{}", file_name, input_extension);
    let out_path = format!("/tmp/{}.mobi", file_name);
    fs::write(&in_path, body)?;
    let cover_gen_output = Command::new("calibre-debug")
        .arg("-c")
        .arg(format!("from calibre.ebooks.covers import *; open('/tmp/cover.jpg', 'wb').write(create_cover('{}', ['{}']))", cover_title.replace("'", "\\'").replace("\"", "\\\""), author))
        .output().await?;
    info!(
        stdout = ?String::from_utf8_lossy(&cover_gen_output.stdout),
        stderr = ?String::from_utf8_lossy(&cover_gen_output.stderr),
        status_code = ?cover_gen_output.status
    );
    if !cover_gen_output.status.success() {
        return Err(errors::Error::GenerateCover);
    };
    let output = Command::new("ebook-convert")
        .arg(&in_path)
        .arg(&out_path)
        .arg("--filter-css")
        .arg(r#""font-family,color,background""#)
        .arg("--authors")
        .arg(author)
        .arg("--title")
        .arg(book_title)
        .arg(r#"--cover"#)
        .arg(r#"/tmp/cover.jpg"#)
        .arg("--output-profile")
        .arg("kindle_oasis")
        .output()
        .await?;
    info!(
        stdout = ?String::from_utf8_lossy(&output.stdout),
        stderr = ?String::from_utf8_lossy(&output.stderr),
        status_code = ?output.status
    );
    if !output.status.success() {
        return Err(errors::Error::ConvertFile);
    }
    let bytes = fs::read(&out_path)?;
    fs::remove_file(&in_path)?;
    fs::remove_file(&out_path)?;
    Ok(bytes)
}

pub async fn generate_kindle_email_validation_mobi(code: &str) -> Result<Vec<u8>, errors::Error> {
    let body = format!("Thank you for using cereal. To validate your kindle email address, please input the following code: {}", code);
    let title = "Cereal Kindle Email Validation Book";

    return generate_mobi("txt", &body, title, title, "Cereal").await;
}
