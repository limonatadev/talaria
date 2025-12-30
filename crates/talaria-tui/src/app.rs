use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use chrono::Local;
use crossbeam_channel::Sender;
use crossterm::event::{KeyCode, KeyEvent};

use crate::storage;
use crate::types::{
    ActivityEntry, ActivityLog, AppCommand, AppEvent, CaptureCommand, CaptureEvent, CaptureStatus,
    PreviewEvent, Severity, StorageCommand, StorageEvent,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppTab {
    Home,
    Capture,
    Curate,
    Upload,
    Enrich,
    Products,
    Activity,
    Settings,
}

#[derive(Debug, Clone)]
pub struct Toast {
    pub message: String,
    pub severity: Severity,
    pub expires_at: Instant,
}

#[derive(Debug, Clone)]
pub struct PickerState {
    pub open: bool,
    pub search: String,
    pub selected: usize,
    pub products: Vec<storage::ProductSummary>,
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub should_quit: bool,
    pub help_open: bool,
    pub active_tab: AppTab,

    pub captures_dir: PathBuf,
    pub stderr_log_path: Option<PathBuf>,

    pub camera_connected: bool,
    pub preview_enabled: bool,
    pub device_index: i32,
    pub burst_count: usize,
    pub capture_status: CaptureStatus,

    pub active_product: Option<storage::ProductManifest>,
    pub active_session: Option<storage::SessionManifest>,

    pub last_capture_rel: Option<String>,
    pub last_commit_message: Option<String>,
    pub last_error: Option<String>,

    pub activity: ActivityLog,
    pub toast: Option<Toast>,

    pub picker: PickerState,

    pub session_frame_selected: usize,
    pub pending_commands: Vec<AppCommand>,
}

impl AppState {
    pub fn new(captures_dir: PathBuf, stderr_log_path: Option<PathBuf>) -> Self {
        let mut activity = ActivityLog::new(200);
        if let Some(path) = &stderr_log_path {
            activity.push(ActivityEntry {
                at: Local::now(),
                severity: Severity::Info,
                message: format!("stderr redirected to {}", path.display()),
            });
        }
        Self {
            should_quit: false,
            help_open: false,
            active_tab: AppTab::Capture,
            captures_dir,
            stderr_log_path,
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
            active_product: None,
            active_session: None,
            last_capture_rel: None,
            last_commit_message: None,
            last_error: None,
            activity,
            toast: None,
            picker: PickerState {
                open: false,
                search: String::new(),
                selected: 0,
                products: Vec::new(),
            },
            session_frame_selected: 0,
            pending_commands: Vec::new(),
        }
    }

    pub fn drain_pending_commands(&mut self) -> Vec<AppCommand> {
        std::mem::take(&mut self.pending_commands)
    }

    pub fn prune_toast(&mut self) {
        if let Some(toast) = &self.toast {
            if Instant::now() >= toast.expires_at {
                self.toast = None;
            }
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

        if self.picker.open {
            self.handle_picker_key(key, command_tx);
            return;
        }

        // Tab-local actions first (to avoid conflicts like Curate: `h` = hero).
        match self.active_tab {
            AppTab::Capture => self.handle_capture_keys(key, command_tx),
            AppTab::Curate => self.handle_curate_keys(key, command_tx),
            AppTab::Activity => {
                if key.code == KeyCode::Char('f') {
                    self.toast("Filter TODO".to_string(), Severity::Info);
                }
            }
            _ => {}
        }

        match key.code {
            KeyCode::Left => self.prev_tab(),
            KeyCode::Right => self.next_tab(),
            KeyCode::Char('h') if self.active_tab != AppTab::Curate => self.prev_tab(),
            KeyCode::Char('l') => self.next_tab(),
            KeyCode::Char('1') => self.active_tab = AppTab::Home,
            KeyCode::Char('2') => self.active_tab = AppTab::Capture,
            KeyCode::Char('3') => self.active_tab = AppTab::Curate,
            KeyCode::Char('4') => self.active_tab = AppTab::Upload,
            KeyCode::Char('5') => self.active_tab = AppTab::Enrich,
            KeyCode::Char('6') => self.active_tab = AppTab::Products,
            KeyCode::Char('7') => self.active_tab = AppTab::Activity,
            KeyCode::Char('8') => self.active_tab = AppTab::Settings,
            _ => {}
        }
    }

    pub fn apply_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Capture(event) => self.apply_capture_event(event),
            AppEvent::Preview(event) => self.apply_preview_event(event),
            AppEvent::Storage(event) => self.apply_storage_event(event),
            AppEvent::Toast { message, severity } => self.toast(message, severity),
            AppEvent::Activity(entry) => self.activity.push(entry),
            other => {
                let _ = other;
            }
        }
    }

    fn apply_preview_event(&mut self, event: PreviewEvent) {
        match event {
            PreviewEvent::Unavailable(message) | PreviewEvent::Error(message) => {
                self.preview_enabled = false;
                self.toast(message, Severity::Warning);
            }
            PreviewEvent::RoiSelected(_) => {
                // TODO: ROI selection wiring.
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
                let _ = command_tx.send(AppCommand::Preview(
                    crate::types::PreviewCommand::SetEnabled(self.preview_enabled),
                ));
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
            KeyCode::Char('n') => {
                let _ =
                    command_tx.send(AppCommand::Storage(StorageCommand::CreateProductAndSession));
            }
            KeyCode::Enter => {
                self.picker.open = true;
                self.picker.search.clear();
                self.picker.selected = 0;
                let _ = command_tx.send(AppCommand::Storage(StorageCommand::ListProducts));
            }
            KeyCode::Char('x') => {
                if let Some(session) = &self.active_session {
                    let _ = command_tx.send(AppCommand::Storage(StorageCommand::CommitSession {
                        session_id: session.session_id.clone(),
                    }));
                } else {
                    self.toast(
                        "No active session to commit.".to_string(),
                        Severity::Warning,
                    );
                }
            }
            KeyCode::Esc => {
                if let Some(session) = &self.active_session {
                    let _ = command_tx.send(AppCommand::Storage(StorageCommand::AbandonSession {
                        session_id: session.session_id.clone(),
                    }));
                }
            }
            _ => {}
        }
    }

    fn handle_curate_keys(&mut self, key: KeyEvent, command_tx: &Sender<AppCommand>) {
        let Some(session) = &self.active_session else {
            if key.code == KeyCode::Char('n') {
                let _ =
                    command_tx.send(AppCommand::Storage(StorageCommand::CreateProductAndSession));
            }
            if key.code == KeyCode::Enter {
                self.picker.open = true;
                self.picker.search.clear();
                self.picker.selected = 0;
                let _ = command_tx.send(AppCommand::Storage(StorageCommand::ListProducts));
            }
            return;
        };

        let frame_count = session.frames.len();
        match key.code {
            KeyCode::Up => {
                if self.session_frame_selected > 0 {
                    self.session_frame_selected -= 1;
                }
            }
            KeyCode::Down => {
                if self.session_frame_selected + 1 < frame_count {
                    self.session_frame_selected += 1;
                }
            }
            KeyCode::Char('h') => {
                if let Some(frame) = session.frames.get(self.session_frame_selected) {
                    let _ = command_tx.send(AppCommand::Storage(StorageCommand::SetHeroPick {
                        session_id: session.session_id.clone(),
                        frame_rel_path: frame.rel_path.clone(),
                    }));
                }
            }
            KeyCode::Char('a') => {
                if let Some(frame) = session.frames.get(self.session_frame_selected) {
                    let _ = command_tx.send(AppCommand::Storage(StorageCommand::AddAnglePick {
                        session_id: session.session_id.clone(),
                        frame_rel_path: frame.rel_path.clone(),
                    }));
                }
            }
            KeyCode::Char('d') => {
                if let Some(frame) = session.frames.get(self.session_frame_selected) {
                    let _ =
                        command_tx.send(AppCommand::Storage(StorageCommand::DeleteSessionFrame {
                            session_id: session.session_id.clone(),
                            frame_rel_path: frame.rel_path.clone(),
                        }));
                }
            }
            KeyCode::Char('x') => {
                let _ = command_tx.send(AppCommand::Storage(StorageCommand::CommitSession {
                    session_id: session.session_id.clone(),
                }));
            }
            _ => {}
        }
    }

    fn handle_picker_key(&mut self, key: KeyEvent, command_tx: &Sender<AppCommand>) {
        match key.code {
            KeyCode::Esc => {
                self.picker.open = false;
            }
            KeyCode::Up => {
                if self.picker.selected > 0 {
                    self.picker.selected -= 1;
                }
            }
            KeyCode::Down => {
                if self.picker.selected + 1 < self.filtered_products().len() {
                    self.picker.selected += 1;
                }
            }
            KeyCode::Enter => {
                if let Some(product) = self.filtered_products().get(self.picker.selected) {
                    let _ = command_tx.send(AppCommand::Storage(
                        StorageCommand::StartSessionForProduct {
                            product_id: product.product_id.clone(),
                        },
                    ));
                    self.picker.open = false;
                }
            }
            KeyCode::Backspace => {
                self.picker.search.pop();
                self.picker.selected = 0;
            }
            KeyCode::Char(c) => {
                if !c.is_control() {
                    self.picker.search.push(c);
                    self.picker.selected = 0;
                }
            }
            _ => {}
        }
    }

    pub fn filtered_products(&self) -> Vec<storage::ProductSummary> {
        let q = self.picker.search.to_lowercase();
        if q.is_empty() {
            return self.picker.products.clone();
        }
        self.picker
            .products
            .iter()
            .cloned()
            .filter(|p| {
                p.sku_alias.to_lowercase().contains(&q)
                    || p.display_name
                        .as_ref()
                        .map(|d| d.to_lowercase().contains(&q))
                        .unwrap_or(false)
            })
            .collect()
    }

    fn apply_capture_event(&mut self, event: CaptureEvent) {
        match event {
            CaptureEvent::Status(status) => {
                self.capture_status = status.clone();
                self.device_index = status.device_index;
                self.camera_connected = status.streaming || status.frame_size.is_some();
            }
            CaptureEvent::Error(message) => {
                self.last_error = Some(message.clone());
                self.activity.push(ActivityEntry {
                    at: Local::now(),
                    severity: Severity::Error,
                    message,
                });
            }
            CaptureEvent::CaptureCompleted {
                path,
                created_at,
                sharpness_score,
            } => {
                let Some(session) = &self.active_session else {
                    self.toast(
                        "Captured frame but no active session.".to_string(),
                        Severity::Warning,
                    );
                    return;
                };
                let rel = self.make_session_rel(session, Path::new(&path));
                self.last_capture_rel = Some(rel.clone());
                self.activity.push(ActivityEntry {
                    at: Local::now(),
                    severity: Severity::Success,
                    message: format!("Captured {}", rel),
                });

                self.pending_commands.push(AppCommand::Storage(
                    StorageCommand::AppendSessionFrame {
                        session_id: session.session_id.clone(),
                        frame_rel_path: rel,
                        created_at,
                        sharpness_score,
                    },
                ));
            }
            CaptureEvent::BurstCompleted { best_path, frames } => {
                let Some(session) = &self.active_session else {
                    self.toast(
                        "Burst captured but no active session.".to_string(),
                        Severity::Warning,
                    );
                    return;
                };

                let mut best_rel = None;
                for frame in frames {
                    let rel = self.make_session_rel(session, Path::new(&frame.path));
                    if frame.path == best_path {
                        best_rel = Some(rel.clone());
                    }
                    self.pending_commands.push(AppCommand::Storage(
                        StorageCommand::AppendSessionFrame {
                            session_id: session.session_id.clone(),
                            frame_rel_path: rel,
                            created_at: frame.created_at,
                            sharpness_score: frame.sharpness_score,
                        },
                    ));
                }

                if let Some(best_rel) = best_rel {
                    self.pending_commands
                        .push(AppCommand::Storage(StorageCommand::SetHeroPick {
                            session_id: session.session_id.clone(),
                            frame_rel_path: best_rel.clone(),
                        }));
                    self.last_capture_rel = Some(best_rel);
                }

                self.toast("Burst saved.".to_string(), Severity::Success);
            }
        }
    }

    fn apply_storage_event(&mut self, event: StorageEvent) {
        match event {
            StorageEvent::ProductsListed(products) => {
                self.picker.products = products;
                self.picker.selected = 0;
            }
            StorageEvent::ProductSelected(product) => {
                self.active_product = Some(product);
            }
            StorageEvent::SessionStarted(session) => {
                let frames_dir =
                    storage::session_frames_dir(&self.captures_dir, &session.session_id);
                self.pending_commands
                    .push(AppCommand::Capture(CaptureCommand::SetOutputDir(
                        frames_dir,
                    )));
                self.pending_commands
                    .push(AppCommand::Capture(CaptureCommand::StartStream));
                self.active_session = Some(session);
                self.session_frame_selected = 0;
                self.active_tab = AppTab::Capture;
            }
            StorageEvent::SessionUpdated(session) => {
                self.active_session = Some(session);
            }
            StorageEvent::CommitCompleted {
                product,
                session,
                committed_count,
            } => {
                self.active_product = Some(product.clone());
                self.active_session = Some(session);
                self.last_commit_message = Some(format!(
                    "Committed {} image(s) to {}",
                    committed_count, product.sku_alias
                ));
                self.toast(
                    self.last_commit_message.clone().unwrap_or_default(),
                    Severity::Success,
                );
                self.pending_commands
                    .push(AppCommand::Capture(CaptureCommand::ClearOutputDir));
            }
            StorageEvent::SessionAbandoned {
                session_id,
                moved_to,
            } => {
                if self
                    .active_session
                    .as_ref()
                    .is_some_and(|s| s.session_id == session_id)
                {
                    self.active_session = None;
                }
                self.pending_commands
                    .push(AppCommand::Capture(CaptureCommand::ClearOutputDir));
                self.toast(
                    format!("Session abandoned → {}", moved_to),
                    Severity::Warning,
                );
            }
            StorageEvent::Error(message) => {
                self.last_error = Some(message.clone());
                self.toast(message, Severity::Error);
            }
        }
    }

    fn make_session_rel(&self, session: &storage::SessionManifest, full: &Path) -> String {
        let base = storage::session_dir(&self.captures_dir, &session.session_id);
        if let Ok(rel) = full.strip_prefix(&base) {
            return rel.to_string_lossy().to_string();
        }
        // fall back to filename under frames/
        let filename = full
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("frame.jpg");
        format!("frames/{filename}")
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
            AppTab::Enrich => AppTab::Products,
            AppTab::Products => AppTab::Activity,
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
            AppTab::Products => AppTab::Enrich,
            AppTab::Activity => AppTab::Products,
            AppTab::Settings => AppTab::Activity,
        };
    }
}
