use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::Local;

pub fn ensure_capture_dir() -> Result<PathBuf> {
    let dir = Path::new("./captures");
    fs::create_dir_all(dir).context("create captures directory")?;
    Ok(dir.to_path_buf())
}

pub fn timestamped_capture_path(ext: &str) -> Result<PathBuf> {
    let dir = ensure_capture_dir()?;
    let ext = ext.trim_start_matches('.');
    let timestamp = Local::now().format("%Y%m%d_%H%M%S_%3f");
    let filename = format!("capture_{timestamp}.{ext}");
    Ok(dir.join(filename))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamped_path_in_captures_dir() {
        let path = timestamped_capture_path("jpg").expect("path");
        let path_str = path.to_string_lossy();
        assert!(path_str.contains("captures"));
        assert!(path_str.ends_with(".jpg"));
    }
}
