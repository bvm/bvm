use async_trait::async_trait;
use bytes::Bytes;
use std::path::{Path, PathBuf};

use crate::types::ErrBox;

#[async_trait]
pub trait Environment: Clone + std::marker::Send + std::marker::Sync + 'static {
    fn is_real(&self) -> bool;
    fn read_file(&self, file_path: &Path) -> Result<Bytes, ErrBox>;
    fn read_file_text(&self, file_path: &Path) -> Result<String, ErrBox>;
    fn write_file(&self, file_path: &Path, bytes: &[u8]) -> Result<(), ErrBox>;
    fn write_file_text(&self, file_path: &Path, file_text: &str) -> Result<(), ErrBox>;
    fn remove_file(&self, file_path: &Path) -> Result<(), ErrBox>;
    fn remove_dir_all(&self, dir_path: &Path) -> Result<(), ErrBox>;
    fn path_exists(&self, file_path: &Path) -> bool;
    fn is_dir_empty(&self, dir_path: &Path) -> Result<bool, ErrBox>;
    fn create_dir_all(&self, path: &Path) -> Result<(), ErrBox>;
    fn cwd(&self) -> Result<PathBuf, ErrBox>;
    fn log(&self, text: &str);
    fn log_error(&self, text: &str);
    async fn download_file(&self, url: &str) -> Result<Bytes, ErrBox>;
    fn get_user_data_dir(&self) -> Result<PathBuf, ErrBox>;
    fn get_time_secs(&self) -> u64;
    /// Gets the directories in the path environment variable.
    fn get_system_path_dirs(&self) -> Vec<PathBuf>;
    fn run_shell_command(&self, cwd: &Path, command: &str) -> Result<(), ErrBox>;
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
