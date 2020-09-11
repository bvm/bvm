use std::path::PathBuf;

pub fn get_system_path_dirs() -> Vec<PathBuf> {
    if let Some(path) = std::env::var_os("PATH") {
        std::env::split_paths(&path).collect() // todo: how to return an iterator?
    } else {
        Vec::new()
    }
}
