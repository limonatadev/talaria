use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_with::skip_serializing_none;
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LlmModel {
    #[serde(rename = "gpt-5.2")]
    Gpt5_2,
    #[serde(rename = "gpt-5-mini")]
    Gpt5Mini,
    #[serde(rename = "gpt-5-nano")]
    Gpt5Nano,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmStageOptions {
    pub model: LlmModel,
    pub reasoning: Option<bool>,
    pub web_search: Option<bool>,
}

/// components.schemas.ApiError
#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    pub code: Option<String>,
    pub detail: Option<String>,
    pub error: String,
    pub fields: Option<HashMap<String, String>>,
    pub request_id: Option<String>,
}

/// components.schemas.HsufEnrichRequest
#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HsufEnrichRequest {
    pub images: Vec<String>,
    pub sku: Option<String>,
    pub context_text: Option<String>,
    pub prompt_rules: Option<String>,
    pub llm_ingest: Option<LlmStageOptions>,
}

/// components.schemas.HsufEnrichResponse
#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HsufEnrichResponse {
    pub product: Product,
    pub usage: Option<IngestUsage>,
}

/// components.schemas.IngestUsage
#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestUsage {
    pub input_tokens: Option<i32>,
    pub output_tokens: Option<i32>,
}

/// components.schemas.PublicListingRequest
#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicListingRequest {
    pub dry_run: Option<bool>,
    pub fulfillment_policy_id: String,
    pub images_source: ImagesSource,
    pub llm_aspects: Option<LlmStageOptions>,
    pub llm_ingest: Option<LlmStageOptions>,
    pub marketplace: Option<MarketplaceId>,
    pub merchant_location_key: String,
    pub overrides: Option<PublicPipelineOverrides>,
    pub payment_policy_id: String,
    pub publish: Option<bool>,
    pub return_policy_id: String,
    pub sku: Option<String>,
    pub use_signed_urls: Option<bool>,
}

/// components.schemas.ListingDraftRequest
#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListingDraftRequest {
    pub sku: String,
    pub merchant_location_key: String,
    pub fulfillment_policy_id: String,
    pub payment_policy_id: String,
    pub return_policy_id: String,
    pub marketplace: Option<MarketplaceId>,
    pub listing: ListingDraftInput,
    pub dry_run: Option<bool>,
    pub publish: Option<bool>,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListingDraftInput {
    pub title: String,
    pub description: String,
    pub price: f64,
    pub currency: String,
    pub images: Vec<String>,
    pub category_id: String,
    pub category_label: Option<String>,
    pub condition: String,
    pub condition_id: i32,
    pub aspects: BTreeMap<String, Vec<String>>,
    pub package: Option<ListingPackageInput>,
    pub quantity: Option<i32>,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListingPackageInput {
    pub weight: Option<ListingWeightInput>,
    pub dimensions: Option<ListingDimensionsInput>,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListingWeightInput {
    pub value: u32,
    pub unit: String,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListingDimensionsInput {
    pub height: f64,
    pub length: f64,
    pub width: f64,
    pub unit: String,
}

/// components.schemas.ContinueRequest
#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContinueRequest {
    pub fulfillment_policy_id: String,
    pub images_source: Option<ImagesSource>,
    pub llm_aspects: Option<LlmStageOptions>,
    pub llm_ingest: Option<LlmStageOptions>,
    pub marketplace: Option<MarketplaceId>,
    pub merchant_location_key: String,
    pub overrides: Option<PublicPipelineOverrides>,
    pub payment_policy_id: String,
    pub return_policy_id: String,
    pub sku: String,
}

/// components.schemas.ListingResponse
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListingResponse {
    pub listing_id: String,
    pub stages: Vec<StageReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductRecord {
    pub id: String,
    pub sku_alias: String,
    pub display_name: Option<String>,
    pub context_text: Option<String>,
    pub structure_json: Option<Value>,
    pub listings_json: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProductCreateRequest {
    pub id: Option<String>,
    pub sku_alias: Option<String>,
    pub display_name: Option<String>,
}

#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProductUpdateRequest {
    pub sku_alias: Option<String>,
    pub display_name: Option<String>,
    pub context_text: Option<String>,
    pub structure_json: Option<Value>,
    pub listings_json: Option<Value>,
}

/// components.schemas.StageReport
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageReport {
    pub elapsed_ms: i64,
    pub name: String,
    pub output: Value,
    pub timestamp: DateTime<Utc>,
}

/// components.schemas.PublicStageOutput
#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicStageOutput {
    pub summary: Option<String>,
    pub warnings: Option<Vec<String>>,
}

/// components.schemas.JobState encoded as a tagged enum on the `state` field.
#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "state")]
pub enum JobState {
    #[serde(rename = "queued")]
    Queued {},
    #[serde(rename = "running")]
    Running {},
    #[serde(rename = "completed")]
    Completed { result: ListingResponse },
    #[serde(rename = "failed")]
    Failed {
        error: String,
        stage: Option<String>,
    },
}

/// components.schemas.JobInfo
#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobInfo {
    #[serde(flatten)]
    pub state: JobState,
    pub created_at: DateTime<Utc>,
    pub id: String,
    pub max_retries: Option<i32>,
    pub request: PublicListingRequest,
    pub retry: Option<i32>,
    pub updated_at: DateTime<Utc>,
}

/// components.schemas.EnqueueResponse
#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnqueueResponse {
    pub job_id: String,
}

/// components.schemas.PricingQuote
#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricingQuote {
    pub breakdown: HashMap<String, i64>,
    pub credits_applied_cents: Option<i64>,
    pub credits_estimated: i64,
    pub enterprise: Option<bool>,
    pub net_due_cents: Option<i64>,
    pub tiers: Option<Vec<TierLine>>,
    pub unit_rate_cents: Option<i64>,
}

/// components.schemas.UsageSummary
#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageSummary {
    pub counters: UsageCounters,
    pub org_id: String,
    pub tiered: Option<TieredUsage>,
    pub window_from: Option<DateTime<Utc>>,
    pub window_to: Option<DateTime<Utc>>,
}

/// components.schemas.UsageCounters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageCounters {
    pub credits_consumed: i64,
    pub jobs_enqueued: i64,
    pub listings_run: i64,
}

/// components.schemas.TieredUsage
#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TieredUsage {
    pub cost_cents: i64,
    pub credit_balance_cents: i64,
    pub credits_applied_cents: i64,
    pub enterprise: bool,
    pub net_due_cents: i64,
    pub tiers: Vec<TierLine>,
    pub total_events: i64,
    pub total_units: i64,
}

/// components.schemas.TierLine
#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierLine {
    pub cost_cents: i64,
    pub enterprise: Option<bool>,
    pub from: i64,
    pub rate_cents: i64,
    pub to: Option<i64>,
    pub units: i64,
}

/// components.schemas.DeviceAuthStartResponse
#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceAuthStartResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub verification_uri_complete: String,
    pub expires_in: i64,
    pub interval: u64,
}

/// components.schemas.DeviceAuthStatus
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceAuthStatus {
    Pending,
    Authorized,
    Consumed,
    Expired,
}

/// components.schemas.DeviceAuthPollResponse
#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceAuthPollResponse {
    pub status: DeviceAuthStatus,
    pub access_token: Option<String>,
}

/// components.schemas.DeviceAuthPollRequest
#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceAuthPollRequest {
    pub device_code: String,
}

/// components.schemas.UserApiKeyCreateRequest
#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserApiKeyCreateRequest {
    pub name: String,
}

/// components.schemas.UserApiKeyCreateResponse
#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserApiKeyCreateResponse {
    pub id: String,
    pub name: String,
    pub prefix: String,
    pub secret: String,
}

/// components.schemas.HealthResponse
#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub service: String,
    pub git_sha: Option<String>,
    pub version: Option<String>,
}

/// components.schemas.CreateUploadRequest
#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateUploadRequest {
    pub content_length: Option<i64>,
    pub content_type: Option<String>,
    pub filename: String,
    pub metadata: Option<HashMap<String, Value>>,
    pub product_id: Option<String>,
    pub purpose: Option<MediaPurpose>,
    pub session_id: Option<String>,
    pub sha256: Option<String>,
}

/// components.schemas.UploadMethod
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UploadMethod {
    #[serde(rename = "PUT")]
    Put,
}

/// components.schemas.UploadSession
#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadSession {
    pub expires_at: DateTime<Utc>,
    pub headers: Option<HashMap<String, String>>,
    pub method: UploadMethod,
    pub object_key: String,
    pub upload_id: String,
    pub upload_url: String,
    pub url: Option<String>,
}

/// components.schemas.CompleteUploadRequest
#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompleteUploadRequest {
    pub etag: Option<String>,
    pub sha256: Option<String>,
}

/// components.schemas.CompleteUploadResponse
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompleteUploadResponse {
    pub media: Media,
}

/// components.schemas.MediaPurpose
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MediaPurpose {
    #[serde(rename = "product_image")]
    ProductImage,
    #[serde(rename = "hero")]
    Hero,
    #[serde(rename = "session_frame")]
    SessionFrame,
}

/// components.schemas.Media
#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Media {
    pub content_length: Option<i64>,
    pub content_type: Option<String>,
    pub created_at: DateTime<Utc>,
    pub filename: Option<String>,
    pub media_id: String,
    pub object_key: String,
    pub product_id: Option<String>,
    pub purpose: Option<MediaPurpose>,
    pub rank: Option<i32>,
    pub session_id: Option<String>,
    pub sha256: Option<String>,
    pub updated_at: DateTime<Utc>,
    pub url: String,
}

/// components.schemas.UpdateMediaRequest
#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateMediaRequest {
    pub metadata: Option<HashMap<String, Value>>,
    pub purpose: Option<MediaPurpose>,
    pub rank: Option<i32>,
}

/// components.schemas.ListMediaResponse
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListMediaResponse {
    pub items: Vec<Media>,
}

/// components.schemas.ImagesSource
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ImagesSource {
    Single(String),
    Multiple(Vec<String>),
}

/// components.schemas.ImageField
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ImageField {
    Single(String),
    Multiple(Vec<String>),
}

/// components.schemas.MarketplaceId
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MarketplaceId {
    #[serde(rename = "EBAY_US")]
    EbayUs,
    #[serde(rename = "EBAY_UK")]
    EbayUk,
    #[serde(rename = "EBAY_DE")]
    EbayDe,
}

/// components.schemas.PublicPipelineOverrides
#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicPipelineOverrides {
    pub category: Option<CategorySelectionInput>,
    pub condition: Option<String>,
    pub condition_id: Option<i32>,
    pub resolved_images: Option<Vec<String>>,
    pub product: Option<Value>,
}

/// components.schemas.CategorySelectionInput
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategorySelectionInput {
    pub confidence: f32,
    pub id: String,
    pub label: String,
    pub rationale: String,
    pub tree_id: String,
}

/// components.schemas.Product
#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Product {
    pub brand: Option<Brand>,
    pub color: Option<String>,
    pub depth: Option<QuantitativeValue>,
    pub description: Option<String>,
    pub height: Option<QuantitativeValue>,
    pub image: ImageField,
    pub material: Option<String>,
    pub mpn: Option<String>,
    pub name: String,
    pub offers: Offer,
    pub size: Option<SizeField>,
    pub sku: Option<String>,
    pub weight: Option<QuantitativeValue>,
    pub width: Option<QuantitativeValue>,
}

/// components.schemas.Brand
#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Brand {
    pub name: Option<String>,
}

/// components.schemas.QuantitativeValue
#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantitativeValue {
    pub unit_code: Option<String>,
    pub unit_text: Option<String>,
    pub value: Option<f64>,
}

/// components.schemas.Offer
#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Offer {
    pub price: Option<f64>,
    pub price_currency: Option<String>,
    pub price_specification: Option<UnitPriceSpecification>,
}

/// components.schemas.UnitPriceSpecification
#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnitPriceSpecification {
    pub price: Option<f64>,
    pub price_currency: Option<String>,
}

/// components.schemas.SizeField
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SizeField {
    Text(String),
    Quantity(QuantitativeValue),
    Spec(SizeSpecification),
}

/// components.schemas.SizeSpecification
#[skip_serializing_none]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SizeSpecification {
    pub name: Option<String>,
    pub size_group: Option<String>,
    pub size_system: Option<String>,
}
