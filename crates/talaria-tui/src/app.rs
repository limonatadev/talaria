use std::path::PathBuf;
use std::time::{Duration, Instant};

use chrono::Local;
use crossbeam_channel::Sender;
use crossterm::event::{KeyCode, KeyEvent};

use crate::types::{
    ActivityEntry, ActivityLog, AppCommand, AppEvent, CaptureCommand, CaptureEvent, CaptureStatus,
    EnrichCommand, EnrichJob, JobStatus, ListingDraft, ListingsCommand, LocalImage, PipelineStage,
    PreviewCommand, PreviewEvent, RemoteImage, Severity, UploadCommand, UploadJob,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppTab {
    Home,
    Capture,
    Curate,
    Upload,
    Enrich,
    Listings,
    Activity,
    Settings,
}

#[derive(Debug, Clone)]
pub struct Metrics {
    pub captured_today: u64,
    pub enriched_today: u64,
    pub listed_today: u64,
}

#[derive(Debug, Clone)]
pub struct AppConfigView {
    pub capture_dir: String,
    pub burst_default: usize,
    pub supabase_bucket: String,
    pub api_base_url: String,
}

#[derive(Debug, Clone)]
pub struct Toast {
    pub message: String,
    pub severity: Severity,
    pub expires_at: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivityFilter {
    All,
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub should_quit: bool,
    pub help_open: bool,
    pub active_tab: AppTab,
    pub camera_connected: bool,
    pub preview_enabled: bool,
    pub device_index: i32,
    pub burst_count: usize,
    pub capture_status: CaptureStatus,
    pub last_capture_path: Option<PathBuf>,
    pub last_burst_best: Option<PathBuf>,
    pub uploads: Vec<UploadJob>,
    pub enrich_jobs: Vec<EnrichJob>,
    pub listing_drafts: Vec<ListingDraft>,
    pub current_item: crate::types::CurrentItem,
    pub activity: ActivityLog,
    pub metrics: Metrics,
    pub toast: Option<Toast>,
    pub last_error: Option<ActivityEntry>,
    pub upload_selected: usize,
    pub enrich_selected: usize,
    pub listing_selected: usize,
    pub local_image_selected: usize,
    pub activity_filter: ActivityFilter,
    pub config_view: AppConfigView,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            should_quit: false,
            help_open: false,
            active_tab: AppTab::Home,
            camera_connected: false,
            preview_enabled: false,
            device_index: 0,
            burst_count: 10,
            capture_status: CaptureStatus {
                streaming: false,
                device_index: 0,
                fps: 0.0,
                dropped_frames: 0,
                frame_size: None,
            },
            last_capture_path: None,
            last_burst_best: None,
            uploads: Vec::new(),
            enrich_jobs: Vec::new(),
            listing_drafts: Vec::new(),
            current_item: crate::types::CurrentItem {
                id: Local::now().format("item-%Y%m%d-%H%M%S").to_string(),
                local_images: Vec::new(),
                selected_hero: None,
                uploaded_images: Vec::new(),
                hsuf_summary: None,
                listing_draft_summary: None,
                stage: PipelineStage::Captured,
            },
            activity: ActivityLog::new(200),
            metrics: Metrics {
                captured_today: 0,
                enriched_today: 0,
                listed_today: 0,
            },
            toast: None,
            last_error: None,
            upload_selected: 0,
            enrich_selected: 0,
            listing_selected: 0,
            local_image_selected: 0,
            activity_filter: ActivityFilter::All,
            config_view: AppConfigView {
                capture_dir: "./captures".to_string(),
                burst_default: 10,
                supabase_bucket: "TODO".to_string(),
                api_base_url: "TODO".to_string(),
            },
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent, command_tx: &Sender<AppCommand>) {
        if key.code == KeyCode::Char('q') {
            self.should_quit = true;
            let _ = command_tx.send(AppCommand::Shutdown);
            return;
        }

        if key.code == KeyCode::Char('?') {
            self.help_open = !self.help_open;
            return;
        }

        if self.help_open {
            if key.code == KeyCode::Esc || key.code == KeyCode::Char('?') {
                self.help_open = false;
            }
            return;
        }

        match key.code {
            KeyCode::Left | KeyCode::Char('h') => self.prev_tab(),
            KeyCode::Right | KeyCode::Char('l') => self.next_tab(),
            KeyCode::Char('1') => self.active_tab = AppTab::Home,
            KeyCode::Char('2') => self.active_tab = AppTab::Capture,
            KeyCode::Char('3') => self.active_tab = AppTab::Curate,
            KeyCode::Char('4') => self.active_tab = AppTab::Upload,
            KeyCode::Char('5') => self.active_tab = AppTab::Enrich,
            KeyCode::Char('6') => self.active_tab = AppTab::Listings,
            KeyCode::Char('7') => self.active_tab = AppTab::Activity,
            KeyCode::Char('8') => self.active_tab = AppTab::Settings,
            _ => {}
        }

        match self.active_tab {
            AppTab::Capture => self.handle_capture_keys(key, command_tx),
            AppTab::Curate => self.handle_curate_keys(key),
            AppTab::Upload => self.handle_upload_keys(key, command_tx),
            AppTab::Enrich => self.handle_enrich_keys(key, command_tx),
            AppTab::Listings => self.handle_listings_keys(key, command_tx),
            AppTab::Activity => self.handle_activity_keys(key),
            _ => {}
        }
    }

    pub fn apply_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Capture(capture_event) => self.apply_capture_event(capture_event),
            AppEvent::Preview(preview_event) => self.apply_preview_event(preview_event),
            AppEvent::UploadJob(job) => self.upsert_upload(job),
            AppEvent::EnrichJob(job) => self.upsert_enrich(job),
            AppEvent::ListingDraft(draft) => self.upsert_listing(draft),
            AppEvent::Activity(entry) => self.push_activity(entry),
            AppEvent::Toast { message, severity } => self.toast(message, severity),
        }
    }

    pub fn prune_toast(&mut self) {
        if let Some(toast) = &self.toast {
            if Instant::now() >= toast.expires_at {
                self.toast = None;
            }
        }
    }

    fn handle_capture_keys(&mut self, key: KeyEvent, command_tx: &Sender<AppCommand>) {
        match key.code {
            KeyCode::Char('s') => {
                let cmd = if self.capture_status.streaming {
                    CaptureCommand::StopStream
                } else {
                    CaptureCommand::StartStream
                };
                let _ = command_tx.send(AppCommand::Capture(cmd));
            }
            KeyCode::Char('p') => {
                self.preview_enabled = !self.preview_enabled;
                let _ = command_tx.send(AppCommand::Preview(PreviewCommand::SetEnabled(
                    self.preview_enabled,
                )));
            }
            KeyCode::Char('d') => {
                self.device_index = (self.device_index - 1).max(0);
                let _ = command_tx.send(AppCommand::Capture(CaptureCommand::SetDevice {
                    index: self.device_index,
                }));
            }
            KeyCode::Char('D') => {
                self.device_index += 1;
                let _ = command_tx.send(AppCommand::Capture(CaptureCommand::SetDevice {
                    index: self.device_index,
                }));
            }
            KeyCode::Char('c') => {
                let _ = command_tx.send(AppCommand::Capture(CaptureCommand::CaptureOne));
            }
            KeyCode::Char('b') => {
                let _ = command_tx.send(AppCommand::Capture(CaptureCommand::CaptureBurst {
                    n: self.burst_count,
                }));
            }
            _ => {}
        }
    }

    fn handle_curate_keys(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up => {
                if self.local_image_selected > 0 {
                    self.local_image_selected -= 1;
                }
            }
            KeyCode::Down => {
                if self.local_image_selected + 1 < self.current_item.local_images.len() {
                    self.local_image_selected += 1;
                }
            }
            KeyCode::Enter => {
                if let Some(image) = self
                    .current_item
                    .local_images
                    .get(self.local_image_selected)
                {
                    self.current_item.selected_hero = Some(image.path.clone());
                    self.current_item.stage = PipelineStage::Curated;
                    self.toast("Hero image set.".to_string(), Severity::Success);
                }
            }
            KeyCode::Char('d') => {
                if self.local_image_selected < self.current_item.local_images.len() {
                    self.current_item
                        .local_images
                        .remove(self.local_image_selected);
                    if self.local_image_selected > 0 {
                        self.local_image_selected -= 1;
                    }
                }
            }
            KeyCode::Char('r') => {
                self.toast("Rename is TODO (local-only)".to_string(), Severity::Info);
            }
            _ => {}
        }
    }

    fn handle_upload_keys(&mut self, key: KeyEvent, command_tx: &Sender<AppCommand>) {
        match key.code {
            KeyCode::Up => {
                if self.upload_selected > 0 {
                    self.upload_selected -= 1;
                }
            }
            KeyCode::Down => {
                if self.upload_selected + 1 < self.uploads.len() {
                    self.upload_selected += 1;
                }
            }
            KeyCode::Char('e') => {
                if let Some(hero) = &self.current_item.selected_hero {
                    let _ =
                        command_tx.send(AppCommand::Upload(UploadCommand::Enqueue(hero.clone())));
                } else {
                    self.toast("No hero image selected.".to_string(), Severity::Warning);
                }
            }
            KeyCode::Char('a') => {
                for image in &self.current_item.local_images {
                    let _ = command_tx.send(AppCommand::Upload(UploadCommand::Enqueue(
                        image.path.clone(),
                    )));
                }
            }
            KeyCode::Char('r') => {
                let _ = command_tx.send(AppCommand::Upload(UploadCommand::RetryFailed));
            }
            KeyCode::Char('x') => {
                if let Some(job) = self.uploads.get(self.upload_selected) {
                    let _ =
                        command_tx.send(AppCommand::Upload(UploadCommand::Cancel(job.id.clone())));
                }
            }
            _ => {}
        }
    }

    fn handle_enrich_keys(&mut self, key: KeyEvent, command_tx: &Sender<AppCommand>) {
        match key.code {
            KeyCode::Up => {
                if self.enrich_selected > 0 {
                    self.enrich_selected -= 1;
                }
            }
            KeyCode::Down => {
                if self.enrich_selected + 1 < self.enrich_jobs.len() {
                    self.enrich_selected += 1;
                }
            }
            KeyCode::Char('e') => {
                let urls: Vec<String> = self
                    .current_item
                    .uploaded_images
                    .iter()
                    .map(|img| img.url.clone())
                    .collect();
                if urls.is_empty() {
                    self.toast("No uploaded URLs yet.".to_string(), Severity::Warning);
                } else {
                    let _ = command_tx.send(AppCommand::Enrich(EnrichCommand::Enqueue(urls)));
                }
            }
            KeyCode::Char('r') => {
                let _ = command_tx.send(AppCommand::Enrich(EnrichCommand::RetryFailed));
            }
            KeyCode::Char('x') => {
                if let Some(job) = self.enrich_jobs.get(self.enrich_selected) {
                    let _ =
                        command_tx.send(AppCommand::Enrich(EnrichCommand::Cancel(job.id.clone())));
                }
            }
            _ => {}
        }
    }

    fn handle_listings_keys(&mut self, key: KeyEvent, command_tx: &Sender<AppCommand>) {
        match key.code {
            KeyCode::Up => {
                if self.listing_selected > 0 {
                    self.listing_selected -= 1;
                }
            }
            KeyCode::Down => {
                if self.listing_selected + 1 < self.listing_drafts.len() {
                    self.listing_selected += 1;
                }
            }
            KeyCode::Char('c') => {
                let _ = command_tx.send(AppCommand::Listings(ListingsCommand::CreateDraft {
                    marketplace: "eBay (TODO)".to_string(),
                }));
            }
            KeyCode::Char('p') => {
                if let Some(draft) = self.listing_drafts.get(self.listing_selected) {
                    let _ = command_tx.send(AppCommand::Listings(ListingsCommand::PushLive(
                        draft.id.clone(),
                    )));
                }
            }
            KeyCode::Char('e') => {
                if let Some(draft) = self.listing_drafts.get(self.listing_selected) {
                    let _ = command_tx.send(AppCommand::Listings(ListingsCommand::ExportJson(
                        draft.id.clone(),
                    )));
                }
            }
            _ => {}
        }
    }

    fn handle_activity_keys(&mut self, key: KeyEvent) {
        if key.code == KeyCode::Char('f') {
            self.activity_filter = match self.activity_filter {
                ActivityFilter::All => ActivityFilter::Info,
                ActivityFilter::Info => ActivityFilter::Success,
                ActivityFilter::Success => ActivityFilter::Warning,
                ActivityFilter::Warning => ActivityFilter::Error,
                ActivityFilter::Error => ActivityFilter::All,
            };
        }
        if key.code == KeyCode::Char('/') {
            self.toast("Search TODO".to_string(), Severity::Info);
        }
    }

    fn apply_capture_event(&mut self, event: CaptureEvent) {
        match event {
            CaptureEvent::Status(status) => {
                self.capture_status = status.clone();
                self.device_index = status.device_index;
                self.camera_connected = status.streaming || status.frame_size.is_some();
            }
            CaptureEvent::Error(message) => {
                self.last_error = Some(ActivityEntry {
                    at: Local::now(),
                    severity: Severity::Error,
                    message: message.clone(),
                });
                self.push_activity(ActivityEntry {
                    at: Local::now(),
                    severity: Severity::Error,
                    message,
                });
            }
            CaptureEvent::CaptureCompleted { path } => {
                let path_buf = PathBuf::from(path.clone());
                self.last_capture_path = Some(path_buf.clone());
                self.current_item.local_images.push(LocalImage {
                    path: path_buf,
                    created_at: Local::now(),
                    sharpness_score: None,
                });
                self.current_item.stage = PipelineStage::Captured;
                self.metrics.captured_today += 1;
                self.push_activity(ActivityEntry {
                    at: Local::now(),
                    severity: Severity::Success,
                    message: format!("Captured frame: {path}"),
                });
            }
            CaptureEvent::BurstCompleted {
                best_path,
                all_paths,
            } => {
                for path in all_paths {
                    self.current_item.local_images.push(LocalImage {
                        path: PathBuf::from(path),
                        created_at: Local::now(),
                        sharpness_score: None,
                    });
                }
                self.current_item.selected_hero = Some(PathBuf::from(best_path.clone()));
                self.current_item.stage = PipelineStage::Curated;
                self.last_burst_best = Some(PathBuf::from(best_path.clone()));
                self.metrics.captured_today += 1;
                self.push_activity(ActivityEntry {
                    at: Local::now(),
                    severity: Severity::Success,
                    message: format!("Burst captured, hero set: {best_path}"),
                });
            }
        }
    }

    fn apply_preview_event(&mut self, event: PreviewEvent) {
        match event {
            PreviewEvent::Error(message) | PreviewEvent::Unavailable(message) => {
                self.preview_enabled = false;
                self.toast(message, Severity::Warning);
            }
            PreviewEvent::RoiSelected(_) => {
                // TODO: forward ROI to capture thread.
            }
        }
    }

    fn upsert_upload(&mut self, job: UploadJob) {
        if let Some(existing) = self.uploads.iter_mut().find(|j| j.id == job.id) {
            *existing = job.clone();
        } else {
            self.uploads.push(job.clone());
        }

        if job.status == JobStatus::Completed {
            self.current_item.uploaded_images.push(RemoteImage {
                url: format!("TODO://{}", job.id),
                status: job.status,
            });
            self.current_item.stage = PipelineStage::Uploaded;
        }
    }

    fn upsert_enrich(&mut self, job: EnrichJob) {
        if let Some(existing) = self.enrich_jobs.iter_mut().find(|j| j.id == job.id) {
            *existing = job.clone();
        } else {
            self.enrich_jobs.push(job.clone());
        }

        if job.status == JobStatus::Completed {
            self.current_item.stage = PipelineStage::Enriched;
            self.metrics.enriched_today += 1;
        }
    }

    fn upsert_listing(&mut self, draft: ListingDraft) {
        if let Some(existing) = self.listing_drafts.iter_mut().find(|d| d.id == draft.id) {
            *existing = draft.clone();
        } else {
            self.listing_drafts.push(draft.clone());
        }

        if draft.status == JobStatus::Completed {
            self.current_item.stage = PipelineStage::Listed;
            self.metrics.listed_today += 1;
        }
    }

    fn push_activity(&mut self, entry: ActivityEntry) {
        self.activity.push(entry.clone());
        if entry.severity == Severity::Error {
            self.last_error = Some(entry);
        }
    }

    fn toast(&mut self, message: String, severity: Severity) {
        self.toast = Some(Toast {
            message,
            severity,
            expires_at: Instant::now() + Duration::from_secs(3),
        });
    }

    fn next_tab(&mut self) {
        self.active_tab = match self.active_tab {
            AppTab::Home => AppTab::Capture,
            AppTab::Capture => AppTab::Curate,
            AppTab::Curate => AppTab::Upload,
            AppTab::Upload => AppTab::Enrich,
            AppTab::Enrich => AppTab::Listings,
            AppTab::Listings => AppTab::Activity,
            AppTab::Activity => AppTab::Settings,
            AppTab::Settings => AppTab::Home,
        };
    }

    fn prev_tab(&mut self) {
        self.active_tab = match self.active_tab {
            AppTab::Home => AppTab::Settings,
            AppTab::Capture => AppTab::Home,
            AppTab::Curate => AppTab::Capture,
            AppTab::Upload => AppTab::Curate,
            AppTab::Enrich => AppTab::Upload,
            AppTab::Listings => AppTab::Enrich,
            AppTab::Activity => AppTab::Listings,
            AppTab::Settings => AppTab::Activity,
        };
    }
}
