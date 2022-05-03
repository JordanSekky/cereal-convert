use std::collections::HashMap;
use std::env;

use anyhow::anyhow;
use anyhow::bail;
use anyhow::Result;
use chrono::DateTime;
use chrono::Utc;
use futures::future::join_all;
use itertools::Itertools;
use mailparse::MailHeaderMap;
use reqwest::Method;
use reqwest::Url;
use rusoto_core::credential::StaticProvider;
use rusoto_core::HttpClient;
use rusoto_core::Region;
use rusoto_s3::GetObjectRequest;
use rusoto_s3::ListObjectsV2Request;
use rusoto_s3::Object;
use rusoto_s3::S3Client;
use rusoto_s3::S3;
use scraper::{Html, Selector};
use selectors::Element;
use tokio::io::AsyncReadExt;
use uuid::Uuid;

use crate::models::{BookKind, ChapterKind, NewBook, NewChapter};

pub fn get_book() -> NewBook {
    NewBook {
        name: "The Wandering Inn".into(),
        author: "Pirateaba".into(),
        metadata: BookKind::TheWanderingInnPatreon,
    }
}

#[tracing::instrument(
    name = "Checking for new patreon wandering inn chapters.",
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
    let chapters = objects.contents.map(|c| {
        c.into_iter()
            .map(|obj| get_chapter_metas(obj, &bucket, &s3, book_uuid))
    });
    match chapters {
        Some(chapters) => {
            let chapters = join_all(chapters)
                .await
                .into_iter()
                .filter_map(|x| match x {
                    Ok(chaps) => Some(chaps.into_iter()),
                    Err(_) => None,
                })
                .flatten()
                .collect_vec();
            Ok(chapters)
        }
        None => Ok(Vec::with_capacity(0)),
    }
}

#[tracing::instrument(
    name = "Reading email files for new wandering inn patreon chapters.",
    level = "info"
    skip(s3),
    ret
)]
async fn get_chapter_metas(
    s3_obj: Object,
    bucket_name: &str,
    s3: &S3Client,
    book_id: &Uuid,
) -> Result<Vec<NewChapter>> {
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
    match chapter_email.headers.get_first_value("Subject") {
        Some(x) => {
            if !x.to_lowercase().contains("pirateaba") {
                bail!("Not a Wandering Inn Email")
            }
        }
        None => bail!("Not a Wandering Inn Email"),
    }
    let body = chapter_email.subparts.iter().last().map(|x| x.get_body());
    let body = match body {
        Some(body) => body?,
        None => bail!("No html body found."),
    };
    tracing::info!("Found wandering inn patreon email with body: {}", body);
    let doc = Html::parse_document(&body);
    let para_tags_selector = Selector::parse("div > p").unwrap();

    let password = doc
        .select(&para_tags_selector)
        .filter(|x| x.text().any(|t| t.to_lowercase().contains("password")))
        .map(|x| x.next_sibling_element().map(|sib| sib.text().join("")))
        .exactly_one()
        .ok()
        .flatten();
    tracing::info!("Found password {:?}", password);

    let links_selector = Selector::parse("div > p a").unwrap();

    let chapters = doc
        .select(&links_selector)
        .filter_map(|x| x.value().attr("href").map(|y| (x.text().join(""), y)))
        .filter_map(|(href, link_text)| {
            Some(NewChapter {
                name: chapter_title_from_link(link_text)?.to_owned(),
                author: String::from("pirateaba"),
                book_id: *book_id,
                published_at,
                metadata: ChapterKind::TheWanderingInnPatreon {
                    url: href,
                    password: password.clone(),
                },
            })
        })
        .collect();

    Ok(chapters)
}

#[tracing::instrument(
    name = "Getting chapter name from link.",
    level = "info"
    ret
)]
fn chapter_title_from_link(link: &str) -> Option<&str> {
    link.split('/').filter(|x| !x.trim().is_empty()).last()
}

#[tracing::instrument(name = "Fetching chapter text from link.", level = "info")]
pub async fn get_chapter_body(link: &str, password: Option<&str>) -> Result<String> {
    let reqwest_client = reqwest::Client::builder().cookie_store(true).build()?;
    if let Some(password) = password {
        let mut form_data = HashMap::with_capacity(2);
        form_data.insert("post_password", password);
        form_data.insert("Submit", "Enter");
        let _password_submit_result = reqwest_client
            .request(Method::POST, "https://wanderinginn.com/wp-pass.php")
            .form(&form_data)
            .send()
            .await?;
    }
    let res = reqwest_client.get(link).send().await?.text().await?;
    let doc = Html::parse_document(&res);
    let chapter_body_elem_selector = Selector::parse("div.entry-content > *").unwrap();

    let body = doc
        .select(&chapter_body_elem_selector)
        .filter(|x| !x.text().any(|t| t == "Next Chapter"))
        .filter(|x| !x.text().any(|t| t == "Previous Chapter"))
        .map(|x| x.html())
        .join("\n");
    if body.trim().is_empty() {
        bail!("Failed to find chapter body.");
    }
    Ok(body)
}

pub fn try_parse_url(url: &str) -> Result<()> {
    let request_url = Url::parse(url)?;
    match (request_url.scheme(), request_url.host_str()) {
        ("patreon", Some("wanderinginn.com")) => Ok(()),
        _ => Err(anyhow!("Not a patreon wandering inn url.")),
    }
}
