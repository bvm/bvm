use std::path::PathBuf;

use crate::environment::Environment;
use crate::types::CommandName;

pub fn get_path_executable_path(environment: &impl Environment, command_name: &CommandName) -> Option<PathBuf> {
  let shim_dir = super::get_shim_dir(environment);
  let env_path = environment.get_env_path();
  let paths = std::env::split_paths(&env_path).filter(|path_dir| path_dir != &shim_dir);
  get_command_executable_path_in_dirs(environment, command_name, paths)
}

pub fn get_command_executable_path_in_dirs(
  environment: &impl Environment,
  command_name: &CommandName,
  dirs: impl Iterator<Item = PathBuf>,
) -> Option<PathBuf> {
  let executable_file_names = get_executable_file_names_for_command(&command_name);

  for path_dir in dirs {
    for executable_file_name in executable_file_names.iter() {
      let final_path = path_dir.join(executable_file_name);
      if environment.path_exists(&final_path) {
        return Some(final_path);
      }
    }
  }

  None
}

fn get_executable_file_names_for_command(command_name: &CommandName) -> Vec<String> {
  // this is probably not exactly correct :)
  let mut results = Vec::new();
  let command_name = command_name.as_str().to_lowercase();

  #[cfg(unix)]
  {
    results.push(command_name.to_string());
    results.push(format!("{}.sh", command_name));
  }
  #[cfg(target_os = "windows")]
  {
    // todo: should maybe check pathext?
    results.push(format!("{}.bat", command_name));
    results.push(format!("{}.exe", command_name));
    results.push(format!("{}.cmd", command_name));
    results.push(format!("{}.ps1", command_name));
  }

  results
}
