pub const PATH_SEPARATOR: &'static str = if cfg!(target_os = "windows") { "\\" } else { "/" };
/// The separator used for the system path
pub const SYS_PATH_DELIMITER: &'static str = if cfg!(target_os = "windows") { ";" } else { ":" };

// todo: change to_variable_name to a constant function then change these to constants in the future

pub fn get_local_data_dir_var_name() -> String {
    to_variable_name("BVM_LOCAL_DATA_DIR")
}

pub fn get_data_dir_var_name() -> String {
    to_variable_name("BVM_DATA_DIR")
}

fn to_variable_name(name: &str) -> String {
    if cfg!(target_os = "windows") {
        format!("%{}%", name)
    } else {
        format!("${}", name)
    }
}
