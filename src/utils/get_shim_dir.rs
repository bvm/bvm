use std::path::PathBuf;

use crate::environment::Environment;
use crate::types::ErrBox;

pub fn get_shim_dir(environment: &impl Environment) -> Result<PathBuf, ErrBox> {
    let user_data_dir = environment.get_user_data_dir()?;
    let bin_dir = user_data_dir.join("shims");
    environment.create_dir_all(&bin_dir)?;
    Ok(bin_dir)
}
