use dprint_cli_core::types::ErrBox;
use std::path::Path;
use std::path::PathBuf;

use crate::environment::Environment;
use crate::types::CommandName;
use crate::utils;

#[cfg(unix)]
pub fn create_shim(environment: &impl Environment, command_name: &CommandName, command_path: &Path) -> Result<(), ErrBox> {
    let shim_dir = utils::get_shim_dir(environment);
    let file_path = shim_dir.join(command_name.as_str());
    let exe_path = std::env::current_exe()?;
    let bvm_path = exe_path.with_file_name("bvm");
    environment.write_file_text(
        &file_path,
        &format!(
            r#"#!/bin/sh
"{}" exec-command {} "{}" "$@"
"#,
            bvm_path.display(),
            command_name.as_str(),
            command_path.display(),
            exe_dir.display(),
        ),
    )?;
    std::process::Command::new("chmod")
        .args(&["+x".to_string(), file_path.to_string_lossy().to_string()])
        .output()?;
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn create_shim(environment: &impl Environment, command_name: &CommandName, command_path: &Path) -> Result<(), ErrBox> {
    let shim_dir = utils::get_shim_dir(environment);
    let exe_path = std::env::current_exe()?;
    let bvm_path = exe_path.with_file_name("bvm");
    environment.write_file_text(
        &shim_dir.join(format!("{}.bat", command_name.as_str())),
        &format!(
            r#"@ECHO OFF
"{}" exec-command {} "{}" %*
"#,
            bvm_path.with_extension("cmd").display(),
            command_name.as_str(),
            command_path.display(),
        ),
    )?;
    environment.write_file_text(
        &shim_dir.join(format!("{}.ps1", command_name.as_str())),
        &format!(
            r#"#!/usr/bin/env pwsh
. "{}" exec-command {} "{}" @args
"#,
            bvm_path.with_extension("ps1").display(),
            command_name.as_str(),
            command_path.display(),
        ),
    )?;
    Ok(())
}

pub fn get_shim_paths(environment: &impl Environment, command_name: &CommandName) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let shim_dir = utils::get_shim_dir(environment);
    #[cfg(target_os = "windows")]
    {
        paths.push(shim_dir.join(format!("{}.bat", command_name.as_str())));
        paths.push(shim_dir.join(format!("{}.ps1", command_name.as_str())));
    }
    #[cfg(unix)]
    paths.push(shim_dir.join(format!("{}", command_name.as_str())));
    paths
}
