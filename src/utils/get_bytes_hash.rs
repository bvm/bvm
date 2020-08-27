use crate::types::ErrBox;

pub fn get_sha256_checksum(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

pub fn verify_sha256_checksum(bytes: &[u8], checksum: &str) -> Result<(), ErrBox> {
    let bytes_checksum = get_sha256_checksum(bytes);
    if bytes_checksum != checksum {
        err!(
            "The checksum {} did not match the expected checksum of {}.",
            bytes_checksum,
            checksum
        )
    } else {
        Ok(())
    }
}
