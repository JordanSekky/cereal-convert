use derive_more::{Display, Error, From};

#[derive(Debug, Display, From, Error)]
#[display(fmt = "RoyalRoad Error: {}")]
pub enum Error {
    UrlParseError(url::ParseError),
    ReqwestError(reqwest::Error),
    RssError(rss::Error),
    #[from(ignore)]
    #[display(fmt = "WebParseError: {}", "_0")]
    WebParseError(#[error(not(source))] String),
    #[from(ignore)]
    #[display(fmt = "UrlError: {}", "_0")]
    UrlError(#[error(not(source))] String),
    #[from(ignore)]
    #[display(fmt = "RssContentsError: {}", "_0")]
    RssContentsError(#[error(not(source))] String),
}
