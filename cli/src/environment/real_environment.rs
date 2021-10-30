use dprint_cli_core::download_url;
use dprint_cli_core::logging::{log_action_with_progress, Logger, ProgressBars};
use dprint_cli_core::types::ErrBox;
use std::env;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::time::SystemTime;

use super::Environment;

#[derive(Clone)]
pub struct RealEnvironment {
    logger: Logger,
    progress_bars: Option<ProgressBars>,
    is_verbose: bool,
}

impl RealEnvironment {
    pub fn new(is_verbose: bool) -> Result<RealEnvironment, ErrBox> {
        let logger = Logger::new("bvm", /* is silent */ false);
        let progress_bars = ProgressBars::new(&logger);
        let environment = RealEnvironment {
            logger,
            progress_bars,
            is_verbose,
        };

        if let Ok(dir) = environment.try_get_local_user_data_dir() {
            environment.create_dir_all(&dir)?;
        }
        if let Ok(dir) = environment.try_get_user_data_dir() {
            environment.create_dir_all(&dir)?;
        }

        Ok(environment)
    }
}

impl Environment for RealEnvironment {
    fn is_real(&self) -> bool {
        true
    }

    fn read_file_text(&self, file_path: &Path) -> Result<String, ErrBox> {
        let file_bytes = self.read_file(file_path)?;
        Ok(String::from_utf8(file_bytes)?)
    }

    fn read_file(&self, file_path: &Path) -> Result<Vec<u8>, ErrBox> {
        log_verbose!(self, "Reading file: {}", file_path.display());
        match fs::read(file_path) {
            Ok(bytes) => Ok(bytes),
            Err(err) => err!("Error reading file {}: {}", file_path.display(), err.to_string()),
        }
    }

    fn write_file_text(&self, file_path: &Path, file_text: &str) -> Result<(), ErrBox> {
        self.write_file(file_path, file_text.as_bytes())
    }

    fn write_file(&self, file_path: &Path, bytes: &[u8]) -> Result<(), ErrBox> {
        log_verbose!(self, "Writing file: {}", file_path.display());
        match fs::write(file_path, bytes) {
            Ok(_) => Ok(()),
            Err(err) => err!("Error writing file {}: {}", file_path.display(), err.to_string()),
        }
    }

    fn remove_file(&self, file_path: &Path) -> Result<(), ErrBox> {
        log_verbose!(self, "Deleting file: {}", file_path.display());
        match fs::remove_file(file_path) {
            Ok(_) => Ok(()),
            Err(err) => err!("Error deleting file {}: {}", file_path.display(), err.to_string()),
        }
    }

    fn remove_dir_all(&self, dir_path: &Path) -> Result<(), ErrBox> {
        log_verbose!(self, "Deleting directory: {}", dir_path.display());
        match fs::remove_dir_all(dir_path) {
            Ok(_) => Ok(()),
            Err(err) => {
                if err.kind() == ErrorKind::NotFound {
                    Ok(())
                } else {
                    err!("Error removing directory {}: {}", dir_path.display(), err.to_string())
                }
            }
        }
    }

    fn download_file(&self, url: &str) -> Result<Vec<u8>, ErrBox> {
        log_verbose!(self, "Downloading url: {}", url);
        download_url(url, &self.progress_bars, |key| self.get_env_var(key))
    }

    fn path_exists(&self, path: &Path) -> bool {
        log_verbose!(self, "Checking path exists: {}", path.display());
        path.exists()
    }

    fn is_dir_empty(&self, dir_path: &Path) -> Result<bool, ErrBox> {
        let mut result = match std::fs::read_dir(dir_path) {
            Ok(result) => result,
            Err(err) => {
                return err!(
                    "Error checking directory empty {}: {}",
                    dir_path.display(),
                    err.to_string()
                )
            }
        };
        Ok(result.next().is_none())
    }

    fn create_dir_all(&self, path: &Path) -> Result<(), ErrBox> {
        log_verbose!(self, "Creating directory: {}", path.display());
        match fs::create_dir_all(path) {
            Ok(_) => Ok(()),
            Err(err) => err!("Error creating directory {}: {}", path.display(), err.to_string()),
        }
    }

    fn cwd(&self) -> PathBuf {
        env::current_dir().unwrap_or_else(|err| panic!("Error getting current working directory: {}", err.to_string()))
    }

    fn log(&self, text: &str) {
        self.logger.log(text, "bvm");
    }

    fn log_error(&self, text: &str) {
        self.logger.log_err(text, "bvm");
    }

    fn log_action_with_progress<
        TResult: std::marker::Send + std::marker::Sync,
        TCreate: FnOnce(Box<dyn Fn(usize)>) -> TResult + std::marker::Send + std::marker::Sync,
    >(
        &self,
        message: &str,
        action: TCreate,
        total_size: usize,
    ) -> TResult {
        log_action_with_progress(&self.progress_bars, message, action, total_size)
    }

    fn try_get_local_user_data_dir(&self) -> Result<PathBuf, ErrBox> {
        log_verbose!(self, "Getting local user data directory.");
        if let Some(dir_path) = self.get_env_var("BVM_LOCAL_USER_DATA_DIR") {
            Ok(PathBuf::from(dir_path))
        } else if cfg!(target_os = "windows") {
            // %LOCALAPPDATA% is used because we don't want to sync this data across a domain.
            let dir = dirs::data_local_dir().ok_or_else(|| err_obj!("Could not get user's local dir."))?;
            Ok(dir.join("bvm"))
        } else {
            get_home_dir()
        }
    }

    fn try_get_user_data_dir(&self) -> Result<PathBuf, ErrBox> {
        log_verbose!(self, "Getting user data directory.");
        if let Some(dir_path) = self.get_env_var("BVM_USER_DATA_DIR") {
            Ok(PathBuf::from(dir_path))
        } else if cfg!(target_os = "windows") {
            let dir = dirs::data_dir().ok_or_else(|| err_obj!("Could not get user's data dir."))?;
            Ok(dir.join("bvm"))
        } else {
            get_home_dir()
        }
    }

    fn try_get_user_home_dir(&self) -> Result<PathBuf, ErrBox> {
        log_verbose!(self, "Getting user home directory.");
        if let Some(dir_path) = self.get_env_var("BVM_HOME_DIR") {
            Ok(PathBuf::from(dir_path))
        } else {
            get_home_dir()
        }
    }

    fn get_env_var(&self, key: &str) -> Option<String> {
        log_verbose!(self, "Getting the {} environment variable.", key);

        if let Some(path) = std::env::var_os(&key) {
            Some(path.to_string_lossy().to_string())
        } else {
            None
        }
    }

    #[cfg(windows)]
    fn ensure_system_path(&self, directory_path: &str) -> Result<(), ErrBox> {
        use winreg::{enums::*, RegKey};
        log_verbose!(self, "Ensuring '{}' is on the path.", directory_path);

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let (env, _) = hkcu.create_subkey("Environment")?;
        let mut path: String = env.get_value("Path")?;

        // add to the path if it doesn't have this entry
        if !path.split(";").any(|p| p == directory_path) {
            if !path.is_empty() && !path.ends_with(';') {
                path.push_str(";")
            }
            path.push_str(&directory_path);
            env.set_value("Path", &path)?;
        }
        Ok(())
    }

    #[cfg(windows)]
    fn ensure_system_path_pre(&self, directory_path: &str) -> Result<(), ErrBox> {
        use winreg::{enums::*, RegKey};

        // always puts the provided directory at the start of the path
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let (env, _) = hkcu.create_subkey("Environment")?;
        let path: String = env.get_value("Path")?;
        let mut paths = path.split(";").collect::<Vec<_>>();
        paths.retain(|p| p != &directory_path && !p.is_empty());
        paths.insert(0, directory_path);
        env.set_value("Path", &paths.join(";"))?;

        Ok(())
    }

    #[cfg(windows)]
    fn remove_system_path(&self, directory_path: &str) -> Result<(), ErrBox> {
        use winreg::{enums::*, RegKey};
        log_verbose!(self, "Ensuring '{}' is on the path.", directory_path);

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let (env, _) = hkcu.create_subkey("Environment")?;
        let path: String = env.get_value("Path")?;
        let mut paths = path.split(";").collect::<Vec<_>>();
        let original_len = paths.len();

        paths.retain(|p| p != &directory_path);

        let was_removed = original_len != paths.len();
        if was_removed {
            env.set_value("Path", &paths.join(";"))?;
        }
        Ok(())
    }

    #[cfg(windows)]
    fn set_env_variable(&self, key: String, value: String) -> Result<(), ErrBox> {
        use winreg::{enums::*, RegKey};
        log_verbose!(self, "Setting '{}' environment variable.", key);

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let (env, _) = hkcu.create_subkey("Environment")?;
        env.set_value(key, &value)?;
        Ok(())
    }

    #[cfg(windows)]
    fn remove_env_variable(&self, key: &str) -> Result<(), ErrBox> {
        use winreg::{enums::*, RegKey};
        log_verbose!(self, "Removing '{}' environment variable.", key);

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let (env, _) = hkcu.create_subkey("Environment")?;

        env.delete_value(key.to_string())?;
        Ok(())
    }

    fn get_time_secs(&self) -> u64 {
        SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    fn run_shell_command(&self, cwd: &Path, command: &str) -> Result<(), ErrBox> {
        #[cfg(unix)]
        return finalize_and_run_command(cwd, Command::new("/bin/sh").arg("-c").arg(command));

        #[cfg(target_os = "windows")]
        return finalize_and_run_command(cwd, Command::new("cmd").arg("/C").arg(command));

        fn finalize_and_run_command(cwd: &Path, command: &mut Command) -> Result<(), ErrBox> {
            let status = command
                .stdin(Stdio::inherit())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .current_dir(cwd)
                .status()?;
            handle_exit_status(status)
        }

        fn handle_exit_status(exit_status: ExitStatus) -> Result<(), ErrBox> {
            match exit_status.code() {
                Some(code) => {
                    if code != 0 {
                        return err!("Received non zero exit code from shell command: {}", code);
                    } else {
                        Ok(())
                    }
                }
                None => err!("Process terminated by signal."),
            }
        }
    }

    fn exit(&self, code: i32) -> Result<(), ErrBox> {
        std::process::exit(code)
    }

    fn is_verbose(&self) -> bool {
        self.is_verbose
    }
}

fn get_home_dir() -> Result<PathBuf, ErrBox> {
    let dir = dirs::home_dir().ok_or_else(|| err_obj!("Could not get user's home directory."))?;
    Ok(dir.join(".bvm"))
}
