mod filters;
use crate::models::DeliveryMethod;
use crate::schema::delivery_methods;
use crate::util::InstrumentedPgConnectionPool;
use crate::{calibre, mailgun, pushover};

use crate::schema::delivery_methods::dsl::*;

use anyhow::Result;
use anyhow::{anyhow, bail};
use chrono::{DateTime, Utc};
use diesel::{QueryDsl, RunQueryDsl};
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

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

#[derive(Debug, Deserialize)]
pub struct GetDeliveryMethodsRequest {
    user_id: String,
}

#[derive(Debug, AsChangeset, Insertable)]
#[table_name = "delivery_methods"]
#[changeset_options(treat_none_as_null = "true")]
struct KindleEmailChangeset {
    user_id: String,
    kindle_email: String,
    kindle_email_verified: bool,
    kindle_email_enabled: bool,
    kindle_email_verification_code_time: Option<DateTime<Utc>>,
    kindle_email_verification_code: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct GetDeliveryMethodsResponse {
    kindle_email: Option<String>,
    pushover_key: Option<String>,
}

#[tracing::instrument(
name = "Get Validated Delivery Methods",
err,
level = "info"
skip(db_pool),
fields(
    request_id = %Uuid::new_v4(),
)
)]
pub async fn get_delivery_methods(
    request: GetDeliveryMethodsRequest,
    db_pool: InstrumentedPgConnectionPool,
) -> Result<GetDeliveryMethodsResponse> {
    let delivery_method: DeliveryMethod = {
        let conn = db_pool.get().await?;
        delivery_methods.find(&request.user_id).first(&*conn)?
    };
    let kindle = if delivery_method.kindle_email_enabled && delivery_method.kindle_email_verified {
        delivery_method.get_kindle_email().clone()
    } else {
        None
    };
    let pushover = if delivery_method.pushover_enabled && delivery_method.pushover_key_verified {
        delivery_method.get_pushover_key().clone()
    } else {
        None
    };
    Ok(GetDeliveryMethodsResponse {
        kindle_email: kindle,
        pushover_key: pushover,
    })
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
    db_pool: InstrumentedPgConnectionPool,
) -> Result<serde_json::Map<String, Value>> {
    let delivery_method: DeliveryMethod = {
        let conn = db_pool.get().await?;
        delivery_methods.find(&request.user_id).first(&*conn)?
    };
    match (
        delivery_method.kindle_email_verification_code,
        delivery_method.kindle_email_verification_code_time,
    ) {
        (Some(code), Some(time)) => {
            if request.verification_code == code
                && (chrono::Utc::now() - time < chrono::Duration::hours(1))
            {
                let _ = {
                    let changeset = KindleEmailChangeset {
                        user_id: request.user_id.clone(),
                        kindle_email: delivery_method.kindle_email.ok_or_else(|| {
                            anyhow!("No kindle email defined in delivery method.")
                        })?,
                        kindle_email_enabled: true,
                        kindle_email_verified: true,
                        kindle_email_verification_code_time: None,
                        kindle_email_verification_code: None,
                    };
                    let conn = db_pool.get().await?;
                    let _result = diesel::insert_into(delivery_methods)
                        .values(&changeset)
                        .on_conflict(user_id)
                        .do_update()
                        .set(&changeset)
                        .execute(&*conn)?;
                };
            } else {
                bail!("User provided the incorrect validation code.");
            }
        }
        _ => {
            bail!("User has no in-progress email validations.");
        }
    };
    Ok(serde_json::Map::new())
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
    db_pool: InstrumentedPgConnectionPool,
) -> Result<serde_json::Map<String, Value>> {
    // Assert email domain is "kindle.com". Emails aren't free.
    let email = addr::parse_email_address(&request.kindle_email)
        .map_err(|err| anyhow!("Failed to parse email address. Err: {:?}", err))?;
    match email.host() {
        addr::email::Host::Domain(hostname) => match hostname.as_str() {
            "kindle.com" => (),
            _ => bail!("Provided email hostname {} is not kindle.com", hostname),
        },
        addr::email::Host::IpAddr(hostname) => {
            bail!("Provided email hostname {:?} is not kindle.com", hostname)
        }
    }

    let _ = {
        let code = rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(10)
            .map(char::from)
            .collect::<String>()
            .to_uppercase();
        let changeset = KindleEmailChangeset {
            user_id: request.user_id,
            kindle_email: request.kindle_email.clone(),
            kindle_email_enabled: false,
            kindle_email_verified: false,
            kindle_email_verification_code_time: Some(chrono::Utc::now()),
            kindle_email_verification_code: Some(code.clone()),
        };
        let conn = db_pool.get().await?;
        let _result = diesel::insert_into(delivery_methods)
            .values(&changeset)
            .on_conflict(user_id)
            .do_update()
            .set(&changeset)
            .execute(&*conn)?;
        let mobi_bytes = calibre::generate_kindle_email_validation_mobi(&code).await?;
        mailgun::send_mobi_file(
            mobi_bytes.as_slice(),
            &request.kindle_email,
            "CerealValidation",
            "Cereal Kindle Email Validation",
        )
        .await?;
    };
    Ok(serde_json::Map::new())
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidatePushoverRequest {
    user_id: String,
    verification_code: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AddPushoverRequest {
    user_id: String,
    pushover_key: String,
}

#[derive(Debug, AsChangeset, Insertable)]
#[table_name = "delivery_methods"]
#[changeset_options(treat_none_as_null = "true")]
struct PushoverChangeset {
    user_id: String,
    pushover_key: String,
    pushover_key_verified: bool,
    pushover_enabled: bool,
    pushover_verification_code_time: Option<DateTime<Utc>>,
    pushover_verification_code: Option<String>,
}

#[tracing::instrument(
name = "Validate pushover token.",
err,
level = "info"
skip(db_pool),
fields(
    request_id = %Uuid::new_v4(),
)
)]
pub async fn validate_pushover_key(
    request: ValidatePushoverRequest,
    db_pool: InstrumentedPgConnectionPool,
) -> Result<serde_json::Map<String, Value>> {
    let delivery_method: DeliveryMethod = {
        let conn = db_pool.get().await?;
        delivery_methods.find(&request.user_id).first(&*conn)?
    };
    match (
        delivery_method.pushover_verification_code,
        delivery_method.pushover_verification_code_time,
    ) {
        (Some(code), Some(time)) => {
            if request.verification_code == code
                && (chrono::Utc::now() - time < chrono::Duration::minutes(5))
            {
                let _ = {
                    let changeset = PushoverChangeset {
                        user_id: request.user_id.clone(),
                        pushover_key: delivery_method.pushover_key.ok_or_else(|| {
                            anyhow!("No pushover key defined in delivery method.")
                        })?,
                        pushover_enabled: true,
                        pushover_key_verified: true,
                        pushover_verification_code_time: None,
                        pushover_verification_code: None,
                    };
                    let conn = db_pool.get().await?;
                    let _result = diesel::insert_into(delivery_methods)
                        .values(&changeset)
                        .on_conflict(user_id)
                        .do_update()
                        .set(&changeset)
                        .execute(&*conn)?;
                };
            } else {
                bail!("User provided the incorrect validation code.");
            }
        }
        _ => {
            bail!("User has no in-progress pushover validations.");
        }
    };
    Ok(serde_json::Map::new())
}

#[tracing::instrument(
name = "Add pushover key.",
err,
level = "info"
skip(db_pool),
fields(
    request_id = %Uuid::new_v4(),
)
)]
pub async fn register_pushover_key(
    request: AddPushoverRequest,
    db_pool: InstrumentedPgConnectionPool,
) -> Result<serde_json::Map<String, Value>> {
    let code = rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(10)
        .map(char::from)
        .collect::<String>()
        .to_uppercase();
    let _ = {
        let changeset = PushoverChangeset {
            user_id: request.user_id,
            pushover_key: request.pushover_key.clone(),
            pushover_enabled: false,
            pushover_key_verified: false,
            pushover_verification_code_time: Some(chrono::Utc::now()),
            pushover_verification_code: Some(code.clone()),
        };
        let conn = db_pool.get().await?;
        let _result = diesel::insert_into(delivery_methods)
            .values(&changeset)
            .on_conflict(user_id)
            .do_update()
            .set(&changeset)
            .execute(&*conn)?;
    };
    pushover::send_verification_token(&request.pushover_key, &code.clone()).await?;
    Ok(serde_json::Map::new())
}
