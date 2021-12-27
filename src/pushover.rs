use std::{collections::HashMap, env};

pub use errors::Error;

pub async fn send_verification_token(user_code: &str, code: &str) -> Result<(), Error> {
    let application_key =
        env::var("CEREAL_PUSHOVER_TOKEN").expect("Pushover app token not provided.");
    let client = reqwest::Client::default();
    let mut map = HashMap::new();
    map.insert("token", application_key);
    map.insert("user", user_code.into());
    map.insert("message", format!("Thank you for using cereal. Please use the following code to validate your pushover token: {}", code));
    let response = client
        .post("https://api.pushover.net/1/messages.json")
        .json(&map)
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(Error::NotificationServiceFailure(response.status()));
    }
    Ok(())
}

mod errors {
    use std::fmt::Display;

    #[derive(Debug)]
    pub enum Error {
        NotificationServiceFailure(reqwest::StatusCode),
        RequestFailure(reqwest::Error),
    }

    impl Display for Error {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_fmt(format_args!("{:?}", self))
        }
    }

    impl std::error::Error for Error {}

    impl From<reqwest::Error> for Error {
        fn from(x: reqwest::Error) -> Self {
            Error::RequestFailure(x)
        }
    }
}
