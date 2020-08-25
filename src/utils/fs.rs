use crate::types::ErrBox;
use std::path::Path;

pub fn is_dir_empty(path: &Path) -> Result<bool, ErrBox> {
    Ok(std::fs::read_dir(path)?.next().is_none())
}
