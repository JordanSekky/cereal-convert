use anyhow::{bail, Error};
use reqwest::multipart::Part;
use std::env;
use uuid::Uuid;

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
        Self {
            to: to.into(),
            subject: subject.into(),
            text: text.map(std::convert::Into::into),
            html: html.map(std::convert::Into::into),
            attachment,
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
pub async fn send_message(message: Message) -> Result<(), Error> {
    let client = reqwest::Client::new();
    let mut form = reqwest::multipart::Form::new()
        .text("to", message.to)
        .text("subject", message.subject)
        .text("from", env::var("CEREAL_FROM_EMAIL_ADDRESS").unwrap());
    if let Some(text) = message.text {
        form = form.text("text", text);
    }
    if let Some(html) = message.html {
        form = form.text("html", html);
    }
    if let Some(attachment) = message.attachment {
        form = form.part(
            "attachment",
            Part::bytes(attachment.bytes)
                .file_name(attachment.file_name)
                .mime_str(&attachment.content_type)?,
        );
    }
    let mailgun_api_key =
        env::var("CEREAL_MAILGUN_API_KEY").expect("Mailgun API key not provided.");
    let send_email_response = client
        .post(env::var("CEREAL_MAILGUN_API_ENDPOINT").unwrap())
        .basic_auth("api", Some(mailgun_api_key))
        .multipart(form)
        .send()
        .await?;
    if !send_email_response.status().is_success() {
        bail!(
            "Received unsuccessful status code from mailgun: {}",
            send_email_response.status()
        );
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
        subject,
        Some(subject),
        Some(subject),
        Some(attachment),
    );
    send_message(message).await
}
