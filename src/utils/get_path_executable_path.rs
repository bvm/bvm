use std::env;
use std::path::PathBuf;

use crate::types::{CommandName, ErrBox};

pub fn get_path_executable_path(command_name: &CommandName) -> Result<Option<PathBuf>, ErrBox> {
    let bin_dir = super::get_bin_dir()?;
    let command_name = command_name.as_str().to_lowercase();
    let executable_file_names = get_executable_file_names(&command_name);

    if let Some(path) = env::var_os("PATH") {
        for path_dir in env::split_paths(&path) {
            if path_dir == bin_dir {
                continue;
            }
            for executable_file_name in executable_file_names.iter() {
                let final_path = path_dir.join(executable_file_name);
                if final_path.exists() {
                    return Ok(Some(final_path));
                }
            }
        }
    }

    Ok(None)
}

fn get_executable_file_names(command_name: &str) -> Vec<String> {
    // this is probably not exactly correct :)
    let mut results = Vec::new();

    #[cfg(unix)]
    {
        results.push(command_name.to_string());
        results.push(format!("{}.sh", command_name));
    }
    #[cfg(target_os = "windows")]
    {
        results.push(format!("{}.bat", command_name));
        results.push(format!("{}.exe", command_name));
    }

    results
}
