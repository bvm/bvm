use std::path::{Path, PathBuf};
use crate::types::ErrBox;

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

fn get_config_file_in_dir(dir: &Path) -> Option<PathBuf> {
    let file_name = ".gvmrc.json";
    let config_path = dir.join(file_name);
    if config_path.exists() {
        return Some(config_path);
    }
    let config_path = dir.join(format!("config/{}", file_name));
    if config_path.exists() {
        return Some(config_path);
    }
    None
}