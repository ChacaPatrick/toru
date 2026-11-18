use std::io;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::platform::INSTALLER_ARGS;

const DETACHED_PROCESS: u32 = 0x0000_0008;
const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Windows' own curl, part of every Windows 10 and 11, set to follow redirects over HTTPS
/// only, fail on an HTTP error, and say nothing. Toru has no console, so curl gets none.
fn curl(seconds: u32) -> Command {
    let windows = std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into());
    let mut curl = Command::new(PathBuf::from(windows).join("System32").join("curl.exe"));
    curl.args([
        "-fsSL",
        "--proto-redir",
        "=https",
        "--connect-timeout",
        "15",
    ])
    .args(["--max-time", &seconds.to_string()])
    .creation_flags(CREATE_NO_WINDOW)
    .stdin(Stdio::null())
    .stderr(Stdio::null());
    curl
}

fn curl_failed(code: Option<i32>) -> io::Error {
    io::Error::other(format!("curl ended with {code:?}"))
}

/// The text at `url`, which is small: an update's manifest. Blocks, so not for the main thread.
pub fn fetch_text(url: &str) -> io::Result<String> {
    let output = curl(30).arg(url).output()?;
    if !output.status.success() {
        return Err(curl_failed(output.status.code()));
    }
    String::from_utf8(output.stdout).map_err(io::Error::other)
}

/// Downloads `url` to the file `to`, replacing it. Blocks for as long as the download takes.
pub fn download(url: &str, to: &Path) -> io::Result<()> {
    let status = curl(900)
        .arg("-o")
        .arg(to)
        .arg(url)
        .stdout(Stdio::null())
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(curl_failed(status.code()))
    }
}

/// Starts an update's `setup` on its own, so it outlives this process: it waits for Toru to
/// exit, installs over it without showing anything, and starts the new Toru
/// (`INSTALLER_ARGS`).
pub fn start_installer(setup: &Path) -> io::Result<()> {
    Command::new(setup)
        .args(INSTALLER_ARGS)
        .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(drop)
}
