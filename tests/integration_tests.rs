use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

pub const SYS_PATH_DELIMITER: &'static str = if cfg!(target_os = "windows") { ";" } else { ":" };

#[cfg(windows)]
#[test]
fn cmd_integration() {
    let root_folder = get_root_folder();
    let envs = setup_command();

    let result = Command::new("C:\\Windows\\System32\\cmd")
        .args([
            "/C",
            root_folder
                .join("tests\\specs\\tests.cmd")
                .to_string_lossy()
                .to_string()
                .as_str(),
        ])
        .current_dir(root_folder.join("target"))
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
        fs::read_to_string(root_folder.join("tests/specs/tests.cmd.out"))
            .unwrap()
            .trim()
            .replace("\r\n", "\n")
    );
}

#[cfg(windows)]
#[test]
fn powershell_integration() {
    let root_folder = get_root_folder();
    let envs = setup_command();

    let result = Command::new("C:\\Windows\\System32\\WINDOWSPOWERSHELL\\v1.0\\powershell.exe")
        .args([
            "-NoProfile",
            &format!("& \"{}\"", root_folder.join("tests\\specs\\tests.ps1").display()),
        ])
        .current_dir(root_folder.join("target"))
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
        fs::read_to_string(root_folder.join("tests/specs/tests.ps1.out"))
            .unwrap()
            .trim()
            .replace("\r\n", "\n")
    );
}

#[cfg(unix)]
#[test]
fn sh_integration() {
    let root_folder = get_root_folder();
    let envs = setup_command();

    let result = Command::new(root_folder.join("tests/specs/tests.sh"))
        .current_dir(root_folder.join("target"))
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
        fs::read_to_string(root_folder.join("tests/specs/tests.sh.out"))
            .unwrap()
            .trim()
            .replace("\r\n", "\n")
    );
}

fn setup_command() -> HashMap<&'static str, String> {
    let root_folder = get_root_folder();
    let local_user_data_dir = root_folder
        .join("target")
        .join("debug")
        .join("cmd")
        .join("local_user_data");
    let user_data_dir = root_folder
        .join("target")
        .join("debug")
        .join("cmd")
        .join("user_data_dir");
    let home_dir = root_folder.join("target").join("debug").join("cmd").join("home_dir");
    let bin_dir = home_dir.join("bin");
    let shims_dir = user_data_dir.join("shims");
    if home_dir.exists() {
        fs::remove_dir_all(&home_dir).unwrap();
    }
    if user_data_dir.exists() {
        fs::remove_dir_all(&user_data_dir).unwrap();
    }
    if local_user_data_dir.exists() {
        fs::remove_dir_all(&local_user_data_dir).unwrap();
    }
    std::fs::create_dir_all(&bin_dir).unwrap();
    std::fs::create_dir_all(&user_data_dir).unwrap();
    std::fs::create_dir_all(&local_user_data_dir).unwrap();
    fs::copy("bvm.cmd", bin_dir.join("bvm.cmd")).unwrap();
    fs::copy("bvm.ps1", bin_dir.join("bvm.ps1")).unwrap();
    fs::copy("bvm.sh", bin_dir.join("bvm.sh")).unwrap();
    let windows_bin = PathBuf::from("target/debug/bvm-bin.exe");
    let unix_bin = PathBuf::from("target/debug/bvm-bin");
    if windows_bin.exists() {
        fs::copy(windows_bin, bin_dir.join("bvm-bin.exe")).unwrap();
    } else if unix_bin.exists() {
        fs::copy(unix_bin, bin_dir.join("bvm-bin")).unwrap();
    } else {
        panic!("Please build the project before running the tests.");
    }

    HashMap::from([
        (
            "PATH",
            format!("{}{}{}", bin_dir.display(), SYS_PATH_DELIMITER, shims_dir.display()),
        ),
        ("BVM_LOCAL_USER_DATA_DIR", local_user_data_dir.display().to_string()),
        ("BVM_USER_DATA_DIR", user_data_dir.display().to_string()),
        ("BVM_HOME_DIR", home_dir.display().to_string()),
    ])
}

#[cfg(unix)]
fn setup_unix_command() -> HashMap<&'static str, String> {
    let root_folder = get_root_folder();
    let local_user_data_dir = root_folder.join("target/debug/cmd/local_user_data");
    let user_data_dir = root_folder.join("target/debug\\cmd\\user_data_dir");
    let home_dir = root_folder.join("target\\debug\\cmd\\home_dir");
    let bin_dir = home_dir.join("bin");
    let shims_dir = user_data_dir.join("shims");
    if home_dir.exists() {
        fs::remove_dir_all(&home_dir).unwrap();
    }
    if user_data_dir.exists() {
        fs::remove_dir_all(&user_data_dir).unwrap();
    }
    if local_user_data_dir.exists() {
        fs::remove_dir_all(&local_user_data_dir).unwrap();
    }
    std::fs::create_dir_all(&bin_dir).unwrap();
    std::fs::create_dir_all(&user_data_dir).unwrap();
    std::fs::create_dir_all(&local_user_data_dir).unwrap();
    fs::copy("bvm.cmd", bin_dir.join("bvm.cmd")).unwrap();
    fs::copy("bvm.ps1", bin_dir.join("bvm.ps1")).unwrap();
    fs::copy("target\\debug\\bvm-bin.exe", bin_dir.join("bvm-bin.exe")).unwrap();

    HashMap::from([
        ("PATH", format!("{};{}", bin_dir.display(), shims_dir.display())),
        ("BVM_LOCAL_USER_DATA_DIR", local_user_data_dir.display().to_string()),
        ("BVM_USER_DATA_DIR", user_data_dir.display().to_string()),
        ("BVM_HOME_DIR", home_dir.display().to_string()),
    ])
}

fn get_root_folder() -> PathBuf {
    std::env::current_dir().unwrap()
}
