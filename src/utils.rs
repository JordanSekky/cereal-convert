use std::fmt::Display;

#[derive(Debug)]
pub enum ResponseError {
    EstablishConnection(mobc::Error<diesel::ConnectionError>),
    QueryResult(diesel::result::Error),
    UrlParseError(url::ParseError),
    ReqwestError(reqwest::Error),
    RoyalRoadError { message: String },
    RoyalRoadUrlError { message: String },
}

impl Display for ResponseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{:?}", self))
    }
}

impl std::error::Error for ResponseError {}

impl From<mobc::Error<diesel::ConnectionError>> for ResponseError {
    fn from(x: mobc::Error<diesel::ConnectionError>) -> Self {
        ResponseError::EstablishConnection(x)
    }
}

impl From<diesel::result::Error> for ResponseError {
    fn from(x: diesel::result::Error) -> Self {
        ResponseError::QueryResult(x)
    }
}

impl From<url::ParseError> for ResponseError {
    fn from(x: url::ParseError) -> Self {
        ResponseError::UrlParseError(x)
    }
}

impl From<reqwest::Error> for ResponseError {
    fn from(x: reqwest::Error) -> Self {
        ResponseError::ReqwestError(x)
    }
}
