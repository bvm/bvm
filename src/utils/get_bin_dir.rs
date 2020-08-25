use std::path::PathBuf;

use super::get_user_data_dir;
use crate::types::ErrBox;

pub fn get_bin_dir() -> Result<PathBuf, ErrBox> {
    let user_data_dir = get_user_data_dir()?;
    let bin_dir = user_data_dir.join("bin");
    std::fs::create_dir_all(&bin_dir)?;
    Ok(bin_dir)
}
