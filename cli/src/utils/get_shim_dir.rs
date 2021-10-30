use std::path::PathBuf;

use crate::environment::Environment;

pub fn get_shim_dir(environment: &impl Environment) -> PathBuf {
  let install_dir = environment.get_env_var("BVM_INSTALL_DIR");
  let root_dir = if cfg!(windows) || install_dir.is_none() {
    environment.get_user_data_dir() // share across domains
  } else {
    PathBuf::from(install_dir.unwrap())
  };
  root_dir.join("shims")
}
