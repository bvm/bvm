pub struct ChecksumUrl {
    pub url: String,
    pub checksum: Option<String>,
}

pub fn parse_checksum_url(text: &str) -> ChecksumUrl {
    match text.rfind('@') {
        Some(index) => ChecksumUrl {
            url: text[..index].to_string(),
            checksum: Some(text[index + 1..].to_string()),
        },
        None => ChecksumUrl {
            url: text.to_string(),
            checksum: None,
        },
    }
}
