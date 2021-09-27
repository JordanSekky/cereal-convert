use std::error::Error;

use rand::Rng;
use rusoto_core::{credential::StaticProvider, HttpClient, Region};
use rusoto_s3::{PutObjectRequest, S3Client, S3};
use std::env;

pub struct StorageLocation {
    pub key: String,
    pub bucket: String,
}

pub async fn store_book(mobi_bytes: Vec<u8>) -> Result<StorageLocation, Box<dyn Error>> {
    let s3 = S3Client::new_with(
        HttpClient::new().expect("failed to create request dispatcher"),
        StaticProvider::new_minimal(
            env::var("CEREAL_SPACES_KEY")?.to_string(),
            env::var("CEREAL_SPACES_SECRET")?.to_string(),
        ),
        Region::Custom {
            name: "SPACES".to_string(),
            endpoint: env::var("CEREAL_SPACES_ENDPOINT")?.to_string(),
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
    Ok(StorageLocation { key, bucket })
}
