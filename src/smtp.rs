use reqwest::multipart::Part;
use std::env;
use uuid::Uuid;

use crate::chapter::BookMeta;

pub use self::errors::Error;

#[derive(Debug, Clone)]
pub struct Attachment {
    pub content_type: String,
    pub file_name: String,
    pub bytes: Vec<u8>,
}

pub struct Message {
    to: String,
    subject: String,
    text: Option<String>,
    html: Option<String>,
    attachment: Option<Attachment>,
}

impl Message {
    pub fn new(
        to: &str,
        subject: &str,
        text: Option<&str>,
        html: Option<&str>,
        attachment: Option<Attachment>,
    ) -> Self {
        Message {
            to: to.into(),
            subject: subject.into(),
            text: text.map(|x| x.into()),
            html: html.map(|x| x.into()),
            attachment: attachment,
        }
    }
}

#[tracing::instrument(
name = "Sending an email",
err,
level = "info"
skip(message),
fields(
    request_id = %Uuid::new_v4(),
)
)]
pub async fn send_file_smtp(message: Message) -> Result<(), Error> {
    let client = reqwest::Client::new();
    let mut form = reqwest::multipart::Form::new()
        .text("to", message.to)
        .text("subject", message.subject)
        .text("from", "postmaster@cereal.works");
    if let Some(text) = message.text {
        form = form.text("text", text)
    }
    if let Some(html) = message.html {
        form = form.text("html", html)
    }
    if let Some(attachment) = message.attachment {
        form = form.part(
            "attachment",
            Part::bytes(attachment.bytes)
                .file_name(attachment.file_name)
                .mime_str(&attachment.content_type)?,
        )
    }
    let mailgun_api_key =
        env::var("CEREAL_MAILGUN_API_KEY").expect("Mailgun API key not provided.");
    let send_email_response = client
        .post("https://api.mailgun.net/v3/mg.cereal.works/messages")
        .basic_auth("api", Some(mailgun_api_key))
        .multipart(form)
        .send()
        .await?;
    if !send_email_response.status().is_success() {
        return Err(Error::MailgunError(send_email_response.status()));
    };
    Ok(())
}

pub async fn send_mobi_file(
    bytes: &[u8],
    email: &str,
    title: &str,
    subject: &str,
) -> Result<(), Error> {
    let attachment = Attachment {
        content_type: "application/x-mobipocket-ebook".into(),
        file_name: format!("{}.mobi", &title),
        bytes: Vec::from(bytes),
    };
    let message = Message::new(
        email,
        &subject,
        Some(&subject),
        Some(&subject),
        Some(attachment),
    );
    send_file_smtp(message).await?;
    Ok(())
}

pub async fn send_book(bytes: &[u8], email: &str, book: &BookMeta) -> Result<(), Error> {
    let subject = format!("A new chapter of {}.", &book.title);
    send_mobi_file(bytes, email, &book.title, &subject).await?;
    Ok(())
}

mod errors {
    use std::fmt::Display;

    #[derive(Debug)]
    pub enum Error {
        ContentTypeError(reqwest::Error),
        MailgunError(reqwest::StatusCode),
    }

    impl Display for Error {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self))
        }
    }

    impl std::error::Error for Error {}

    impl From<reqwest::Error> for Error {
        fn from(x: reqwest::Error) -> Self {
            Error::ContentTypeError(x)
        }
    }
}
