use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Once;

static INIT: Once = Once::new();

pub const SYS_PATH_DELIMITER: &'static str = if cfg!(target_os = "windows") { ";" } else { ":" };

#[cfg(windows)]
#[test]
fn cmd_integration() {
    ensure_setup();
    let cli_folder = get_cli_folder();
    let envs = get_env_vars();

    let result = Command::new("C:\\Windows\\System32\\cmd")
        .args([
            "/C",
            cli_folder
                .join("tests\\specs\\tests.cmd")
                .to_string_lossy()
                .to_string()
                .as_str(),
        ])
        .current_dir(cli_folder.join("tests\\specs"))
        .envs(envs)
        .output()
        .unwrap();
    let stdout = strip_ansi_escapes::strip(&result.stdout).unwrap();
    let output = String::from_utf8_lossy(&stdout);
    let error = String::from_utf8_lossy(&result.stderr);

    println!("Output: {}", output);
    println!("Error: {}", error);
    println!("Status: {}", result.status);

    assert_eq!(
        output.trim().replace("\r\n", "\n"),
        fs::read_to_string(cli_folder.join("tests/specs/tests.cmd.out"))
            .unwrap()
            .trim()
            .replace("\r\n", "\n")
    );
}

#[cfg(windows)]
#[test]
fn powershell_integration() {
    ensure_setup();
    let cli_folder = get_cli_folder();
    let envs = get_env_vars();

    let result = Command::new("C:\\Windows\\System32\\WINDOWSPOWERSHELL\\v1.0\\powershell.exe")
        .args([
            "-NoProfile",
            &format!("& \"{}\"", cli_folder.join("tests\\specs\\tests.ps1").display()),
        ])
        .current_dir(cli_folder.join("tests\\specs"))
        .envs(envs)
        .output()
        .unwrap();
    let stdout = strip_ansi_escapes::strip(&result.stdout).unwrap();
    let output = String::from_utf8_lossy(&stdout);
    let error = String::from_utf8_lossy(&result.stderr);

    println!("Output: {}", output);
    println!("Error: {}", error);
    println!("Status: {}", result.status);

    assert_eq!(
        output.trim().replace("\r\n", "\n"),
        fs::read_to_string(cli_folder.join("tests/specs/tests.ps1.out"))
            .unwrap()
            .trim()
            .replace("\r\n", "\n")
    );
}

#[cfg(unix)]
#[test]
fn sh_integration() {
    ensure_setup();
    let cli_folder = get_cli_folder();
    let envs = get_env_vars();

    let result = Command::new(cli_folder.join("tests/specs/tests.sh"))
        .current_dir(cli_folder.join("tests/specs"))
        .envs(envs)
        .output()
        .unwrap();
    let stdout = strip_ansi_escapes::strip(&result.stdout).unwrap();
    let output = String::from_utf8_lossy(&stdout);
    let error = String::from_utf8_lossy(&result.stderr);

    println!("Output: {}", output);
    println!("Error: {}", error);
    println!("Status: {}", result.status);

    assert_eq!(
        output.trim().replace("\r\n", "\n"),
        fs::read_to_string(cli_folder.join("tests/specs/tests.sh.out"))
            .unwrap()
            .trim()
            .replace("\r\n", "\n")
    );
}

fn ensure_setup() {
    INIT.call_once(|| {
        let paths = get_test_paths();
        if paths.home_dir.exists() {
            fs::remove_dir_all(&paths.home_dir).unwrap();
        }
        if paths.user_data_dir.exists() {
            fs::remove_dir_all(&paths.user_data_dir).unwrap();
        }
        if paths.local_user_data_dir.exists() {
            fs::remove_dir_all(&paths.local_user_data_dir).unwrap();
        }
        std::fs::create_dir_all(&paths.bin_dir).unwrap();
        std::fs::create_dir_all(&paths.user_data_dir).unwrap();
        std::fs::create_dir_all(&paths.local_user_data_dir).unwrap();
        if cfg!(windows) {
            fs::copy("bvm.cmd", paths.bin_dir.join("bvm.cmd")).unwrap();
            fs::copy("bvm.ps1", paths.bin_dir.join("bvm.ps1")).unwrap();
        } else {
            fs::copy("bvm.sh", paths.bin_dir.join("bvm.sh")).unwrap();
        }
        let windows_bin = paths.build_folder.join("bvm-bin.exe");
        let unix_bin = paths.build_folder.join("bvm-bin");
        if windows_bin.exists() {
            fs::copy(windows_bin, paths.bin_dir.join("bvm-bin.exe")).unwrap();
        } else if unix_bin.exists() {
            fs::copy(unix_bin, paths.bin_dir.join("bvm-bin")).unwrap();
        } else {
            panic!("Please build the project before running the tests.");
        }

        build_args_test_util();
    })
}

fn get_env_vars() -> HashMap<&'static str, String> {
    let paths = get_test_paths();
    let mut path_value = format!(
        "{}{}{}",
        paths.bin_dir.display(),
        SYS_PATH_DELIMITER,
        paths.shims_dir.display()
    );
    if cfg!(unix) {
        path_value.push_str(SYS_PATH_DELIMITER);
        path_value.push_str("/bin");
        path_value.push_str(SYS_PATH_DELIMITER);
        path_value.push_str("/usr/bin");
        path_value.push_str(SYS_PATH_DELIMITER);
        path_value.push_str("/usr/local/bin");
    }

    HashMap::from([
        ("PATH", path_value),
        (
            "BVM_LOCAL_USER_DATA_DIR",
            paths.local_user_data_dir.display().to_string(),
        ),
        ("BVM_USER_DATA_DIR", paths.user_data_dir.display().to_string()),
        ("BVM_HOME_DIR", paths.home_dir.display().to_string()),
    ])
}

struct TestPaths {
    build_folder: PathBuf,
    local_user_data_dir: PathBuf,
    user_data_dir: PathBuf,
    home_dir: PathBuf,
    bin_dir: PathBuf,
    shims_dir: PathBuf,
}

fn get_test_paths() -> TestPaths {
    let build_folder = get_cli_folder()
        .parent()
        .unwrap()
        .join("target")
        .join(if cfg!(debug_assertions) { "debug" } else { "release" });
    let local_user_data_dir = build_folder.join("local_user_data");
    let user_data_dir = build_folder.join("user_data_dir");
    let home_dir = build_folder.join("home_dir");
    let bin_dir = home_dir.join("bin");
    let shims_dir = user_data_dir.join("shims");

    TestPaths {
        build_folder,
        local_user_data_dir,
        user_data_dir,
        home_dir,
        bin_dir,
        shims_dir,
    }
}

#[cfg(windows)]
fn build_args_test_util() {
    let cli_folder = get_cli_folder();
    let root_folder = cli_folder.parent().unwrap();
    let status = Command::new("pwsh.exe")
        .args([
            "-NoProfile",
            "-Command",
            &format!(
                "& \"{}\"",
                root_folder.join("scripts/setup_args_test_util.ps1").display()
            ),
        ])
        .current_dir(root_folder)
        .status()
        .unwrap();
    assert!(status.success());
}

#[cfg(not(windows))]
fn build_args_test_util() {
    let cli_folder = get_cli_folder();
    let root_folder = cli_folder.parent().unwrap();
    let status = Command::new("scripts/setup_args_test_util.sh")
        .current_dir(root_folder)
        .status()
        .unwrap();
    assert!(status.success());
}

fn get_cli_folder() -> PathBuf {
    std::env::current_dir().unwrap()
}
