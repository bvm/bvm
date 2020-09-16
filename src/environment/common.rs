use std::path::PathBuf;

pub(super) fn get_system_path_dirs() -> Vec<PathBuf> {
    if let Some(path) = std::env::var_os("PATH") {
        std::env::split_paths(&path).collect() // todo: how to return an iterator?
    } else {
        Vec::new()
    }
}

pub const PATH_SEPARATOR: &'static str = if cfg!(target_os = "windows") { "\\" } else { "/" };
/// The separator used for the system path
pub const SYS_PATH_DELIMITER: &'static str = if cfg!(target_os = "windows") { ";" } else { ":" };
