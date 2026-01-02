use crate::camera;
use crate::error::{Error, Result};
use crate::supabase::SupabaseClient;
use std::fs;
use std::path::{Path, PathBuf};

/// Build a Supabase client if configuration is present.
pub fn supabase_from_config(config: &crate::config::Config) -> Result<Option<SupabaseClient>> {
    match &config.supabase {
        Some(cfg) => SupabaseClient::from_config(cfg).map(Some),
        None => Ok(None),
    }
}

pub async fn upload_paths(paths: &[PathBuf], client: &SupabaseClient) -> Result<Vec<String>> {
    let mut urls = Vec::new();
    for path in paths {
        let url = client.upload_image_file(path).await?;
        urls.push(url);
    }
    Ok(urls)
}

pub async fn upload_dir(dir: &Path, client: &SupabaseClient) -> Result<Vec<String>> {
    if !dir.is_dir() {
        return Err(Error::MissingSupabaseConfig(format!(
            "not a directory: {}",
            dir.display()
        )));
    }
    let mut paths = Vec::new();
    for entry in fs::read_dir(dir).map_err(|err| Error::MissingSupabaseConfig(err.to_string()))? {
        let entry = entry.map_err(|err| Error::MissingSupabaseConfig(err.to_string()))?;
        let path = entry.path();
        if path.is_file() {
            paths.push(path);
        }
    }
    if paths.is_empty() {
        return Err(Error::MissingSupabaseConfig(format!(
            "no files found in {}",
            dir.display()
        )));
    }
    upload_paths(&paths, client).await
}

pub async fn capture_and_upload(
    count: usize,
    device_idx: Option<u32>,
    out_dir: &Path,
    client: &SupabaseClient,
) -> Result<Vec<String>> {
    let captures = camera::capture_many(count, device_idx, out_dir)?;
    upload_paths(&captures, client).await
}
