use std::path::Path;

use dprint_cli_core::checksums::parse_checksum_path_or_url;
use dprint_cli_core::types::ErrBox;
use url::Url;

#[derive(Clone)]
pub struct ChecksumUrl {
  pub unresolved_path: String,
  pub url: Url,
  pub checksum: Option<String>,
}

impl ChecksumUrl {
  pub fn from_path_and_checksum(path: &str, checksum: String, base: &Url) -> Result<Self, ErrBox> {
    Ok(ChecksumUrl {
      unresolved_path: path.to_string(),
      url: parse_path_or_url_to_url(&path, base)?,
      checksum: Some(checksum),
    })
  }

  pub fn with_checksum(&self, checksum: String) -> ChecksumUrl {
    ChecksumUrl {
      url: self.url.clone(),
      checksum: Some(checksum),
      unresolved_path: self.unresolved_path.clone(),
    }
  }
}

pub fn parse_path_or_url_to_url(text: &str, base: &Url) -> Result<Url, ErrBox> {
  Ok(Url::parse(&text).or_else(|_| base.join(text))?)
}

pub fn parse_checksum_url(text: &str, base: &Url) -> Result<ChecksumUrl, ErrBox> {
  let checksum_path_or_url = parse_checksum_path_or_url(text);
  Ok(ChecksumUrl {
    unresolved_path: checksum_path_or_url.path_or_url.clone(),
    url: parse_path_or_url_to_url(&checksum_path_or_url.path_or_url, base)?,
    checksum: checksum_path_or_url.checksum,
  })
}

pub fn get_url_from_directory(dir: impl AsRef<Path>) -> Url {
  if cfg!(windows) && dir.as_ref().to_string_lossy().starts_with("/") {
    // should only happen in testing...
    Url::parse(&format!("file://{}", dir.as_ref().to_string_lossy().replace("\\", "/"))).unwrap()
  } else {
    Url::from_directory_path(dir).unwrap()
  }
}
