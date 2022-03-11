use derive_more::{Display, Error, From};

#[derive(Debug, Display, From, Error)]
#[display(fmt = "RoyalRoad Error: {}")]
pub enum Error {
    UrlParse(url::ParseError),
    Reqwest(reqwest::Error),
    Rss(rss::Error),
    #[from(ignore)]
    #[display(fmt = "WebParseError: {}", "_0")]
    WebParse(#[error(not(source))] String),
    #[from(ignore)]
    #[display(fmt = "UrlError: {}", "_0")]
    Url(#[error(not(source))] String),
    #[from(ignore)]
    #[display(fmt = "RssContentsError: {}", "_0")]
    RssContents(#[error(not(source))] String),
}
