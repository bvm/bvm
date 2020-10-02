use async_trait::async_trait;
use dprint_cli_core::types::ErrBox;
use path_clean::PathClean;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use super::Environment;

#[derive(Clone)]
pub struct TestEnvironment {
    // todo: single arc and mutex...
    is_verbose: Arc<Mutex<bool>>,
    cwd: Arc<Mutex<String>>,
    files: Arc<Mutex<HashMap<PathBuf, Vec<u8>>>>,
    logged_messages: Arc<Mutex<Vec<String>>>,
    logged_errors: Arc<Mutex<Vec<String>>>,
    run_shell_commands: Arc<Mutex<Vec<(String, String)>>>,
    remote_files: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    deleted_directories: Arc<Mutex<Vec<PathBuf>>>,
    path_dirs: Arc<Mutex<Vec<PathBuf>>>,
    #[cfg(target_os = "windows")]
    sys_env_variables: Arc<Mutex<HashMap<String, String>>>,
    env_variables: Arc<Mutex<HashMap<String, String>>>,
}

impl TestEnvironment {
    pub fn new() -> TestEnvironment {
        let mut env_variables = HashMap::new();
        env_variables.insert("PATH".to_string(), "/data/shims".to_string());
        TestEnvironment {
            is_verbose: Arc::new(Mutex::new(false)),
            cwd: Arc::new(Mutex::new(String::from("/"))),
            files: Arc::new(Mutex::new(HashMap::new())),
            logged_messages: Arc::new(Mutex::new(Vec::new())),
            logged_errors: Arc::new(Mutex::new(Vec::new())),
            run_shell_commands: Arc::new(Mutex::new(Vec::new())),
            remote_files: Arc::new(Mutex::new(HashMap::new())),
            deleted_directories: Arc::new(Mutex::new(Vec::new())),
            path_dirs: Arc::new(Mutex::new(vec![PathBuf::from("/data/shims")])),
            #[cfg(target_os = "windows")]
            sys_env_variables: Arc::new(Mutex::new(HashMap::new())),
            env_variables: Arc::new(Mutex::new(env_variables)),
        }
    }

    pub fn take_logged_messages(&self) -> Vec<String> {
        self.logged_messages.lock().unwrap().drain(..).collect()
    }

    pub fn clear_logs(&self) {
        self.logged_messages.lock().unwrap().clear();
        self.logged_errors.lock().unwrap().clear();
    }

    pub fn take_logged_errors(&self) -> Vec<String> {
        self.logged_errors.lock().unwrap().drain(..).collect()
    }

    pub fn take_run_shell_commands(&self) -> Vec<(String, String)> {
        self.run_shell_commands.lock().unwrap().drain(..).collect()
    }

    pub fn get_sys_env_variables(&self) -> Vec<(String, String)> {
        #[cfg(target_os = "windows")]
        let mut items = self
            .sys_env_variables
            .lock()
            .unwrap()
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect::<Vec<_>>();
        #[cfg(not(target_os = "windows"))]
        let mut items = Vec::<(String, String)>::new();
        items.sort();
        items
    }

    pub fn add_remote_file(&self, path: &str, bytes: Vec<u8>) {
        let mut remote_files = self.remote_files.lock().unwrap();
        remote_files.insert(String::from(path), bytes);
    }

    pub fn is_dir_deleted(&self, path: &Path) -> bool {
        let deleted_directories = self.deleted_directories.lock().unwrap();
        deleted_directories.contains(&path.to_path_buf())
    }

    pub fn set_cwd(&self, new_path: &str) {
        let mut cwd = self.cwd.lock().unwrap();
        *cwd = String::from(new_path);
    }

    pub fn set_verbose(&self, value: bool) {
        let mut is_verbose = self.is_verbose.lock().unwrap();
        *is_verbose = value;
    }

    pub fn add_path_dir(&self, dir: PathBuf) {
        let mut path_dirs = self.path_dirs.lock().unwrap();
        path_dirs.push(dir);
    }

    pub fn get_system_path_dirs(&self) -> Vec<PathBuf> {
        self.path_dirs.lock().unwrap().clone()
    }

    pub fn set_env_path(&self, new_path: &str) {
        let mut env_variables = self.env_variables.lock().unwrap();
        let env_path = env_variables.get_mut("PATH").unwrap();
        *env_path = new_path.to_string();
    }

    pub fn set_env_var(&self, key: &str, value: &str) {
        let mut env_variables = self.env_variables.lock().unwrap();
        env_variables.insert(key.to_string(), value.to_string());
    }

    pub fn remove_env_var(&self, key: &str) {
        let mut env_variables = self.env_variables.lock().unwrap();
        env_variables.remove(&key.to_string());
    }
}

impl Drop for TestEnvironment {
    fn drop(&mut self) {
        // If this panics that means the logged messages or errors weren't inspected for a test.
        // Use take_logged_messages() or take_logged_errors() and inspect the results.
        if !std::thread::panicking() && Arc::strong_count(&self.logged_messages) == 1 {
            assert_eq!(
                self.logged_messages.lock().unwrap().clone(),
                Vec::<String>::new(),
                "should not have logged messages left on drop"
            );
            assert_eq!(
                self.logged_errors.lock().unwrap().clone(),
                Vec::<String>::new(),
                "should not have logged errors left on drop"
            );
            assert_eq!(
                self.run_shell_commands.lock().unwrap().clone(),
                Vec::<(String, String)>::new(),
                "should not have run shell commands left on drop"
            );
        }
    }
}

#[async_trait]
impl Environment for TestEnvironment {
    fn is_real(&self) -> bool {
        false
    }

    fn read_file_text(&self, file_path: &Path) -> Result<String, ErrBox> {
        let file_bytes = self.read_file(file_path)?;
        Ok(String::from_utf8(file_bytes.to_vec()).unwrap())
    }

    fn read_file(&self, file_path: &Path) -> Result<Vec<u8>, ErrBox> {
        let files = self.files.lock().unwrap();
        // temporary until https://github.com/danreeves/path-clean/issues/4 is fixed in path-clean
        let file_path = PathBuf::from(file_path.to_string_lossy().replace("\\", "/"));
        match files.get(&file_path.clean()) {
            Some(text) => Ok(text.clone()),
            None => err!("Could not find file at path {}", file_path.display()),
        }
    }

    fn write_file_text(&self, file_path: &Path, file_text: &str) -> Result<(), ErrBox> {
        self.write_file(file_path, file_text.as_bytes())
    }

    fn write_file(&self, file_path: &Path, bytes: &[u8]) -> Result<(), ErrBox> {
        let mut files = self.files.lock().unwrap();
        files.insert(file_path.to_path_buf().clean(), Vec::from(bytes));
        Ok(())
    }

    fn remove_file(&self, file_path: &Path) -> Result<(), ErrBox> {
        let mut files = self.files.lock().unwrap();
        files.remove(&file_path.to_path_buf().clean());
        Ok(())
    }

    fn remove_dir_all(&self, dir_path: &Path) -> Result<(), ErrBox> {
        {
            let mut deleted_directories = self.deleted_directories.lock().unwrap();
            deleted_directories.push(dir_path.to_owned());
        }
        let dir_path = dir_path.to_path_buf().clean();
        let mut files = self.files.lock().unwrap();
        let mut delete_paths = Vec::new();
        for (file_path, _) in files.iter() {
            if file_path.starts_with(&dir_path) {
                delete_paths.push(file_path.clone());
            }
        }
        for path in delete_paths {
            files.remove(&path);
        }
        Ok(())
    }

    async fn download_file(&self, url: &str) -> Result<Vec<u8>, ErrBox> {
        let remote_files = self.remote_files.lock().unwrap();
        match remote_files.get(&String::from(url)) {
            Some(bytes) => Ok(bytes.clone()),
            None => err!("Could not find file at url {}", url),
        }
    }

    fn path_exists(&self, file_path: &Path) -> bool {
        let files = self.files.lock().unwrap();
        files.contains_key(&file_path.to_path_buf().clean())
    }

    fn create_dir_all(&self, _: &Path) -> Result<(), ErrBox> {
        Ok(())
    }

    fn is_dir_empty(&self, dir_path: &Path) -> Result<bool, ErrBox> {
        let dir_path = dir_path.to_path_buf().clean();
        let files = self.files.lock().unwrap();
        for file_path in files.keys() {
            if file_path.starts_with(&dir_path) {
                return Ok(false);
            }
        }

        Ok(true)
    }

    fn get_env_var(&self, key: &str) -> Option<String> {
        let env_vars = self.env_variables.lock().unwrap();
        env_vars.get(&key.to_string()).as_ref().map(|key| key.to_string())
    }

    #[cfg(windows)]
    fn ensure_system_path(&self, directory_path: &str) -> Result<(), ErrBox> {
        let mut path_dirs = self.path_dirs.lock().unwrap();
        let directory_path = PathBuf::from(directory_path);
        if !path_dirs.contains(&directory_path) {
            path_dirs.push(directory_path);
        }
        Ok(())
    }

    #[cfg(windows)]
    fn ensure_system_path_pre(&self, directory_path: &str) -> Result<(), ErrBox> {
        let mut path_dirs = self.path_dirs.lock().unwrap();
        let directory_path = PathBuf::from(directory_path);
        if let Some(pos) = path_dirs.iter().position(|p| p == &directory_path) {
            path_dirs.remove(pos);
        }
        path_dirs.insert(0, directory_path);
        Ok(())
    }

    #[cfg(windows)]
    fn remove_system_path(&self, directory_path: &str) -> Result<(), ErrBox> {
        let mut path_dirs = self.path_dirs.lock().unwrap();
        let directory_path = PathBuf::from(directory_path);
        if let Some(pos) = path_dirs.iter().position(|p| p == &directory_path) {
            path_dirs.remove(pos);
        }
        Ok(())
    }

    #[cfg(windows)]
    fn set_env_variable(&self, key: String, value: String) -> Result<(), ErrBox> {
        let mut sys_env_variables = self.sys_env_variables.lock().unwrap();
        sys_env_variables.insert(key, value);
        Ok(())
    }

    #[cfg(windows)]
    fn remove_env_variable(&self, key: &str) -> Result<(), ErrBox> {
        let mut sys_env_variables = self.sys_env_variables.lock().unwrap();
        sys_env_variables.remove(key);
        Ok(())
    }

    fn run_shell_command(&self, cwd: &Path, command: &str) -> Result<(), ErrBox> {
        let mut run_shell_commands = self.run_shell_commands.lock().unwrap();
        run_shell_commands.push((cwd.to_string_lossy().to_string(), command.to_string()));
        Ok(())
    }

    fn cwd(&self) -> Result<PathBuf, ErrBox> {
        let cwd = self.cwd.lock().unwrap();
        Ok(PathBuf::from(cwd.to_owned()))
    }

    fn log(&self, text: &str) {
        self.logged_messages.lock().unwrap().push(String::from(text));
    }

    fn log_error(&self, text: &str) {
        self.logged_errors.lock().unwrap().push(String::from(text));
    }

    async fn log_action_with_progress<
        TResult: std::marker::Send + std::marker::Sync,
        TCreate: FnOnce(Box<dyn Fn(usize)>) -> TResult + std::marker::Send + std::marker::Sync,
    >(
        &self,
        message: &str,
        action: TCreate,
        _: usize,
    ) -> Result<TResult, ErrBox> {
        self.log_error(message);
        Ok(action(Box::new(|_| {})))
    }

    fn get_local_user_data_dir(&self) -> PathBuf {
        PathBuf::from("/local-data")
    }

    fn get_user_data_dir(&self) -> PathBuf {
        PathBuf::from("/data")
    }

    fn get_time_secs(&self) -> u64 {
        123456
    }

    fn exit(&self, code: i32) -> Result<(), ErrBox> {
        err!("Exited with code {}", code)
    }

    fn is_verbose(&self) -> bool {
        *self.is_verbose.lock().unwrap()
    }
}
