use crate::config::SupabaseConfig;
use crate::error::{Error, Result};
use argon2::{Argon2, PasswordHash, PasswordVerifier};
use chrono::{DateTime, Utc};
use mime_guess::MimeGuess;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};
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

#[derive(Debug, Clone)]
pub struct ApiKeyContext {
    pub org_id: String,
    pub api_key_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupabaseProductRow {
    pub id: String,
    pub org_id: String,
    pub sku_alias: String,
    pub display_name: Option<String>,
    #[serde(default)]
    pub context_text: Option<String>,
    #[serde(default)]
    pub structure_json: Option<serde_json::Value>,
    #[serde(default)]
    pub listings_json: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SupabaseProductInsert {
    pub id: String,
    pub org_id: String,
    pub sku_alias: String,
    pub display_name: Option<String>,
    pub context_text: Option<String>,
    pub structure_json: Option<serde_json::Value>,
    pub listings_json: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[serde_with::skip_serializing_none]
#[derive(Debug, Clone, Serialize, Default)]
pub struct SupabaseProductUpdate {
    pub display_name: Option<String>,
    pub context_text: Option<String>,
    pub structure_json: Option<serde_json::Value>,
    pub listings_json: Option<serde_json::Value>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Deserialize)]
struct SupabaseApiKeyRow {
    id: String,
    org_id: String,
    hashed_key: String,
    revoked_at: Option<String>,
    expires_at: Option<String>,
}

#[derive(Clone)]
pub struct SupabaseDbClient {
    http: Client,
    base_url: Url,
    service_role_key: String,
}

impl SupabaseDbClient {
    pub fn from_config(config: &SupabaseConfig) -> Result<Self> {
        let base_url = config
            .url
            .parse::<Url>()
            .map_err(|err| Error::InvalidConfig(format!("invalid SUPABASE_URL: {err}")))?;
        let Some(key) = config.service_role_key.clone() else {
            return Err(Error::MissingSupabaseConfig(
                "SUPABASE_SERVICE_ROLE_KEY required for database access".into(),
            ));
        };

        let http = Client::builder()
            .user_agent("talaria-supabase-db/0.1")
            .build()
            .map_err(|err| {
                Error::InvalidConfig(format!("failed to build supabase db client: {err}"))
            })?;

        Ok(Self {
            http,
            base_url,
            service_role_key: key,
        })
    }

    pub async fn resolve_api_key_context(
        &self,
        presented_key: &str,
    ) -> Result<Option<ApiKeyContext>> {
        let Some(prefix) = derive_prefix(presented_key) else {
            return Ok(None);
        };
        let record = self.fetch_api_key_by_prefix(&prefix).await?;
        let Some(record) = record else {
            return Ok(None);
        };
        if record.revoked_at.is_some() {
            return Ok(None);
        }
        if let Some(expires_at) = &record.expires_at
            && let Ok(exp) = DateTime::parse_from_rfc3339(expires_at)
            && exp.with_timezone(&Utc) <= Utc::now()
        {
            return Ok(None);
        }

        let parsed = PasswordHash::new(&record.hashed_key)
            .map_err(|err| Error::InvalidConfig(format!("invalid api key hash: {err}")))?;
        if Argon2::default()
            .verify_password(presented_key.as_bytes(), &parsed)
            .is_err()
        {
            return Ok(None);
        }

        Ok(Some(ApiKeyContext {
            org_id: record.org_id,
            api_key_id: record.id,
        }))
    }

    pub async fn list_products(&self, org_id: &str) -> Result<Vec<SupabaseProductRow>> {
        let mut url = self.rest_url("products")?;
        {
            let mut q = url.query_pairs_mut();
            q.append_pair(
                "select",
                "id,org_id,sku_alias,display_name,context_text,structure_json,listings_json,created_at,updated_at",
            );
            q.append_pair("org_id", &format!("eq.{org_id}"));
            q.append_pair("order", "updated_at.desc");
        }
        self.get_json(url).await
    }

    pub async fn fetch_product(
        &self,
        org_id: &str,
        product_id: &str,
    ) -> Result<Option<SupabaseProductRow>> {
        let mut url = self.rest_url("products")?;
        {
            let mut q = url.query_pairs_mut();
            q.append_pair(
                "select",
                "id,org_id,sku_alias,display_name,context_text,structure_json,listings_json,created_at,updated_at",
            );
            q.append_pair("org_id", &format!("eq.{org_id}"));
            q.append_pair("id", &format!("eq.{product_id}"));
            q.append_pair("limit", "1");
        }
        let mut rows: Vec<SupabaseProductRow> = self.get_json(url).await?;
        Ok(rows.pop())
    }

    pub async fn create_product(
        &self,
        insert: &SupabaseProductInsert,
    ) -> Result<SupabaseProductRow> {
        let url = self.rest_url("products")?;
        let resp = self
            .http
            .post(url)
            .headers(self.auth_headers())
            .header("Prefer", "return=representation")
            .json(insert)
            .send()
            .await
            .map_err(Error::Http)?;
        self.parse_single_row(resp).await
    }

    pub async fn update_product(
        &self,
        org_id: &str,
        product_id: &str,
        update: &SupabaseProductUpdate,
    ) -> Result<SupabaseProductRow> {
        let mut url = self.rest_url("products")?;
        {
            let mut q = url.query_pairs_mut();
            q.append_pair("org_id", &format!("eq.{org_id}"));
            q.append_pair("id", &format!("eq.{product_id}"));
        }
        let resp = self
            .http
            .patch(url)
            .headers(self.auth_headers())
            .header("Prefer", "return=representation")
            .json(update)
            .send()
            .await
            .map_err(Error::Http)?;
        self.parse_single_row(resp).await
    }

    pub async fn delete_product(&self, org_id: &str, product_id: &str) -> Result<()> {
        let mut url = self.rest_url("products")?;
        {
            let mut q = url.query_pairs_mut();
            q.append_pair("org_id", &format!("eq.{org_id}"));
            q.append_pair("id", &format!("eq.{product_id}"));
        }
        let resp = self
            .http
            .delete(url)
            .headers(self.auth_headers())
            .send()
            .await
            .map_err(Error::Http)?;
        self.ensure_success(resp).await
    }

    async fn fetch_api_key_by_prefix(&self, prefix: &str) -> Result<Option<SupabaseApiKeyRow>> {
        let mut url = self.rest_url("api_keys")?;
        {
            let mut q = url.query_pairs_mut();
            q.append_pair("select", "id,org_id,hashed_key,revoked_at,expires_at");
            q.append_pair("prefix", &format!("eq.{prefix}"));
            q.append_pair("limit", "1");
        }
        let mut rows: Vec<SupabaseApiKeyRow> = self.get_json(url).await?;
        Ok(rows.pop())
    }

    async fn get_json<T: for<'de> Deserialize<'de>>(&self, url: Url) -> Result<T> {
        let resp = self
            .http
            .get(url)
            .headers(self.auth_headers())
            .send()
            .await
            .map_err(Error::Http)?;
        self.parse_json(resp).await
    }

    fn rest_url(&self, path: &str) -> Result<Url> {
        self.base_url
            .join(&format!("rest/v1/{path}"))
            .map_err(|err| Error::InvalidConfig(format!("invalid supabase rest url: {err}")))
    }

    fn auth_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", self.service_role_key))
                .unwrap_or_else(|_| HeaderValue::from_static("")),
        );
        headers.insert(
            "apikey",
            HeaderValue::from_str(&self.service_role_key)
                .unwrap_or_else(|_| HeaderValue::from_static("")),
        );
        headers
    }

    async fn parse_single_row(&self, resp: reqwest::Response) -> Result<SupabaseProductRow> {
        let mut rows: Vec<SupabaseProductRow> = self.parse_json(resp).await?;
        rows.pop().ok_or_else(|| Error::SupabaseDb {
            status: reqwest::StatusCode::NOT_FOUND,
            message: "supabase response missing row".to_string(),
        })
    }

    async fn parse_json<T: for<'de> Deserialize<'de>>(&self, resp: reqwest::Response) -> Result<T> {
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            let snippet = body.chars().take(200).collect::<String>();
            return Err(Error::SupabaseDb {
                status,
                message: snippet,
            });
        }
        resp.json::<T>().await.map_err(Error::Http)
    }

    async fn ensure_success(&self, resp: reqwest::Response) -> Result<()> {
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            let snippet = body.chars().take(200).collect::<String>();
            return Err(Error::SupabaseDb {
                status,
                message: snippet,
            });
        }
        Ok(())
    }
}

fn derive_prefix(presented: &str) -> Option<String> {
    if let Some(idx) = presented.find('_') {
        let parts: Vec<&str> = presented.split('_').collect();
        if parts.len() >= 3 {
            return Some(parts[1].trim().to_string());
        }
        return Some(presented[..idx].trim().to_string());
    }
    Some(presented.chars().take(8).collect::<String>())
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
