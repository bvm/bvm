use std::path::PathBuf;
use crate::types::ErrBox;

const APP_INFO: app_dirs::AppInfo = app_dirs::AppInfo { name: "gvm", author: "gvm" };

pub fn get_cache_dir() -> Result<PathBuf, ErrBox> {
    match app_dirs::app_dir(app_dirs::AppDataType::UserCache, &APP_INFO, "cache") {
        Ok(path) => Ok(path),
        Err(err) => err!("Error getting cache directory: {:?}", err),
    }
}