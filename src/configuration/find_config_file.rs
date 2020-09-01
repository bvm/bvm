use dprint_cli_core::types::ErrBox;
use std::path::{Path, PathBuf};

use crate::environment::Environment;

pub fn find_config_file(environment: &impl Environment) -> Result<Option<PathBuf>, ErrBox> {
    let cwd = environment.cwd()?;

    if let Some(config_file_path) = get_config_file_in_dir(environment, &cwd) {
        return Ok(Some(config_file_path));
    }

    for ancestor_dir in cwd.ancestors() {
        if let Some(config_file_path) = get_config_file_in_dir(environment, ancestor_dir) {
            return Ok(Some(config_file_path));
        }
    }

    Ok(None)
}

pub const CONFIG_FILE_NAME: &'static str = ".bvmrc.json";

fn get_config_file_in_dir(environment: &impl Environment, dir: &Path) -> Option<PathBuf> {
    let config_path = dir.join(CONFIG_FILE_NAME);
    if environment.path_exists(&config_path) {
        return Some(config_path);
    }
    // I'm not sure if this complexity should exist. It also slows down file resolution.
    /*
    let config_path = dir.join(format!("config/{}", CONFIG_FILE_NAME));
    if environment.path_exists(&config_path) {
        return Some(config_path);
    }
    */
    None
}
