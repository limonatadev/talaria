use crate::config::SupabaseConfig;
use crate::error::{Error, Result};
use mime_guess::MimeGuess;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use reqwest::{Client, Url};
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone)]
pub struct SupabaseClient {
    http: Client,
    base_url: Url,
    public_base: Url,
    bucket: String,
    service_role_key: String,
    upload_prefix: String,
}

impl SupabaseClient {
    pub fn from_config(config: &SupabaseConfig) -> Result<Self> {
        let base_url = config
            .url
            .parse::<Url>()
            .map_err(|err| Error::InvalidConfig(format!("invalid SUPABASE_URL: {err}")))?;
        let public_base_url = config
            .public_base
            .as_ref()
            .map(|s| {
                s.parse::<Url>().map_err(|err| {
                    Error::InvalidConfig(format!("invalid SUPABASE_PUBLIC_BASE: {err}"))
                })
            })
            .transpose()?
            .unwrap_or_else(|| base_url.clone());

        let Some(key) = config.service_role_key.clone() else {
            return Err(Error::MissingSupabaseConfig(
                "SUPABASE_SERVICE_ROLE_KEY required for uploads".into(),
            ));
        };

        let http = Client::builder()
            .user_agent("talaria-supabase/0.1")
            .build()
            .map_err(|err| {
                Error::InvalidConfig(format!("failed to build supabase client: {err}"))
            })?;

        Ok(Self {
            http,
            base_url,
            public_base: public_base_url,
            bucket: config.bucket.clone(),
            service_role_key: key,
            upload_prefix: config.upload_prefix.clone(),
        })
    }

    pub fn bucket(&self) -> &str {
        &self.bucket
    }

    pub fn upload_prefix(&self) -> &str {
        &self.upload_prefix
    }

    pub fn with_prefix(&self, custom: Option<String>) -> Self {
        let mut clone = self.clone();
        if let Some(pref) = custom {
            clone.upload_prefix = pref;
        }
        clone
    }

    pub async fn upload_image_file(&self, path: &Path) -> Result<String> {
        let data = fs::read(path).map_err(|err| {
            Error::MissingSupabaseConfig(format!("read error {}: {err}", path.display()))
        })?;
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("image.jpg")
            .to_string();
        self.upload_image_bytes(&name, data).await
    }

    pub async fn upload_image_bytes(&self, filename_hint: &str, bytes: Vec<u8>) -> Result<String> {
        let object_path = format!(
            "{}/{}-{}",
            self.upload_prefix.trim_end_matches('/'),
            timestamp_ms(),
            sanitize_filename(filename_hint)
        );
        let url = self
            .base_url
            .join(&format!(
                "storage/v1/object/{}/{}",
                self.bucket, object_path
            ))
            .map_err(|err| Error::InvalidConfig(format!("invalid supabase upload url: {err}")))?;

        let mime = MimeGuess::from_path(filename_hint)
            .first_raw()
            .unwrap_or("application/octet-stream");

        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", self.service_role_key))
                .map_err(|_| Error::InvalidConfig("invalid supabase key".into()))?,
        );
        headers.insert(
            "apikey",
            HeaderValue::from_str(&self.service_role_key)
                .map_err(|_| Error::InvalidConfig("invalid supabase key".into()))?,
        );
        headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_str(mime)
                .map_err(|_| Error::InvalidConfig("invalid mime type".into()))?,
        );

        let resp = self
            .http
            .post(url)
            .headers(headers)
            .body(bytes)
            .send()
            .await
            .map_err(Error::Http)?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            let snippet = body.chars().take(200).collect::<String>();
            return Err(Error::SupabaseUpload {
                status,
                message: snippet,
            });
        }

        Ok(self.public_url(&object_path))
    }

    pub fn public_url(&self, object_path: &str) -> String {
        format!(
            "{}/storage/v1/object/public/{}/{}",
            self.public_base.as_str().trim_end_matches('/'),
            self.bucket,
            object_path
        )
    }
}

fn sanitize_filename(name: &str) -> String {
    let clean = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '.' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>();
    if clean.is_empty() {
        "image.jpg".to_string()
    } else {
        clean
    }
}

fn timestamp_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}
