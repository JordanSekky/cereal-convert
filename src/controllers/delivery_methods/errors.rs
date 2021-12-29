use std::fmt::Display;

use crate::{calibre, mailgun, pushover};

#[derive(Debug)]
pub enum Error {
    EstablishConnection(mobc::Error<diesel::ConnectionError>),
    QueryResult(diesel::result::Error),
    EmailParseError,
    NotKindleEmailError,
    NoPushoverKeyError,
    Validation(String),
    ValidationConversion(calibre::Error),
    ValidationDelivery(mailgun::Error),
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{:?}", self))
    }
}

impl std::error::Error for Error {}

impl From<mobc::Error<diesel::ConnectionError>> for Error {
    fn from(x: mobc::Error<diesel::ConnectionError>) -> Self {
        Error::EstablishConnection(x)
    }
}

impl From<diesel::result::Error> for Error {
    fn from(x: diesel::result::Error) -> Self {
        Error::QueryResult(x)
    }
}

impl From<calibre::Error> for Error {
    fn from(x: calibre::Error) -> Self {
        Error::ValidationConversion(x)
    }
}

impl From<mailgun::Error> for Error {
    fn from(x: mailgun::Error) -> Self {
        Error::ValidationDelivery(x)
    }
}

impl<'a> From<addr::error::Error<'a>> for Error {
    fn from(_: addr::error::Error) -> Self {
        Error::EmailParseError
    }
}

impl From<pushover::Error> for Error {
    fn from(x: pushover::Error) -> Self {
        match x {
            pushover::Error::NotificationServiceFailure(_) => {
                Error::Validation("Pushover Notification Server Replied with an error.".into())
            }
            pushover::Error::RequestFailure(_) => {
                Error::Validation("Failed to reach pushover notification server.".into())
            }
        }
    }
}
