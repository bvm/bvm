use std::path::PathBuf;

use crate::environment::Environment;

pub fn get_shim_dir(environment: &impl Environment) -> PathBuf {
    let user_data_dir = environment.get_user_data_dir(); // share across domains
    let bin_dir = user_data_dir.join("shims");
    environment.create_dir_all(&bin_dir).expect("should create the shim dir");
    bin_dir
}
