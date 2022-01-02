use derive_more::{Display, Error, From};

use crate::{models, royalroad};

#[derive(Debug, Display, Error, From)]
pub enum Error {
    EstablishConnection(mobc::Error<diesel::ConnectionError>),
    QueryResult(diesel::result::Error),
    RoyalRoadError(royalroad::Error),
    MetadataParse(#[error(not(source))] String),
    GatherBookMetadata(models::BookKindToNewBookError),
}
