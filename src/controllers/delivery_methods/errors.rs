use std::fmt::Display;

use crate::{calibre, smtp};

#[derive(Debug)]
pub enum Error {
    EstablishConnection(mobc::Error<diesel::ConnectionError>),
    QueryResult(diesel::result::Error),
    Validation(String),
    ValidationConversion(calibre::Error),
    ValidationDelivery(smtp::Error),
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

impl From<smtp::Error> for Error {
    fn from(x: smtp::Error) -> Self {
        Error::ValidationDelivery(x)
    }
}
