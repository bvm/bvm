use dprint_cli_core::types::ErrBox;
use std::path::PathBuf;

use crate::environment::Environment;
use crate::types::CommandName;
use crate::utils;

#[cfg(unix)]
pub(super) fn create_shim(environment: &impl Environment, command_name: &CommandName) -> Result<(), ErrBox> {
    let file_path = get_shim_path(environment, command_name);
    environment.write_file_text(
        &file_path,
        &format!(
            r#"#!/bin/sh
exe_path=$(bvm-bin resolve {})
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
pub(super) fn create_shim(environment: &impl Environment, command_name: &CommandName) -> Result<(), ErrBox> {
    // https://stackoverflow.com/a/6362922/188246
    // todo: needs to handle when this fails to find the binary or something
    let file_path = get_shim_path(environment, command_name);
    environment.write_file_text(
        &file_path,
        &format!(
            r#"@ECHO OFF
FOR /F "tokens=* USEBACKQ" %%F IN (`bvm-bin resolve {}`) DO (
  SET exe_path=%%F
)
"%exe_path%" %*"#,
            command_name.as_str()
        ),
    )
}

pub fn get_shim_path(environment: &impl Environment, command_name: &CommandName) -> PathBuf {
    let shim_dir = utils::get_shim_dir(environment);
    #[cfg(target_os = "windows")]
    return shim_dir.join(format!("{}.bat", command_name.as_str()));
    #[cfg(unix)]
    return shim_dir.join(format!("{}", command_name.as_str()));
}
