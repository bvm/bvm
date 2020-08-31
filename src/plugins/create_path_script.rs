use std::path::{Path, PathBuf};

use crate::environment::Environment;
use crate::types::{CommandName, ErrBox};

#[cfg(unix)]
pub fn create_path_script(
    environment: &impl Environment,
    binaries_cache_dir: &Path,
    command_name: &CommandName,
) -> Result<(), ErrBox> {
    let file_path = get_path_script_path(binaries_cache_dir, command_name);
    environment.write_file_text(
        &file_path,
        &format!(
            r#"#!/bin/sh
exe_path=$(bvm resolve {})
"$exe_path" "$@""#,
            command_name.as_str()
        ),
    )?;
    std::process::Command::new("chmod")
        .args(&["+x".to_string(), file_path.to_string_lossy().to_string()])
        .output()?;
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn create_path_script(
    environment: &impl Environment,
    binaries_cache_dir: &Path,
    command_name: &CommandName,
) -> Result<(), ErrBox> {
    // https://stackoverflow.com/a/6362922/188246
    // todo: needs to handle when this fails to find the binary or something
    let file_path = get_path_script_path(binaries_cache_dir, command_name);
    environment.write_file_text(
        &file_path,
        &format!(
            r#"@ECHO OFF
FOR /F "tokens=* USEBACKQ" %%F IN (`bvm resolve {}`) DO (
  SET exe_path=%%F
)
"%exe_path%" %*"#,
            command_name.as_str()
        ),
    )?;
    Ok(())
}

pub fn get_path_script_path(binaries_cache_dir: &Path, command_name: &CommandName) -> PathBuf {
    #[cfg(target_os = "windows")]
    return binaries_cache_dir.join(format!("{}.bat", command_name.as_str()));
    #[cfg(unix)]
    return binaries_cache_dir.join(format!("{}", command_name.as_str()));
}
