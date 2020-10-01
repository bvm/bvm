use std::path::PathBuf;

use crate::environment::Environment;
use crate::types::{BinaryName, Version};

pub fn get_plugin_dir(
    environment: &impl Environment,
    binary_name: &BinaryName,
    version: &Version,
) -> PathBuf {
    let local_data_dir = environment.get_local_user_data_dir(); // do not share across domains
    local_data_dir.join(get_plugin_dir_relative_local_user_data(binary_name, version))
}

pub fn get_plugin_dir_relative_local_user_data(binary_name: &BinaryName, version: &Version) -> PathBuf {
    PathBuf::from("binaries")
        .join(&binary_name.owner)
        .join(binary_name.name.as_str())
        .join(version.as_str())
}
