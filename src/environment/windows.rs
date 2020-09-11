use std::path::Path;
use dprint_cli_core::types::ErrBox;

pub fn ensure_system_path(directory_path: &Path) -> Result<bool, ErrBox> {
    let directory_path = directory_path.to_string_lossy();
    use winreg::{enums::*, RegKey};

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (env, _) = hkcu.create_subkey("Environment")?;
    let mut path: String = env.get_value("Path")?;

    // add to the path if it doesn't have this entry
    if !path.split(";").any(|p| p == directory_path) {
        if !path.is_empty() {
            path.push_str(";")
        }
        path.push_str(&directory_path);
        env.set_value("Path", &path)?;
        Ok(true)
    } else {
        Ok(false)
    }
}
