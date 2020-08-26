use crate::types::ErrBox;
use std::path::{Path, PathBuf};

#[cfg(unix)]
pub fn create_path_script(binaries_cache_dir: &Path, binary_name: &str) -> Result<(), ErrBox> {
    let file_path = get_path_script_path(binaries_cache_dir, binary_name);
    std::fs::write(
        &file_path,
        format!(
            r#"#!/bin/sh
exe_path=$(bvm resolve {})
$exe_path "$@""#,
            binary_name
        ),
    )?;
    std::process::Command::new("chmod")
        .args(&["+x".to_string(), file_path.to_string_lossy().to_string()])
        .output()?;
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn create_path_script(binaries_cache_dir: &Path, binary_name: &str) -> Result<(), ErrBox> {
    // https://stackoverflow.com/a/6362922/188246
    // todo: needs to handle when this fails to find the binary or something
    let file_path = get_path_script_path(binaries_cache_dir, binary_name);
    std::fs::write(
        &file_path,
        format!(
            r#"@ECHO OFF
FOR /F "tokens=* USEBACKQ" %%F IN (`bvm resolve {}`) DO (
  SET exe_path=%%F
)
%exe_path% %*"#,
            binary_name
        ),
    )?;
    Ok(())
}

pub fn get_path_script_path(binaries_cache_dir: &Path, binary_name: &str) -> PathBuf {
    #[cfg(target_os = "windows")]
    return binaries_cache_dir.join(format!("{}.bat", binary_name));
    #[cfg(unix)]
    return binaries_cache_dir.join(format!("{}", binary_name));
}
