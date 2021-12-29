use derive_more::{Display, Error, From};

use crate::{calibre, mailgun, pushover};

#[derive(Debug, Display, From, Error)]
pub enum Error {
    EstablishConnection(mobc::Error<diesel::ConnectionError>),
    QueryResult(diesel::result::Error),
    ValidationConversion(calibre::Error),
    ValidationDelivery(mailgun::Error),
    #[from(ignore)]
    Validation(#[error(not(source))] String),
    #[from(ignore)]
    EmailParse(#[error(not(source))] String),
    NotKindleEmail,
    NoPushoverKey,
    PushoverDelivery(pushover::Error),
}

impl<'a> From<addr::error::Error<'a>> for Error {
    fn from(x: addr::error::Error<'a>) -> Self {
        Self::EmailParse(x.to_string())
    }
}
