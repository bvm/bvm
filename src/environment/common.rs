/// The separator used for the system path
pub const SYS_PATH_DELIMITER: &'static str = if cfg!(target_os = "windows") { ";" } else { ":" };
