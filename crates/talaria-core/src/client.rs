use crate::config::Config;
use crate::error::{Error, Result};
use crate::models::*;
use reqwest::header::{ACCEPT, AUTHORIZATION, HeaderMap, HeaderValue, RETRY_AFTER};
use reqwest::{Client, Method, StatusCode, Url};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::time::Duration;
use tokio::time::sleep;

const USER_AGENT: &str = "talaria/0.1";

#[derive(Clone)]
pub struct HermesClient {
    http: Client,
    base_url: Url,
    api_key: Option<String>,
}

impl HermesClient {
    pub fn new(config: Config) -> Result<Self> {
        let mut base = config
            .base_url
            .parse::<Url>()
            .map_err(|err| Error::InvalidConfig(format!("invalid base url: {err}")))?;
        if !base.as_str().ends_with('/') {
            base = base
                .join("/")
                .map_err(|err| Error::InvalidConfig(format!("invalid base url: {err}")))?;
        }

        let http = Client::builder()
            .timeout(Duration::from_secs(180))
            .user_agent(USER_AGENT)
            .build()
            .map_err(|err| Error::InvalidConfig(format!("failed to build client: {err}")))?;

        Ok(Self {
            http,
            base_url: base,
            api_key: config.api_key,
        })
    }

    pub fn base_url(&self) -> &Url {
        &self.base_url
    }

    pub fn has_api_key(&self) -> bool {
        self.api_key.is_some()
    }

    pub async fn health(&self) -> Result<HealthResponse> {
        self.request::<(), _>(Method::GET, "health", None, None, false, true)
            .await
    }

    pub async fn device_auth_start(&self) -> Result<DeviceAuthStartResponse> {
        self.request::<(), _>(
            Method::POST,
            "v1/auth/device/start",
            None,
            None,
            false,
            false,
        )
        .await
    }

    pub async fn device_auth_poll(&self, device_code: &str) -> Result<DeviceAuthPollResponse> {
        let body = DeviceAuthPollRequest {
            device_code: device_code.to_string(),
        };
        self.request(
            Method::POST,
            "v1/auth/device/poll",
            None,
            Some(&body),
            false,
            false,
        )
        .await
    }

    pub async fn create_user_api_key(
        &self,
        access_token: &str,
        name: &str,
    ) -> Result<UserApiKeyCreateResponse> {
        let body = UserApiKeyCreateRequest {
            name: name.to_string(),
        };
        self.request_user_auth(Method::POST, "user/api-keys", Some(&body), access_token)
            .await
    }

    pub async fn hsuf_enrich(
        &self,
        body: &HsufEnrichRequest,
        include_usage: bool,
    ) -> Result<HsufEnrichResponse> {
        let mut query = Vec::new();
        if include_usage {
            query.push(("include_usage".to_string(), "true".to_string()));
        }
        self.request(
            Method::POST,
            "hsuf/enrich",
            Some(query),
            Some(body),
            true,
            false,
        )
        .await
    }

    pub async fn create_listing(&self, body: &PublicListingRequest) -> Result<ListingResponse> {
        self.request(Method::POST, "listings", None, Some(body), true, false)
            .await
    }

    pub async fn enqueue_listing(&self, body: &PublicListingRequest) -> Result<EnqueueResponse> {
        self.request(Method::POST, "jobs/listings", None, Some(body), true, false)
            .await
    }

    pub async fn continue_listing(&self, body: &ContinueRequest) -> Result<ListingResponse> {
        self.request(
            Method::POST,
            "listings/continue",
            None,
            Some(body),
            true,
            false,
        )
        .await
    }

    pub async fn publish_listing_draft(
        &self,
        body: &ListingDraftRequest,
    ) -> Result<ListingResponse> {
        self.request(
            Method::POST,
            "listings/publish-draft",
            None,
            Some(body),
            true,
            false,
        )
        .await
    }

    pub async fn get_job_status(&self, id: &str) -> Result<JobInfo> {
        let path = format!("jobs/{id}");
        self.request(Method::GET, &path, None, Option::<&()>::None, true, true)
            .await
    }

    pub async fn pricing_quote(&self, body: &PublicListingRequest) -> Result<PricingQuote> {
        self.request(
            Method::POST,
            "v1/pricing/quote",
            None,
            Some(body),
            true,
            false,
        )
        .await
    }

    pub async fn usage(
        &self,
        org_id: Option<String>,
        from: Option<String>,
        to: Option<String>,
    ) -> Result<Vec<UsageSummary>> {
        let mut query = Vec::new();
        if let Some(org) = org_id {
            query.push(("org_id".to_string(), org));
        }
        if let Some(f) = from {
            query.push(("from".to_string(), f));
        }
        if let Some(t) = to {
            query.push(("to".to_string(), t));
        }
        self.request(
            Method::GET,
            "v1/usage",
            Some(query),
            Option::<&()>::None,
            true,
            true,
        )
        .await
    }

    pub async fn create_media_upload(&self, body: &CreateUploadRequest) -> Result<UploadSession> {
        self.request(
            Method::POST,
            "v1/media/uploads",
            None,
            Some(body),
            true,
            false,
        )
        .await
    }

    pub async fn complete_media_upload(
        &self,
        upload_id: &str,
        body: Option<&CompleteUploadRequest>,
    ) -> Result<CompleteUploadResponse> {
        let path = format!("v1/media/uploads/{upload_id}/complete");
        if let Some(b) = body {
            self.request(Method::POST, &path, None, Some(b), true, false)
                .await
        } else {
            self.request(Method::POST, &path, None, Option::<&()>::None, true, false)
                .await
        }
    }

    pub async fn abort_media_upload(&self, upload_id: &str) -> Result<()> {
        let path = format!("v1/media/uploads/{upload_id}/abort");
        self.request_no_content(Method::POST, &path, None, Option::<&()>::None, true, false)
            .await
    }

    pub async fn delete_media(&self, media_id: &str) -> Result<()> {
        let path = format!("v1/media/{media_id}");
        self.request_no_content(
            Method::DELETE,
            &path,
            None,
            Option::<&()>::None,
            true,
            false,
        )
        .await
    }

    pub async fn update_media(&self, media_id: &str, body: &UpdateMediaRequest) -> Result<Media> {
        let path = format!("v1/media/{media_id}");
        self.request(Method::PATCH, &path, None, Some(body), true, false)
            .await
    }

    pub async fn list_product_media(&self, product_id: &str) -> Result<ListMediaResponse> {
        let path = format!("v1/products/{product_id}/media");
        self.request(Method::GET, &path, None, Option::<&()>::None, true, true)
            .await
    }

    pub async fn list_products(&self) -> Result<Vec<ProductRecord>> {
        self.request(
            Method::GET,
            "v1/products",
            None,
            Option::<&()>::None,
            true,
            true,
        )
        .await
    }

    pub async fn create_product(&self, body: &ProductCreateRequest) -> Result<ProductRecord> {
        self.request(Method::POST, "v1/products", None, Some(body), true, false)
            .await
    }

    pub async fn get_product(&self, product_id: &str) -> Result<ProductRecord> {
        let path = format!("v1/products/{product_id}");
        self.request(Method::GET, &path, None, Option::<&()>::None, true, true)
            .await
    }

    pub async fn update_product(
        &self,
        product_id: &str,
        body: &ProductUpdateRequest,
    ) -> Result<ProductRecord> {
        let path = format!("v1/products/{product_id}");
        self.request(Method::PATCH, &path, None, Some(body), true, false)
            .await
    }

    pub async fn delete_product(&self, product_id: &str) -> Result<()> {
        let path = format!("v1/products/{product_id}");
        self.request_no_content(
            Method::DELETE,
            &path,
            None,
            Option::<&()>::None,
            true,
            false,
        )
        .await
    }

    async fn request<B, T>(
        &self,
        method: Method,
        path: &str,
        query: Option<Vec<(String, String)>>,
        body: Option<&B>,
        auth: bool,
        retry: bool,
    ) -> Result<T>
    where
        B: Serialize + ?Sized,
        T: DeserializeOwned,
    {
        let mut url = self
            .base_url
            .join(path)
            .map_err(|err| Error::InvalidConfig(format!("invalid url: {err}")))?;
        if let Some(q) = &query {
            let mut pairs = url.query_pairs_mut();
            for (key, value) in q {
                pairs.append_pair(key, value);
            }
        }

        if auth && self.api_key.is_none() {
            return Err(Error::MissingApiKey {
                endpoint: path.to_string(),
            });
        }

        let mut attempts = 0usize;
        let max_attempts = if retry { 3 } else { 1 };
        loop {
            attempts += 1;
            let mut headers = HeaderMap::new();
            headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
            if auth && let Some(key) = &self.api_key {
                // Keep the value out of logs.
                headers.insert(
                    "X-Hermes-Key",
                    HeaderValue::from_str(key).map_err(|_| {
                        Error::InvalidConfig("invalid characters in api key".into())
                    })?,
                );
            }

            let mut req = self
                .http
                .request(method.clone(), url.clone())
                .headers(headers);
            if let Some(b) = body {
                req = req.json(b);
            }

            let response = req.send().await?;
            let status = response.status();
            let headers = response.headers().clone();
            if status.is_success() {
                let parsed = response.json::<T>().await?;
                return Ok(parsed);
            }

            let request_id = headers
                .get("x-request-id")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());

            let text = response.text().await.unwrap_or_default();
            let api_error = serde_json::from_str::<ApiError>(&text).ok();
            let should_retry = retry && is_retryable(status);

            if should_retry && attempts < max_attempts {
                let delay = compute_backoff(attempts, headers.get(RETRY_AFTER));
                sleep(delay).await;
                continue;
            }

            return Err(Error::from_api(status, api_error, Some(text), request_id));
        }
    }

    async fn request_no_content<B>(
        &self,
        method: Method,
        path: &str,
        query: Option<Vec<(String, String)>>,
        body: Option<&B>,
        auth: bool,
        retry: bool,
    ) -> Result<()>
    where
        B: Serialize + ?Sized,
    {
        let mut url = self
            .base_url
            .join(path)
            .map_err(|err| Error::InvalidConfig(format!("invalid url: {err}")))?;
        if let Some(q) = &query {
            let mut pairs = url.query_pairs_mut();
            for (key, value) in q {
                pairs.append_pair(key, value);
            }
        }

        if auth && self.api_key.is_none() {
            return Err(Error::MissingApiKey {
                endpoint: path.to_string(),
            });
        }

        let mut attempts = 0usize;
        let max_attempts = if retry { 3 } else { 1 };
        loop {
            attempts += 1;
            let mut headers = HeaderMap::new();
            headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
            if auth && let Some(key) = &self.api_key {
                headers.insert(
                    "X-Hermes-Key",
                    HeaderValue::from_str(key).map_err(|_| {
                        Error::InvalidConfig("invalid characters in api key".into())
                    })?,
                );
            }

            let mut req = self
                .http
                .request(method.clone(), url.clone())
                .headers(headers);
            if let Some(b) = body {
                req = req.json(b);
            }

            let response = req.send().await?;
            let status = response.status();
            let headers = response.headers().clone();
            if status.is_success() {
                return Ok(());
            }

            let request_id = headers
                .get("x-request-id")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());

            let text = response.text().await.unwrap_or_default();
            let api_error = serde_json::from_str::<ApiError>(&text).ok();
            let should_retry = retry && is_retryable(status);

            if should_retry && attempts < max_attempts {
                let delay = compute_backoff(attempts, headers.get(RETRY_AFTER));
                sleep(delay).await;
                continue;
            }

            return Err(Error::from_api(status, api_error, Some(text), request_id));
        }
    }

    async fn request_user_auth<B, T>(
        &self,
        method: Method,
        path: &str,
        body: Option<&B>,
        access_token: &str,
    ) -> Result<T>
    where
        B: Serialize + ?Sized,
        T: DeserializeOwned,
    {
        let url = self
            .base_url
            .join(path)
            .map_err(|err| Error::InvalidConfig(format!("invalid url: {err}")))?;

        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", access_token))
                .map_err(|_| Error::InvalidConfig("invalid characters in access token".into()))?,
        );

        let mut req = self.http.request(method, url).headers(headers);
        if let Some(b) = body {
            req = req.json(b);
        }

        let response = req.send().await?;
        let status = response.status();
        let headers = response.headers().clone();
        if status.is_success() {
            let parsed = response.json::<T>().await?;
            return Ok(parsed);
        }

        let request_id = headers
            .get("x-request-id")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        let text = response.text().await.unwrap_or_default();
        let api_error = serde_json::from_str::<ApiError>(&text).ok();
        Err(Error::from_api(status, api_error, Some(text), request_id))
    }
}

fn is_retryable(status: StatusCode) -> bool {
    status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error()
}

fn compute_backoff(attempt: usize, retry_after: Option<&HeaderValue>) -> Duration {
    if let Some(header) = retry_after
        && let Ok(val) = header.to_str()
        && let Ok(secs) = val.parse::<u64>()
    {
        return Duration::from_secs(secs);
    }
    let base = 500u64 * (1 << (attempt.saturating_sub(1)).min(4));
    Duration::from_millis(base)
}
