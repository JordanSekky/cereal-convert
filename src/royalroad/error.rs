use std::fmt::Display;

#[derive(Debug)]
pub enum Error {
    UrlParseError(url::ParseError),
    ReqwestError(reqwest::Error),
    WebParseError(String),
    UrlError(String),
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{:?}", self))
    }
}

impl std::error::Error for Error {}
impl From<url::ParseError> for Error {
    fn from(x: url::ParseError) -> Self {
        Error::UrlParseError(x)
    }
}

impl From<reqwest::Error> for Error {
    fn from(x: reqwest::Error) -> Self {
        Error::ReqwestError(x)
    }
}
