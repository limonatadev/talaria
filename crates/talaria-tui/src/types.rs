use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;

use chrono::{DateTime, Local};
use talaria_core::config::EbaySettings;
use talaria_core::models::MarketplaceId;

#[derive(Debug, Clone)]
pub struct CaptureStatus {
    pub streaming: bool,
    pub device_index: i32,
    pub fps: f32,
    pub dropped_frames: u64,
    pub frame_size: Option<(i32, i32)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobStatus {
    InProgress,
    Completed,
    Failed,
}

impl std::fmt::Display for JobStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            JobStatus::InProgress => "active",
            JobStatus::Completed => "done",
            JobStatus::Failed => "failed",
        };
        write!(f, "{s}")
    }
}

#[derive(Debug, Clone)]
pub struct UploadJob {
    pub id: String,
    pub status: JobStatus,
    pub progress: f32,
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
    UploadProduct { product_id: String },
    Shutdown,
}

#[derive(Debug, Clone)]
pub enum PreviewCommand {
    SetEnabled(bool),
    ShowImage(Option<PathBuf>),
    Shutdown,
}

#[derive(Debug, Clone)]
pub enum PreviewEvent {
    Error(String),
    Unavailable(String),
}

#[derive(Debug, Clone)]
pub enum AppCommand {
    Capture(CaptureCommand),
    Preview(PreviewCommand),
    Upload(UploadCommand),
    Storage(StorageCommand),
    Shutdown,
}

#[derive(Debug, Clone)]
pub enum AppEvent {
    Capture(CaptureEvent),
    Preview(PreviewEvent),
    UploadJob(UploadJob),
    UploadFinished { product_id: String },
    Activity(ActivityEntry),
    Storage(StorageEvent),
}

#[derive(Debug, Clone)]
pub enum StorageCommand {
    CreateProductAndSession,
    ListProducts,
    StartSessionForProduct {
        product_id: String,
    },
    DeleteProduct {
        product_id: String,
    },
    SetProductContextText {
        product_id: String,
        text: String,
    },
    SetProductStructureJson {
        product_id: String,
        structure_json: serde_json::Value,
    },
    SetProductListings {
        product_id: String,
        listings: HashMap<String, crate::storage::MarketplaceListing>,
    },
    GenerateProductStructure {
        product_id: String,
        sku_alias: String,
    },
    GenerateProductListing {
        product_id: String,
        sku_alias: String,
        marketplace: MarketplaceId,
        settings: EbaySettings,
        condition: Option<String>,
        condition_id: Option<i32>,
        dry_run: bool,
        publish: bool,
    },
    SyncProductMedia {
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
    ToggleSessionFrameSelection {
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
    ProductDeleted {
        product_id: String,
        removed_sessions: usize,
    },
    SessionAbandoned {
        session_id: String,
        moved_to: String,
    },
    Error(String),
}
