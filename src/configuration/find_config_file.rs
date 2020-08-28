use crate::types::ErrBox;
use std::path::{Path, PathBuf};

pub fn find_config_file() -> Result<Option<PathBuf>, ErrBox> {
    let cwd = std::env::current_dir()?;

    if let Some(config_file_path) = get_config_file_in_dir(&cwd) {
        return Ok(Some(config_file_path));
    }

    for ancestor_dir in cwd.ancestors() {
        if let Some(config_file_path) = get_config_file_in_dir(ancestor_dir) {
            return Ok(Some(config_file_path));
        }
    }

    Ok(None)
}

pub const CONFIG_FILE_NAME: &'static str = ".bvmrc.json";

fn get_config_file_in_dir(dir: &Path) -> Option<PathBuf> {
    let config_path = dir.join(CONFIG_FILE_NAME);
    if config_path.exists() {
        return Some(config_path);
    }
    let config_path = dir.join(format!("config/{}", CONFIG_FILE_NAME));
    if config_path.exists() {
        return Some(config_path);
    }
    None
}
