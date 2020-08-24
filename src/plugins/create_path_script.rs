use std::path::Path;
use crate::types::ErrBox;

#[cfg(target_os="linux")]
#[cfg(target_os="macos")]
pub fn create_path_script(binary_name: &str, binaries_cache_dir: &Path) -> Result<(), ErrBox> {
    let file_path = binaries_cache_dir.join(format!("{}", binary_name));
    std::fs::write(&file_path, format!(r#"#!/bin/sh
gvm run {} "$@""#, binary_name))?;
    std::process::Command::new("chmod")
        .args(&["+x".to_string(), file_path.to_string_lossy().to_string()])
        .output()?;
    Ok(())
}

#[cfg(target_os="windows")]
pub fn create_path_script(binary_name: &str, binaries_cache_dir: &Path) -> Result<(), ErrBox> {
    let file_path = binaries_cache_dir.join(format!("{}.bat", binary_name));
    std::fs::write(&file_path, format!(r#"@ECHO OFF\r\ngvm run {} %*"#, binary_name))?;
    Ok(())
}
