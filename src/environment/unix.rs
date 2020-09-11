use dprint_cli_core::types::ErrBox;
use std::path::{Path, PathBuf};

pub fn ensure_system_path(directory_path: &Path) -> Result<bool, ErrBox> {
    // Adapted and copy + pasted from https://github.com/freesig/env_perm/blob/37bf63e06c41893118a5dfbf4b38e6887a2778f7/src/lib.rs#L41
    // Copyright (c) 2018 Tom Gowan, MIT License
    use std::fs::File;
    use std::fs::OpenOptions;
    use std::io::{self, Write};

    let system_path_dirs = super::common::get_system_path_dirs();

    return if system_path_dirs.contains(&directory_path.to_path_buf()) {
        return Ok(false);
    } else {
        let mut profile = get_profile()?;
        writeln!(profile, "export PATH=\"$PATH:{}\"", directory_path.to_string_lossy())?;
        profile.flush()?;
        Ok(true)
    };

    fn get_profile() -> io::Result<File> {
        dirs::home_dir()
            .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "No home directory"))
            .and_then(find_profile)
    }

    fn find_profile(mut profile: PathBuf) -> io::Result<File> {
        profile.push(".bash_profile");
        let mut oo = OpenOptions::new();
        oo.append(true).create(false);
        oo.open(profile.clone())
            .or_else(|_| {
                profile.pop();
                profile.push(".bash_login");
                oo.open(profile.clone())
            })
            .or_else(|_| {
                profile.pop();
                profile.push(".profile");
                oo.open(profile.clone())
            })
            .or_else(|_| {
                profile.pop();
                profile.push(".bash_profile");
                oo.create(true);
                oo.open(profile.clone())
            })
    }
}
