mod errors;
mod filters;
use crate::models::DeliveryMethod;
use crate::{calibre, smtp};
use crate::{connection_pool::PgConnectionManager, schema::delivery_methods};

use crate::schema::delivery_methods::dsl::*;

use chrono::{DateTime, Utc};
use diesel::{QueryDsl, RunQueryDsl};
use mobc::Pool;
use rand::Rng;
use serde::Deserialize;
use serde_json::Value;
use tracing::{span, Instrument, Level};
use uuid::Uuid;

pub use errors::Error;
pub use filters::get_filters;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidateKindleEmailRequest {
    user_id: String,
    verification_code: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AddKindleEmailRequest {
    user_id: String,
    kindle_email: String,
}

#[derive(Debug, AsChangeset, Insertable)]
#[table_name = "delivery_methods"]
struct AddKindleEmailChangeset {
    user_id: String,
    kindle_email: String,
    kindle_email_verified: bool,
    kindle_email_enabled: bool,
    kindle_email_verification_code_time: Option<DateTime<Utc>>,
    kindle_email_verification_code: Option<String>,
}

#[derive(Debug, AsChangeset, Insertable)]
#[table_name = "delivery_methods"]
struct VerifyKindleEmailChangeset {
    user_id: String,
    kindle_email: Option<String>,
    kindle_email_verified: bool,
    kindle_email_enabled: bool,
    kindle_email_verification_code_time: Option<DateTime<Utc>>,
    kindle_email_verification_code: Option<String>,
}

#[tracing::instrument(
name = "Validate kindle email.",
err,
level = "info"
skip(db_pool),
fields(
    request_id = %Uuid::new_v4(),
)
)]
pub async fn validate_kindle_email(
    request: ValidateKindleEmailRequest,
    db_pool: Pool<PgConnectionManager>,
) -> Result<serde_json::Map<String, Value>, Error> {
    let conn = db_pool
        .get()
        .instrument(tracing::info_span!("Acquiring a DB Connection."))
        .await?;
    let conn = conn.into_inner();

    let db_check_span = span!(Level::INFO, "Inserting or updating kindle email.");
    let delivery_method: DeliveryMethod = {
        let _a = db_check_span.enter();
        delivery_methods.find(&request.user_id).first(&conn)?
    };
    match (
        delivery_method.kindle_email_verification_code,
        delivery_method.kindle_email_verification_code_time,
    ) {
        (Some(code), Some(time)) => {
            if request.verification_code == code
                && (chrono::Utc::now() - time < chrono::Duration::minutes(5))
            {
                let db_span = span!(Level::INFO, "Inserting or updating kindle email.");
                let _ = {
                    let _a = db_span.enter();
                    let changeset = VerifyKindleEmailChangeset {
                        user_id: request.user_id.clone(),
                        kindle_email: delivery_method.kindle_email.clone(),
                        kindle_email_enabled: true,
                        kindle_email_verified: true,
                        kindle_email_verification_code_time: None,
                        kindle_email_verification_code: None,
                    };
                    let _result = diesel::insert_into(delivery_methods)
                        .values(&changeset)
                        .on_conflict(user_id)
                        .do_update()
                        .set(&changeset)
                        .execute(&conn)?;
                };
            } else {
                return Err(Error::Validation("Invalid code.".into()));
            }
        }
        _ => {
            return Err(Error::Validation(
                "User has no in-progress email validations.".into(),
            ))
        }
    };
    return Ok(serde_json::Map::new());
}

#[tracing::instrument(
name = "Add kindle email as a delivery option.",
err,
level = "info"
skip(db_pool),
fields(
    request_id = %Uuid::new_v4(),
)
)]
pub async fn register_kindle_email(
    request: AddKindleEmailRequest,
    db_pool: Pool<PgConnectionManager>,
) -> Result<serde_json::Map<String, Value>, Error> {
    let conn = db_pool
        .get()
        .instrument(tracing::info_span!("Acquiring a DB Connection."))
        .await?;
    let conn = conn.into_inner();

    let db_check_span = span!(Level::INFO, "Inserting or updating kindle email.");
    let _ = {
        let _a = db_check_span.enter();
        let code = rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(10)
            .map(char::from)
            .collect::<String>()
            .to_uppercase();
        let changeset = AddKindleEmailChangeset {
            user_id: request.user_id,
            kindle_email: request.kindle_email.clone(),
            kindle_email_enabled: false,
            kindle_email_verified: false,
            kindle_email_verification_code_time: Some(chrono::Utc::now()),
            kindle_email_verification_code: Some(code.clone()),
        };
        let _result = diesel::insert_into(delivery_methods)
            .values(&changeset)
            .on_conflict(user_id)
            .do_update()
            .set(&changeset)
            .execute(&conn)?;
        let mobi_bytes = calibre::generate_kindle_email_validation_mobi(&code)?;
        smtp::send_mobi_file(
            mobi_bytes.as_slice(),
            &request.kindle_email,
            "CerealValidation",
            "Cereal Kindle Email Validation",
        )
        .await?;
    };
    return Ok(serde_json::Map::new());
}
