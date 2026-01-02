use std::path::{Path, PathBuf};

use anyhow::Result;
use chrono::Local;

pub fn timestamped_capture_path(dir: &Path, ext: &str) -> Result<PathBuf> {
    let ext = ext.trim_start_matches('.');
    let timestamp = Local::now().format("%Y%m%d_%H%M%S_%3f");
    let filename = format!("frame_{timestamp}.{ext}");
    Ok(dir.join(filename))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamped_path_in_captures_dir() {
        let path = timestamped_capture_path(Path::new("./captures"), "jpg").expect("path");
        let path_str = path.to_string_lossy();
        assert!(path_str.contains("captures"));
        assert!(path_str.ends_with(".jpg"));
    }
}
