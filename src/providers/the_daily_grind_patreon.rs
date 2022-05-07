use std::env;

use anyhow::anyhow;
use anyhow::bail;
use anyhow::Result;
use chrono::DateTime;
use chrono::Utc;
use futures::future::join_all;
use itertools::Itertools;
use mailparse::MailHeaderMap;
use reqwest::Url;
use rusoto_core::credential::StaticProvider;
use rusoto_core::HttpClient;
use rusoto_core::Region;
use rusoto_s3::GetObjectRequest;
use rusoto_s3::ListObjectsV2Request;
use rusoto_s3::Object;
use rusoto_s3::S3Client;
use rusoto_s3::S3;
use tokio::io::AsyncReadExt;
use uuid::Uuid;

use crate::models::{BookKind, ChapterKind, NewBook, NewChapter};

pub fn get_book() -> NewBook {
    NewBook {
        name: "The Daily Grind".into(),
        author: "argusthecat".into(),
        metadata: BookKind::TheDailyGrindPatreon,
    }
}

#[tracing::instrument(
    name = "Checking for new patreon daily grind chapters.",
    ret,
    level = "info"
)]
pub async fn get_chapters(book_uuid: &Uuid) -> Result<Vec<NewChapter>> {
    let s3 = S3Client::new_with(
        HttpClient::new().expect("failed to create request dispatcher"),
        StaticProvider::new_minimal(
            env::var("AWS_ACCESS_KEY")?,
            env::var("AWS_SECRET_ACCESS_KEY")?,
        ),
        Region::default(),
    );
    let bucket = env::var("AWS_EMAIL_BUCKET")?;
    let objects = s3
        .list_objects_v2(ListObjectsV2Request {
            bucket: bucket.clone(),
            ..Default::default()
        })
        .await?;
    let chapters = objects
        .contents
        .ok_or_else(|| anyhow!("Object had no body."))?
        .into_iter()
        .map(|obj| get_chapter_meta(obj, &bucket, &s3, book_uuid));
    let chapters = join_all(chapters)
        .await
        .into_iter()
        .filter_map(|r| match r {
            Ok(c) => Some(c),
            Err(_) => None,
        })
        .collect_vec();
    Ok(chapters)
}

#[tracing::instrument(
    name = "Reading email files for new daily grind patreon chapters.",
    level = "info"
    skip(s3),
    ret
)]
async fn get_chapter_meta(
    s3_obj: Object,
    bucket_name: &str,
    s3: &S3Client,
    book_id: &Uuid,
) -> Result<NewChapter> {
    let chapter_object = s3
        .get_object(GetObjectRequest {
            bucket: bucket_name.to_owned(),
            key: s3_obj
                .key
                .ok_or_else(|| anyhow!("No key found on s3 object."))?,
            ..Default::default()
        })
        .await?;
    tracing::info!("Last modified at {:?}", chapter_object.last_modified);
    let published_at = chapter_object
        .last_modified
        .ok_or_else(|| anyhow!("No modification date on email s3 object."))?;
    let published_at: DateTime<Utc> = DateTime::parse_from_rfc2822(&published_at)?.into();
    tracing::info!("Published at {:?}", published_at);
    let mut chapter_bytes = Vec::new();
    chapter_object
        .body
        .ok_or_else(|| anyhow!("No body on s3 object."))?
        .into_async_read()
        .read_to_end(&mut chapter_bytes)
        .await?;
    let chapter_email = mailparse::parse_mail(&chapter_bytes)?;
    let subject = chapter_email.headers.get_first_value("Subject");
    match &subject {
        Some(x) => {
            if !x.to_lowercase().contains("daily grind") {
                bail!("Not a the daily grind Email")
            }
        }
        None => bail!("Not a the daily grind Email"),
    }
    let body = chapter_email.subparts.iter().last().map(|x| x.get_body());
    let body = match body {
        Some(body) => body?,
        None => bail!("No html body found."),
    };

    Ok(NewChapter {
        name: chapter_title_from_subject(&subject.unwrap())
            .ok_or_else(|| anyhow!("Failed to find chapter title from email subject"))?
            .into(),
        author: String::from("argusthecat"),
        book_id: *book_id,
        published_at,
        metadata: ChapterKind::TheDailyGrindPatreon { html: body },
    })
}

#[tracing::instrument(
    name = "Getting chapter name from link.",
    level = "info"
    ret
)]
fn chapter_title_from_subject(subject: &str) -> Option<&str> {
    subject.split('"').nth(1)
}

pub fn try_parse_url(url: &str) -> Result<()> {
    let request_url = Url::parse(url)?;
    match (request_url.scheme(), request_url.host_str()) {
        ("patreon", Some("thedailygrind.com")) => Ok(()),
        _ => Err(anyhow!("Not a patreon daily grind url.")),
    }
}
