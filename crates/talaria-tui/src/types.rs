use std::collections::VecDeque;
use std::path::PathBuf;

use chrono::{DateTime, Local};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoiRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone)]
pub struct CaptureStatus {
    pub streaming: bool,
    pub device_index: i32,
    pub fps: f32,
    pub dropped_frames: u64,
    pub frame_size: Option<(i32, i32)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineStage {
    Captured,
    Curated,
    Uploaded,
    Enriched,
    ReadyToList,
    Listed,
    Error,
}

#[derive(Debug, Clone)]
pub struct LocalImage {
    pub path: PathBuf,
    pub created_at: DateTime<Local>,
    pub sharpness_score: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct RemoteImage {
    pub url: String,
    pub status: JobStatus,
}

#[derive(Debug, Clone)]
pub struct HsufSummary {
    pub title: String,
    pub category_hint: String,
    pub confidence: Option<f32>,
}

#[derive(Debug, Clone)]
pub struct ListingSummary {
    pub marketplace: String,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct CurrentItem {
    pub id: String,
    pub local_images: Vec<LocalImage>,
    pub selected_hero: Option<PathBuf>,
    pub uploaded_images: Vec<RemoteImage>,
    pub hsuf_summary: Option<HsufSummary>,
    pub listing_draft_summary: Option<ListingSummary>,
    pub stage: PipelineStage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    Canceled,
}

#[derive(Debug, Clone)]
pub struct UploadJob {
    pub id: String,
    pub path: PathBuf,
    pub status: JobStatus,
    pub progress: f32,
    pub retries: u32,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct EnrichJob {
    pub id: String,
    pub image_urls: Vec<String>,
    pub status: JobStatus,
    pub started_at: Option<DateTime<Local>>,
    pub finished_at: Option<DateTime<Local>>,
    pub usage_estimate: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ListingDraft {
    pub id: String,
    pub marketplace: String,
    pub status: JobStatus,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Debug, Clone)]
pub struct ActivityEntry {
    pub at: DateTime<Local>,
    pub severity: Severity,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct ActivityLog {
    pub entries: VecDeque<ActivityEntry>,
    pub capacity: usize,
}

impl ActivityLog {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub fn push(&mut self, entry: ActivityEntry) {
        if self.entries.len() == self.capacity {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
    }
}

#[derive(Debug, Clone)]
pub enum CaptureCommand {
    StartStream,
    StopStream,
    SetDevice { index: i32 },
    SetOutputDir(PathBuf),
    ClearOutputDir,
    CaptureOne,
    CaptureBurst { n: usize },
    SetRoi(Option<RoiRect>),
    Shutdown,
}

#[derive(Debug, Clone)]
pub enum CaptureEvent {
    Status(CaptureStatus),
    Error(String),
    CaptureCompleted {
        path: String,
        created_at: DateTime<Local>,
        sharpness_score: Option<f64>,
    },
    BurstCompleted {
        best_path: String,
        frames: Vec<CapturedFrame>,
    },
}

#[derive(Debug, Clone)]
pub struct CapturedFrame {
    pub path: String,
    pub created_at: DateTime<Local>,
    pub sharpness_score: Option<f64>,
}

#[derive(Debug, Clone)]
pub enum UploadCommand {
    Enqueue(PathBuf),
    EnqueueAllCurrent,
    RetryFailed,
    Cancel(String),
    Shutdown,
}

#[derive(Debug, Clone)]
pub enum EnrichCommand {
    Enqueue(Vec<String>),
    RetryFailed,
    Cancel(String),
    Shutdown,
}

#[derive(Debug, Clone)]
pub enum ListingsCommand {
    CreateDraft { marketplace: String },
    PushLive(String),
    ExportJson(String),
    Shutdown,
}

#[derive(Debug, Clone)]
pub enum PreviewCommand {
    SetEnabled(bool),
    Shutdown,
}

#[derive(Debug, Clone)]
pub enum PreviewEvent {
    RoiSelected(RoiRect),
    Error(String),
    Unavailable(String),
}

#[derive(Debug, Clone)]
pub enum AppCommand {
    Capture(CaptureCommand),
    Preview(PreviewCommand),
    Upload(UploadCommand),
    Enrich(EnrichCommand),
    Listings(ListingsCommand),
    Storage(StorageCommand),
    Shutdown,
}

#[derive(Debug, Clone)]
pub enum AppEvent {
    Capture(CaptureEvent),
    Preview(PreviewEvent),
    UploadJob(UploadJob),
    EnrichJob(EnrichJob),
    ListingDraft(ListingDraft),
    Activity(ActivityEntry),
    Toast { message: String, severity: Severity },
    Storage(StorageEvent),
}

#[derive(Debug, Clone)]
pub enum StorageCommand {
    CreateProductAndSession,
    ListProducts,
    StartSessionForProduct {
        product_id: String,
    },
    AbandonSession {
        session_id: String,
    },
    CommitSession {
        session_id: String,
    },
    AppendSessionFrame {
        session_id: String,
        frame_rel_path: String,
        created_at: DateTime<Local>,
        sharpness_score: Option<f64>,
    },
    SetHeroPick {
        session_id: String,
        frame_rel_path: String,
    },
    AddAnglePick {
        session_id: String,
        frame_rel_path: String,
    },
    DeleteSessionFrame {
        session_id: String,
        frame_rel_path: String,
    },
    Shutdown,
}

#[derive(Debug, Clone)]
pub enum StorageEvent {
    ProductsListed(Vec<crate::storage::ProductSummary>),
    ProductSelected(crate::storage::ProductManifest),
    SessionStarted(crate::storage::SessionManifest),
    SessionUpdated(crate::storage::SessionManifest),
    CommitCompleted {
        product: crate::storage::ProductManifest,
        session: crate::storage::SessionManifest,
        committed_count: usize,
    },
    SessionAbandoned {
        session_id: String,
        moved_to: String,
    },
    Error(String),
}
