//! Building child processes without flashing a console window on Windows.
use std::ffi::OsStr;
use std::process::Command;

/// Like `Command::new`, but the child process does not open a console window.
///
/// Bumeter is a GUI (Windows subsystem) app, so spawning a console program (`curl`, `git`,
/// `claude`, `kiro-cli`…) would otherwise pop up a black window each time it runs.
pub fn command(program: impl AsRef<OsStr>) -> Command {
    let mut cmd = Command::new(program);
    hide_console(&mut cmd);
    cmd
}

#[cfg(windows)]
fn hide_console(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn hide_console(_cmd: &mut Command) {}
