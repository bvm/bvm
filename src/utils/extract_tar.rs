use std::io::prelude::*;
use std::path::Path;

use crate::environment::Environment;
use crate::types::ErrBox;
use tar::Archive;

pub fn extract_tar_gz(environment: &impl Environment, tar_gz_bytes: &[u8], dir_path: &Path) -> Result<(), ErrBox> {
    let tar_bytes = super::gz_decompress(&tar_gz_bytes)?;
    extract_tar(environment, &tar_bytes, dir_path)
}

pub fn extract_tar(environment: &impl Environment, tar_bytes: &[u8], dir_path: &Path) -> Result<(), ErrBox> {
    let reader = std::io::Cursor::new(&tar_bytes);
    let mut a = Archive::new(reader);

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
    }

    Ok(())
}
