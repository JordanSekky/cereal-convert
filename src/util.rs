use serde::Serialize;

#[derive(Serialize)]
pub struct ErrorMessage {
    pub message: String,
}
