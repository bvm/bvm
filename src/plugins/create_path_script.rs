use crate::types::ErrBox;
use std::path::Path;

#[cfg(unix)]
pub fn create_path_script(binary_name: &str, binaries_cache_dir: &Path) -> Result<(), ErrBox> {
    let file_path = binaries_cache_dir.join(format!("{}", binary_name));
    std::fs::write(
        &file_path,
        format!(
            r#"#!/bin/sh
exe_path=$(gvm resolve {})
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
pub fn create_path_script(binary_name: &str, binaries_cache_dir: &Path) -> Result<(), ErrBox> {
    // https://stackoverflow.com/a/6362922/188246
    let file_path = binaries_cache_dir.join(format!("{}.bat", binary_name));
    std::fs::write(
        &file_path,
        format!(
            r#"@ECHO OFF
FOR /F "tokens=* USEBACKQ" %%F IN (`gvm resolve {}`) DO (
  SET exe_path=%%F
)
%exe_path% %*"#,
            binary_name
        ),
    )?;
    Ok(())
}
