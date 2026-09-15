//! Shared helper to run reference CLI binaries as sandboxed subprocesses.

use std::ffi::OsStr;
use std::io::Read;
use std::process::{Command, Stdio};
use std::thread;

use crate::{ReferenceError, ReferenceResult};

/// Spawns `binary`, streams stdout and stderr out of the pipes concurrently
/// (so a chatty subprocess can never fill a pipe and deadlock), then returns
/// `(exit_code, stdout, stderr)`.
pub(crate) fn run_captured(
    binary: &'static str,
    args: &[&OsStr],
) -> ReferenceResult<(i32, Vec<u8>, String)> {
    let mut child = Command::new(binary)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| match err.kind() {
            std::io::ErrorKind::NotFound => ReferenceError::ReferenceBinaryNotFound { binary },
            _ => ReferenceError::Spawn {
                binary,
                source: err,
            },
        })?;

    let mut stdout = child.stdout.take().expect("stdout was piped");
    let mut stderr = child.stderr.take().expect("stderr was piped");

    let stdout_handle = thread::spawn(move || {
        let mut buf = Vec::new();
        stdout
            .read_to_end(&mut buf)
            .map(|_| buf)
            .unwrap_or_default()
    });
    let stderr_handle = thread::spawn(move || {
        let mut buf = String::new();
        stderr
            .read_to_string(&mut buf)
            .map(|_| buf)
            .unwrap_or_default()
    });

    let status = child
        .wait()
        .map_err(|source| ReferenceError::Spawn { binary, source })?;

    let stdout_bytes = stdout_handle.join().unwrap_or_default();
    let stderr_text = stderr_handle.join().unwrap_or_default();
    let exit_code = status.code().unwrap_or(-1);
    Ok((exit_code, stdout_bytes, stderr_text))
}
