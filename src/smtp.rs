use crate::chapter::AggregateBook;
use lettre::message::header::ContentType;
use lettre::message::Attachment;
use lettre::{transport::smtp::authentication::Credentials, Message, SmtpTransport, Transport};
use std::env;
use std::path::Path;

pub async fn send_file_smtp(path: &Path, email: &str, book: &AggregateBook) {
    let from = String::from("postmaster@cereal.works");
    let email_body = Message::builder()
        .from(from.parse().unwrap())
        .to(email.parse().unwrap())
        .subject(format!("A new chapter of {}.", &book.title))
        .singlepart(Attachment::new(path.to_str().unwrap().to_owned()).body(
            std::fs::read(path).unwrap(),
            ContentType::parse("application/x-mobipocket-ebook").unwrap(),
        ))
        .unwrap();
    let transport = SmtpTransport::relay(&env::var("CEREAL_SMTP_HOST").unwrap())
        .unwrap()
        .credentials(Credentials::new(
            env::var("CEREAL_SMTP_USERNAME").unwrap(),
            env::var("CEREAL_SMTP_PASSWORD").unwrap(),
        ))
        .build();

    transport.send(&email_body).unwrap();
}
