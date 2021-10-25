use dprint_cli_core::types::ErrBox;
use std::path::{Path, PathBuf};
use url::Url;

pub trait Environment: Clone + std::marker::Send + std::marker::Sync + 'static {
    fn is_real(&self) -> bool;
    fn read_file(&self, file_path: &Path) -> Result<Vec<u8>, ErrBox>;
    fn read_file_text(&self, file_path: &Path) -> Result<String, ErrBox>;
    fn write_file(&self, file_path: &Path, bytes: &[u8]) -> Result<(), ErrBox>;
    fn write_file_text(&self, file_path: &Path, file_text: &str) -> Result<(), ErrBox>;
    fn remove_file(&self, file_path: &Path) -> Result<(), ErrBox>;
    fn remove_dir_all(&self, dir_path: &Path) -> Result<(), ErrBox>;
    fn path_exists(&self, file_path: &Path) -> bool;
    fn is_dir_empty(&self, dir_path: &Path) -> Result<bool, ErrBox>;
    fn create_dir_all(&self, path: &Path) -> Result<(), ErrBox>;
    fn cwd(&self) -> PathBuf;
    fn log(&self, text: &str);
    fn log_error(&self, text: &str);
    fn fetch_url(&self, url: &Url) -> Result<Vec<u8>, ErrBox> {
        if url.scheme() == "file" {
            self.read_file(
                &url.to_file_path()
                    .map_err(|_| err_obj!("Error converting {} to a file path.", url.as_str()))?,
            )
        } else {
            self.download_file(url.as_str())
        }
    }
    fn download_file(&self, url: &str) -> Result<Vec<u8>, ErrBox>;
    fn log_action_with_progress<
        TResult: std::marker::Send + std::marker::Sync,
        TCreate: FnOnce(Box<dyn Fn(usize)>) -> TResult + std::marker::Send + std::marker::Sync,
    >(
        &self,
        message: &str,
        action: TCreate,
        total_size: usize,
    ) -> TResult;
    /// Attempts to get the local user data dir.
    fn try_get_local_user_data_dir(&self) -> Result<PathBuf, ErrBox>;
    /// Data that is specific to a user on a local machine.
    fn get_local_user_data_dir(&self) -> PathBuf {
        self.try_get_local_user_data_dir().unwrap()
    }
    /// Attempts to get the user data dir.
    fn try_get_user_data_dir(&self) -> Result<PathBuf, ErrBox>;
    /// Data that is specific to a user across machines.
    fn get_user_data_dir(&self) -> PathBuf {
        self.try_get_user_data_dir().unwrap()
    }
    fn try_get_user_home_dir(&self) -> Result<PathBuf, ErrBox>;
    fn get_user_home_dir(&self) -> PathBuf {
        self.try_get_user_home_dir().unwrap()
    }
    fn get_time_secs(&self) -> u64;
    /// Gets the specified environment variable.
    fn get_env_var(&self, key: &str) -> Option<String>;
    /// Gets the environment path variable.
    fn get_env_path(&self) -> String {
        self.get_env_var("PATH").unwrap_or(String::new())
    }
    /// Ensures the provided directory to be on the system path environment variable.
    #[cfg(windows)]
    fn ensure_system_path(&self, directory_path: &str) -> Result<(), ErrBox>;
    /// Ensures the provided directory is at the start of the system path environment variable.
    #[cfg(windows)]
    fn ensure_system_path_pre(&self, directory_path: &str) -> Result<(), ErrBox>;
    #[cfg(windows)]
    fn remove_system_path(&self, directory_path: &str) -> Result<(), ErrBox>;
    #[cfg(windows)]
    fn set_env_variable(&self, key: String, value: String) -> Result<(), ErrBox>;
    #[cfg(windows)]
    fn remove_env_variable(&self, key: &str) -> Result<(), ErrBox>;
    fn run_shell_command(&self, cwd: &Path, command: &str) -> Result<(), ErrBox>;
    fn exit(&self, code: i32) -> Result<(), ErrBox>;
    fn is_verbose(&self) -> bool;
}

// use a macro here so the expression provided is only evaluated when in verbose mode
macro_rules! log_verbose {
    ($environment:expr, $($arg:tt)*) => {
        if $environment.is_verbose() {
            let mut text = String::from("[VERBOSE]: ");
            text.push_str(&format!($($arg)*));
            $environment.log(&text);
        }
    }
}
