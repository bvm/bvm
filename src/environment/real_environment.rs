use async_trait::async_trait;
use bytes::Bytes;
use reqwest::Client;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use crate::types::ErrBox;

use super::Environment;

#[derive(Clone)]
pub struct RealEnvironment {
    output_lock: Arc<Mutex<u8>>,
    is_verbose: bool,
}

impl RealEnvironment {
    pub fn new(is_verbose: bool) -> RealEnvironment {
        RealEnvironment {
            output_lock: Arc::new(Mutex::new(0)),
            is_verbose,
        }
    }
}

const APP_INFO: app_dirs::AppInfo = app_dirs::AppInfo {
    name: "bvm",
    author: "bvm",
};

#[async_trait]
impl Environment for RealEnvironment {
    fn is_real(&self) -> bool {
        true
    }

    fn read_file_text(&self, file_path: &Path) -> Result<String, ErrBox> {
        log_verbose!(self, "Reading file: {}", file_path.display());
        Ok(fs::read_to_string(file_path)?)
    }

    fn read_file(&self, file_path: &Path) -> Result<Bytes, ErrBox> {
        log_verbose!(self, "Reading file: {}", file_path.display());
        Ok(Bytes::from(fs::read(file_path)?))
    }

    fn write_file_text(&self, file_path: &Path, file_text: &str) -> Result<(), ErrBox> {
        log_verbose!(self, "Writing file: {}", file_path.display());
        fs::write(file_path, file_text)?;
        Ok(())
    }

    fn write_file(&self, file_path: &Path, bytes: &[u8]) -> Result<(), ErrBox> {
        log_verbose!(self, "Writing file: {}", file_path.display());
        fs::write(file_path, bytes)?;
        Ok(())
    }

    fn remove_file(&self, file_path: &Path) -> Result<(), ErrBox> {
        log_verbose!(self, "Deleting file: {}", file_path.display());
        fs::remove_file(file_path)?;
        Ok(())
    }

    fn remove_dir_all(&self, dir_path: &Path) -> Result<(), ErrBox> {
        log_verbose!(self, "Deleting directory: {}", dir_path.display());
        fs::remove_dir_all(dir_path)?;
        Ok(())
    }

    async fn download_file(&self, url: &str) -> Result<Bytes, ErrBox> {
        log_verbose!(self, "Downloading url: {}", url);

        let client = Client::new();
        let resp = client.get(url).send().await?;
        Ok(resp.bytes().await?)
    }

    fn path_exists(&self, file_path: &Path) -> bool {
        log_verbose!(self, "Checking path exists: {}", file_path.display());
        file_path.exists()
    }

    fn is_dir_empty(&self, dir_path: &Path) -> Result<bool, ErrBox> {
        Ok(std::fs::read_dir(dir_path)?.next().is_none())
    }

    fn create_dir_all(&self, path: &Path) -> Result<(), ErrBox> {
        fs::create_dir_all(path)?;
        Ok(())
    }

    fn cwd(&self) -> Result<PathBuf, ErrBox> {
        Ok(std::env::current_dir()?)
    }

    fn log(&self, text: &str) {
        let _g = self.output_lock.lock().unwrap();
        println!("{}", text);
    }

    fn log_error(&self, text: &str) {
        let _g = self.output_lock.lock().unwrap();
        eprintln!("{}", text);
    }

    fn get_user_data_dir(&self) -> Result<PathBuf, ErrBox> {
        log_verbose!(self, "Getting user data directory.");
        match app_dirs::app_root(app_dirs::AppDataType::UserData, &APP_INFO) {
            Ok(path) => Ok(path),
            Err(err) => err!("Error getting user data directory: {:?}", err),
        }
    }

    fn get_system_path_dirs(&self) -> Vec<PathBuf> {
        if let Some(path) = std::env::var_os("PATH") {
            std::env::split_paths(&path).collect() // todo: how to return an iterator?
        } else {
            Vec::new()
        }
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
        return finalize_and_run_command(cwd, Command::new("cmd").arg("/k").arg(command));

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
                        Ok(())
                    } else {
                        Ok(())
                    }
                }
                None => err!("Process terminated by signal."),
            }
        }
    }

    fn is_verbose(&self) -> bool {
        self.is_verbose
    }
}
