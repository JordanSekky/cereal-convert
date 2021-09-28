use crate::chapter::BookMeta;
use lettre::message::header::ContentType;
use lettre::message::Attachment;
use lettre::{transport::smtp::authentication::Credentials, Message, SmtpTransport, Transport};
use std::env;
use std::error::Error;

pub async fn send_file_smtp(
    bytes: Vec<u8>,
    email: &str,
    book: &BookMeta,
) -> Result<(), Box<dyn Error>> {
    let from = String::from("postmaster@cereal.works");
    let email_body = Message::builder()
        .from(from.parse()?)
        .to(email.parse()?)
        .subject(format!("A new chapter of {}.", &book.title))
        .singlepart(
            Attachment::new(format!("{}.mobi", &book.title))
                .body(bytes, ContentType::parse("application/x-mobipocket-ebook")?),
        )?;
    let transport = SmtpTransport::relay(&env::var("CEREAL_SMTP_HOST")?)
        .unwrap()
        .credentials(Credentials::new(
            env::var("CEREAL_SMTP_USERNAME")?,
            env::var("CEREAL_SMTP_PASSWORD")?,
        ))
        .build();

    transport.send(&email_body)?;
    Ok(())
}
