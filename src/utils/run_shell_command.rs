use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};

use crate::types::ErrBox;

pub fn run_shell_command(cwd: &Path, command: &str) -> Result<(), ErrBox> {
    #[cfg(unix)]
    return finalize_and_run_command(Command::new("/bin/sh").arg("-c").arg(command));

    #[cfg(target_os = "windows")]
    return finalize_and_run_command(cwd, Command::new("cmd").arg("/k").arg(command));
}

fn finalize_and_run_command(cwd: &Path, command: &mut Command) -> Result<(), ErrBox> {
    command
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .current_dir(cwd)
        .status()?;
    Ok(())
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
