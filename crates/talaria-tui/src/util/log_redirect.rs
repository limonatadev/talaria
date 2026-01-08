use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

#[cfg(unix)]
use std::fs::OpenOptions;
#[cfg(unix)]
use std::os::unix::io::AsRawFd;

#[cfg(unix)]
pub fn redirect_stderr_to_file(path: &Path) -> Result<PathBuf> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("create log directory")?;
    }

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("open log file {}", path.display()))?;

    // Safety: dup2 is required to redirect the process-wide stderr FD so that GUI/library
    // log spam cannot corrupt the terminal UI. We do this before spawning worker threads.
    unsafe {
        if libc::dup2(file.as_raw_fd(), libc::STDERR_FILENO) < 0 {
            return Err(anyhow::anyhow!("dup2 stderr failed"));
        }
    }

    Ok(path.to_path_buf())
}

#[cfg(windows)]
pub fn redirect_stderr_to_file(path: &Path) -> Result<PathBuf> {
    // Windows doesn't have Unix file descriptors; simplest cross-platform behavior is no-op.
    // If you later want real redirection, implement with Windows handles / CRT _dup2.
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("create log directory")?;
    }
    Ok(path.to_path_buf())
}
