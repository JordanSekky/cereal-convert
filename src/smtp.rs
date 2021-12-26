use crate::chapter::BookMeta;
use lettre::message::header::ContentType;
use lettre::message::Attachment;
use lettre::{transport::smtp::authentication::Credentials, Message, SmtpTransport, Transport};
use std::env;
use uuid::Uuid;

pub use self::errors::Error;

#[tracing::instrument(
name = "Sending an email",
err,
level = "info"
skip(bytes),
fields(
    request_id = %Uuid::new_v4(),
)
)]
pub async fn send_file_smtp(
    bytes: &[u8],
    attachment_name: &str,
    email: &str,
    subject: &str,
) -> Result<(), Error> {
    let from = String::from("postmaster@cereal.works");
    let email_body = Message::builder()
        .from(from.parse()?)
        .to(email.parse()?)
        .subject(subject)
        .singlepart(Attachment::new(attachment_name.into()).body::<Vec<u8>>(
            bytes.into(),
            ContentType::parse("application/x-mobipocket-ebook")?,
        ))?;
    let transport =
        SmtpTransport::relay(&env::var("CEREAL_SMTP_HOST").expect("SMTP Host not provided."))
            .unwrap()
            .credentials(Credentials::new(
                env::var("CEREAL_SMTP_USERNAME").expect("SMTP Credentials not provided."),
                env::var("CEREAL_SMTP_PASSWORD").expect("SMTP Credentials not provided."),
            ))
            .build();

    transport.send(&email_body)?;
    Ok(())
}

pub async fn send_book_smtp(bytes: &[u8], email: &str, book: &BookMeta) -> Result<(), Error> {
    let subject = format!("A new chapter of {}.", &book.title);
    let attachment_name = format!("{}.mobi", &book.title);
    send_file_smtp(bytes, &attachment_name, email, &subject).await?;
    Ok(())
}

mod errors {
    use std::fmt::Display;

    #[derive(Debug)]
    pub enum Error {
        AddressError(lettre::address::AddressError),
        ContentTypeError(lettre::message::header::ContentTypeErr),
        MessageError(lettre::error::Error),
        SmtpError(lettre::transport::smtp::Error),
    }

    impl Display for Error {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self))
        }
    }

    impl std::error::Error for Error {}

    impl From<lettre::address::AddressError> for Error {
        fn from(x: lettre::address::AddressError) -> Self {
            Error::AddressError(x)
        }
    }

    impl From<lettre::message::header::ContentTypeErr> for Error {
        fn from(x: lettre::message::header::ContentTypeErr) -> Self {
            Error::ContentTypeError(x)
        }
    }
    impl From<lettre::error::Error> for Error {
        fn from(x: lettre::error::Error) -> Self {
            Error::MessageError(x)
        }
    }

    impl From<lettre::transport::smtp::Error> for Error {
        fn from(x: lettre::transport::smtp::Error) -> Self {
            Error::SmtpError(x)
        }
    }
}
