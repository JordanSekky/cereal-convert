use derive_more::From;
use serde::Serialize;

#[derive(Serialize, From)]
pub struct ErrorMessage {
    pub message: String,
}

impl From<&str> for ErrorMessage {
    fn from(x: &str) -> Self {
        x.to_owned().into()
    }
}
