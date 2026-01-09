use crate::error::{Error, Result};
use dirs::config_dir;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

pub const DEFAULT_BASE_URL: &str = "https://api.hermes-api.dev";
pub const ENV_BASE_URL: &str = "HERMES_BASE_URL";
pub const ENV_API_KEY: &str = "HERMES_API_KEY";
pub const ENV_SUPABASE_URL: &str = "SUPABASE_URL";
pub const ENV_SUPABASE_SERVICE_ROLE_KEY: &str = "SUPABASE_SERVICE_ROLE_KEY";
pub const ENV_SUPABASE_BUCKET: &str = "SUPABASE_BUCKET";
pub const ENV_SUPABASE_PUBLIC_BASE: &str = "SUPABASE_PUBLIC_BASE";
pub const ENV_SUPABASE_UPLOAD_PREFIX: &str = "SUPABASE_UPLOAD_PREFIX";
pub const ENV_EBAY_MARKETPLACE: &str = "EBAY_MARKETPLACE";
pub const ENV_EBAY_MERCHANT_LOCATION_KEY: &str = "EBAY_MERCHANT_LOCATION_KEY";
pub const ENV_EBAY_FULFILLMENT_POLICY_ID: &str = "EBAY_FULFILLMENT_POLICY_ID";
pub const ENV_EBAY_PAYMENT_POLICY_ID: &str = "EBAY_PAYMENT_POLICY_ID";
pub const ENV_EBAY_RETURN_POLICY_ID: &str = "EBAY_RETURN_POLICY_ID";
pub const DEFAULT_SUPABASE_BUCKET: &str = "images-bucket";
pub const DEFAULT_SUPABASE_UPLOAD_PREFIX: &str = "talaria";
pub const DEFAULT_EBAY_MARKETPLACE: &str = "EBAY_US";

/// Runtime configuration resolved from environment and optional config file.
#[derive(Debug, Clone)]
pub struct Config {
    pub base_url: String,
    pub api_key: Option<String>,
    pub supabase: Option<SupabaseConfig>,
    pub ebay: EbaySettings,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct ConfigFile {
    base_url: Option<String>,
    api_key: Option<String>,
    supabase_url: Option<String>,
    supabase_service_role_key: Option<String>,
    supabase_bucket: Option<String>,
    supabase_public_base: Option<String>,
    supabase_upload_prefix: Option<String>,
    ebay_marketplace: Option<String>,
    ebay_merchant_location_key: Option<String>,
    ebay_fulfillment_policy_id: Option<String>,
    ebay_payment_policy_id: Option<String>,
    ebay_return_policy_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConfigDoctor {
    pub base_url: String,
    pub api_key_redacted: Option<String>,
    pub source: String,
    pub supabase: Option<SupabaseDoctor>,
    pub ebay: EbaySettings,
}

#[derive(Debug, Clone, Serialize)]
pub struct SupabaseDoctor {
    pub supabase_url: String,
    pub bucket: String,
    pub upload_prefix: String,
    pub service_role_key_redacted: Option<String>,
    pub public_base: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SupabaseConfig {
    pub url: String,
    pub service_role_key: Option<String>,
    pub bucket: String,
    pub public_base: Option<String>,
    pub upload_prefix: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EbaySettings {
    pub marketplace: Option<String>,
    pub merchant_location_key: Option<String>,
    pub fulfillment_policy_id: Option<String>,
    pub payment_policy_id: Option<String>,
    pub return_policy_id: Option<String>,
}

impl Config {
    pub fn load() -> Result<Self> {
        let file_path = config_path();
        let file_config = file_path
            .as_ref()
            .and_then(|path| fs::read_to_string(path).ok())
            .map(|contents| toml::from_str::<ConfigFile>(&contents))
            .transpose()
            .map_err(|err| Error::InvalidConfig(format!("config parse error: {err}")))?;

        let base_url = std::env::var(ENV_BASE_URL)
            .ok()
            .or_else(|| file_config.as_ref().and_then(|c| c.base_url.clone()))
            .unwrap_or_else(|| DEFAULT_BASE_URL.to_string());

        let api_key = std::env::var(ENV_API_KEY)
            .ok()
            .or_else(|| file_config.as_ref().and_then(|c| c.api_key.clone()))
            .filter(|v| !v.trim().is_empty());

        let supabase = resolve_supabase(file_config.as_ref());
        let ebay = resolve_ebay(file_config.as_ref());

        Ok(Self {
            base_url,
            api_key,
            supabase,
            ebay,
        })
    }

    pub fn save(&self) -> Result<()> {
        let Some(path) = config_path() else {
            return Err(Error::InvalidConfig(
                "unable to determine config directory".into(),
            ));
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                Error::InvalidConfig(format!("failed to create config dir: {err}"))
            })?;
        }
        let file_config = ConfigFile {
            base_url: Some(self.base_url.clone()),
            api_key: self.api_key.clone(),
            supabase_url: self.supabase.as_ref().map(|s| s.url.clone()),
            supabase_service_role_key: self
                .supabase
                .as_ref()
                .and_then(|s| s.service_role_key.clone()),
            supabase_bucket: self.supabase.as_ref().map(|s| s.bucket.clone()),
            supabase_public_base: self.supabase.as_ref().and_then(|s| s.public_base.clone()),
            supabase_upload_prefix: self.supabase.as_ref().map(|s| s.upload_prefix.clone()),
            ebay_marketplace: self.ebay.marketplace.clone(),
            ebay_merchant_location_key: self.ebay.merchant_location_key.clone(),
            ebay_fulfillment_policy_id: self.ebay.fulfillment_policy_id.clone(),
            ebay_payment_policy_id: self.ebay.payment_policy_id.clone(),
            ebay_return_policy_id: self.ebay.return_policy_id.clone(),
        };
        let serialized = toml::to_string_pretty(&file_config)
            .map_err(|err| Error::InvalidConfig(format!("failed to serialize config: {err}")))?;
        fs::write(&path, serialized)
            .map_err(|err| Error::InvalidConfig(format!("failed to write config: {err}")))?;
        Ok(())
    }

    pub fn doctor(&self) -> ConfigDoctor {
        let source = if std::env::var(ENV_BASE_URL).is_ok() || std::env::var(ENV_API_KEY).is_ok() {
            "environment".to_string()
        } else {
            "config file / defaults".to_string()
        };
        ConfigDoctor {
            base_url: self.base_url.clone(),
            api_key_redacted: self.redacted_api_key(),
            source,
            supabase: self.supabase.as_ref().map(|s| SupabaseDoctor {
                supabase_url: s.url.clone(),
                bucket: s.bucket.clone(),
                upload_prefix: s.upload_prefix.clone(),
                service_role_key_redacted: s.service_role_key.as_ref().map(|v| redact(v)),
                public_base: s.public_base.clone(),
            }),
            ebay: self.ebay.clone(),
        }
    }

    pub fn redacted_api_key(&self) -> Option<String> {
        self.api_key.as_ref().map(|v| redact(v))
    }
}

fn resolve_supabase(file_config: Option<&ConfigFile>) -> Option<SupabaseConfig> {
    let supabase_url = std::env::var(ENV_SUPABASE_URL)
        .ok()
        .or_else(|| file_config.and_then(|c| c.supabase_url.clone()));

    let service_role_key = std::env::var(ENV_SUPABASE_SERVICE_ROLE_KEY)
        .ok()
        .or_else(|| file_config.and_then(|c| c.supabase_service_role_key.clone()))
        .filter(|s| !s.trim().is_empty());

    let bucket = std::env::var(ENV_SUPABASE_BUCKET)
        .ok()
        .or_else(|| file_config.and_then(|c| c.supabase_bucket.clone()))
        .unwrap_or_else(|| DEFAULT_SUPABASE_BUCKET.to_string());

    let public_base = std::env::var(ENV_SUPABASE_PUBLIC_BASE)
        .ok()
        .or_else(|| file_config.and_then(|c| c.supabase_public_base.clone()))
        .filter(|s| !s.trim().is_empty());

    let upload_prefix = std::env::var(ENV_SUPABASE_UPLOAD_PREFIX)
        .ok()
        .or_else(|| file_config.and_then(|c| c.supabase_upload_prefix.clone()))
        .unwrap_or_else(|| DEFAULT_SUPABASE_UPLOAD_PREFIX.to_string());

    supabase_url.map(|url| SupabaseConfig {
        url,
        service_role_key,
        bucket,
        public_base,
        upload_prefix,
    })
}

fn resolve_ebay(file_config: Option<&ConfigFile>) -> EbaySettings {
    let marketplace = std::env::var(ENV_EBAY_MARKETPLACE)
        .ok()
        .or_else(|| file_config.and_then(|c| c.ebay_marketplace.clone()))
        .or_else(|| Some(DEFAULT_EBAY_MARKETPLACE.to_string()))
        .filter(|v| !v.trim().is_empty());
    let merchant_location_key = std::env::var(ENV_EBAY_MERCHANT_LOCATION_KEY)
        .ok()
        .or_else(|| file_config.and_then(|c| c.ebay_merchant_location_key.clone()))
        .filter(|v| !v.trim().is_empty());
    let fulfillment_policy_id = std::env::var(ENV_EBAY_FULFILLMENT_POLICY_ID)
        .ok()
        .or_else(|| file_config.and_then(|c| c.ebay_fulfillment_policy_id.clone()))
        .filter(|v| !v.trim().is_empty());
    let payment_policy_id = std::env::var(ENV_EBAY_PAYMENT_POLICY_ID)
        .ok()
        .or_else(|| file_config.and_then(|c| c.ebay_payment_policy_id.clone()))
        .filter(|v| !v.trim().is_empty());
    let return_policy_id = std::env::var(ENV_EBAY_RETURN_POLICY_ID)
        .ok()
        .or_else(|| file_config.and_then(|c| c.ebay_return_policy_id.clone()))
        .filter(|v| !v.trim().is_empty());

    EbaySettings {
        marketplace,
        merchant_location_key,
        fulfillment_policy_id,
        payment_policy_id,
        return_policy_id,
    }
}

fn config_path() -> Option<PathBuf> {
    config_dir().map(|dir| dir.join("talaria").join("config.toml"))
}

fn redact(key: &str) -> String {
    if key.len() <= 4 {
        return "****".to_string();
    }
    let suffix = &key[key.len().saturating_sub(4)..];
    format!("****{}", suffix)
}
