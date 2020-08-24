use std::path::PathBuf;
use crate::types::ErrBox;

const APP_INFO: app_dirs::AppInfo = app_dirs::AppInfo { name: "gvm", author: "gvm" };

pub fn get_user_data_dir() -> Result<PathBuf, ErrBox> {
    match app_dirs::app_root(app_dirs::AppDataType::UserData, &APP_INFO) {
        Ok(path) => Ok(path),
        Err(err) => err!("Error getting user data directory: {:?}", err),
    }
}