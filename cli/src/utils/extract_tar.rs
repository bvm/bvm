use dprint_cli_core::types::ErrBox;
use std::io::prelude::*;
use std::path::Path;

use crate::environment::Environment;
use tar::Archive;

pub fn extract_tar_gz(
  message: &str,
  environment: &impl Environment,
  tar_gz_bytes: &[u8],
  dir_path: &Path,
) -> Result<(), ErrBox> {
  let tar_bytes = super::gz_decompress(&tar_gz_bytes)?;
  extract_tar(message, environment, &tar_bytes, dir_path)
}

pub fn extract_tar(
  message: &str,
  environment: &impl Environment,
  tar_bytes: &[u8],
  dir_path: &Path,
) -> Result<(), ErrBox> {
  let length = tar_bytes.len();

  environment.log_action_with_progress(
    message,
    move |update_size| -> Result<(), ErrBox> {
      let reader = std::io::Cursor::new(tar_bytes);
      let mut a = Archive::new(reader);
      let mut position = 0;
      for entry in a.entries()? {
        let mut entry = entry?;
        let file_path = dir_path.join(entry.path()?);
        if environment.is_real() {
          if cfg!(unix) {
            entry.set_preserve_permissions(true);
          }

          entry.unpack_in(&dir_path)?;
        } else {
          let mut bytes = Vec::new();
          entry.read_to_end(&mut bytes)?;
          environment.write_file(&file_path, &bytes)?;
        }
        position += entry.size();
        update_size(position as usize);
      }
      Ok(())
    },
    length,
  )
}
