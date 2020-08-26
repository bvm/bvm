use crate::types::ErrBox;
use std::path::PathBuf;

const APP_INFO: app_dirs::AppInfo = app_dirs::AppInfo {
    name: "bvm",
    author: "bvm",
};

pub fn get_user_data_dir() -> Result<PathBuf, ErrBox> {
    match app_dirs::app_root(app_dirs::AppDataType::UserData, &APP_INFO) {
        Ok(path) => Ok(path),
        Err(err) => err!("Error getting user data directory: {:?}", err),
    }
}
