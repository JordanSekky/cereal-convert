pub use errors::Error;

use rand::Rng;
use rusoto_core::{credential::StaticProvider, HttpClient, Region};
use rusoto_s3::{GetObjectRequest, PutObjectRequest, S3Client, S3Location, S3};
use std::env;

pub async fn store_book(mobi_bytes: Vec<u8>) -> Result<S3Location, Error> {
    let s3 = S3Client::new_with(
        HttpClient::new().expect("failed to create request dispatcher"),
        StaticProvider::new_minimal(
            env::var("CEREAL_SPACES_KEY")?,
            env::var("CEREAL_SPACES_SECRET")?,
        ),
        Region::Custom {
            name: "SPACES".to_string(),
            endpoint: env::var("CEREAL_SPACES_ENDPOINT")?,
        },
    );
    let file_name: String = rand::thread_rng()
        .sample_iter(rand::distributions::Alphanumeric)
        .take(30)
        .map(char::from)
        .collect();
    let key = file_name + ".mobi";
    let bucket = env::var("CEREAL_SPACES_NAME")?;
    s3.put_object(PutObjectRequest {
        bucket: bucket.clone(),
        key: key.clone(),
        body: Some(mobi_bytes.into()),
        ..Default::default()
    })
    .await?;
    Ok(S3Location {
        prefix: key,
        bucket_name: bucket,
        ..Default::default()
    })
}

pub async fn fetch_book(location: &S3Location) -> Result<Vec<u8>, Error> {
    let s3 = S3Client::new_with(
        HttpClient::new().expect("failed to create request dispatcher"),
        StaticProvider::new_minimal(
            env::var("CEREAL_SPACES_KEY")?,
            env::var("CEREAL_SPACES_SECRET")?,
        ),
        Region::Custom {
            name: "SPACES".to_string(),
            endpoint: env::var("CEREAL_SPACES_ENDPOINT")?,
        },
    );
    let response = s3
        .get_object(GetObjectRequest {
            bucket: location.bucket_name.clone(),
            key: location.prefix.clone(),
            ..Default::default()
        })
        .await?;
    let body_len_bytes = response.content_length.unwrap_or(0);
    let body_len_bytes = usize::try_from(body_len_bytes).unwrap_or(0);
    let bytes = match response.body {
        Some(body) => {
            use tokio::io::AsyncReadExt;
            let mut out = Vec::with_capacity(body_len_bytes);
            body.into_async_read().read_to_end(&mut out).await?;
            out
        }
        None => Vec::with_capacity(0),
    };
    Ok(bytes)
}

mod errors {
    use derive_more::{Display, Error, From};

    #[derive(Debug, Display, Error, From)]
    pub enum Error {
        EnvironmentCredentials(std::env::VarError),
        PutObject(rusoto_core::RusotoError<rusoto_s3::PutObjectError>),
        GetObject(rusoto_core::RusotoError<rusoto_s3::GetObjectError>),
        IO(std::io::Error),
    }
}
