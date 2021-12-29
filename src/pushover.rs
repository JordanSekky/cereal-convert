use std::{collections::HashMap, env};

pub use errors::Error;

pub async fn send_verification_token(user_code: &str, code: &str) -> Result<(), Error> {
    let message = format!("Thank you for using cereal. Please use the following code to validate your pushover token: {}", code);
    return send_message(user_code, &message).await;
}

pub async fn send_message(user_code: &str, message: &str) -> Result<(), Error> {
    let application_key =
        env::var("CEREAL_PUSHOVER_TOKEN").expect("Pushover app token not provided.");
    let client = reqwest::Client::default();
    let mut map = HashMap::new();
    map.insert("token", application_key);
    map.insert("user", user_code.into());
    map.insert("message", message.into());
    let response = client
        .post("https://api.pushover.net/1/messages.json")
        .json(&map)
        .send()
        .await?
        .error_for_status();
    match response {
        Ok(_) => Ok(()),
        Err(status) => Err(Error::NotificationServiceFailure(status)),
    }
}

mod errors {
    use derive_more::{Display, Error, From};

    #[derive(Debug, Display, From, Error)]
    pub enum Error {
        #[from(ignore)]
        NotificationServiceFailure(reqwest::Error),
        RequestFailure(reqwest::Error),
    }
}
