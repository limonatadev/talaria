use std::fs;
use std::fs::OpenOptions;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

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
