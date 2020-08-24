use bytes::Bytes;
use reqwest::Client;

use crate::types::ErrBox;

pub async fn download_file(url: &str) -> Result<Bytes, ErrBox> {
    let client = Client::new();
    let resp = client.get(url).send().await?;
    Ok(resp.bytes().await?)
}
