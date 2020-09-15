use std::env;
use std::path::PathBuf;

fn main() {
    let install_location = env::args().collect::<Vec<_>>().pop().unwrap();
    let app_dir = std::env::var("APPDATA").expect("Expected to get the app data environment variable.");
    prepend_to_path(&PathBuf::from(install_location).join("bin").to_string_lossy());
    prepend_to_path(&PathBuf::from(app_dir).join("bvm").join("shims").to_string_lossy());
}

fn prepend_to_path(directory_path: &str) {
    use winreg::{enums::*, RegKey};

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (env, _) = hkcu
        .create_subkey("Environment")
        .expect("Expected to get Environment sub key.");
    let mut path: String = env
        .get_value("Path")
        .expect("Expected to get path environment variable.");

    // add to the path if it doesn't have this entry
    if !path.split(";").any(|p| p == directory_path) {
        if !path.is_empty() && !path.starts_with(';') {
            path = format!(";{}", path);
        }
        path = format!("{}{}", directory_path, path);
        env.set_value("Path", &path).expect("Expected to set path.");
    }
}
