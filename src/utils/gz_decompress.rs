use crate::types::ErrBox;
use flate2::read::GzDecoder;
use std::io::prelude::*;

pub fn gz_decompress(bytes: &[u8]) -> Result<Vec<u8>, ErrBox> {
    let mut d = GzDecoder::new(bytes);
    let mut final_bytes = Vec::new();
    d.read_to_end(&mut final_bytes)?;
    Ok(final_bytes)
}
