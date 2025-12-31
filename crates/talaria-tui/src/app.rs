use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::PreviewCommand;
use crate::storage;
use crate::types::{
    ActivityEntry, ActivityLog, AppCommand, AppEvent, CaptureCommand, CaptureEvent, CaptureStatus,
    JobStatus, PreviewEvent, Severity, StorageCommand, StorageEvent, UploadCommand, UploadJob,
};
use chrono::Local;
use crossbeam_channel::Sender;
use crossterm::event::{KeyCode, KeyEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppTab {
    Home,
    Products,
    Activity,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductsMode {
    Grid,
    Workspace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductsSubTab {
    Context,
    Structure,
    Listings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextFocus {
    Images,
    Text,
}

#[derive(Debug, Clone)]
pub struct Toast {
    pub message: String,
    pub severity: Severity,
    pub expires_at: Instant,
}

#[derive(Debug, Clone)]
pub struct DeleteConfirm {
    pub product_id: String,
    pub sku_alias: String,
    pub expires_at: Instant,
}

#[derive(Debug, Clone)]
pub struct PickerState {
    pub open: bool,
    pub search: String,
    pub selected: usize,
    pub products: Vec<storage::ProductSummary>,
}

#[derive(Debug, Clone, Default)]
pub struct ConfigInfo {
    pub base_url: Option<String>,
    pub hermes_api_key_present: bool,
    pub online_ready: bool,
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
    pub delete_confirm: Option<DeleteConfirm>,

    pub picker: PickerState,

    pub config: ConfigInfo,

    pub uploads: Vec<UploadJob>,
    pub upload_selected: usize,
    pub product_grid_selected: usize,
    pub product_grid_cols: usize,
    pub products_mode: ProductsMode,
    pub products_subtab: ProductsSubTab,
    pub context_focus: ContextFocus,

    pub session_frame_selected: usize,
    pub context_text: String,
    pub text_editing: bool,
    pub pending_commands: Vec<AppCommand>,
}

impl AppState {
    pub fn new(
        captures_dir: PathBuf,
        stderr_log_path: Option<PathBuf>,
        config: ConfigInfo,
        startup_warnings: Vec<String>,
    ) -> Self {
        let mut activity = ActivityLog::new(200);
        if let Some(path) = &stderr_log_path {
            activity.push(ActivityEntry {
                at: Local::now(),
                severity: Severity::Info,
                message: format!("stderr redirected to {}", path.display()),
            });
        }
        for warning in startup_warnings {
            activity.push(ActivityEntry {
                at: Local::now(),
                severity: Severity::Warning,
                message: warning,
            });
        }
        Self {
            should_quit: false,
            help_open: false,
            active_tab: AppTab::Home,
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
            delete_confirm: None,
            picker: PickerState {
                open: false,
                search: String::new(),
                selected: 0,
                products: Vec::new(),
            },
            config,
            uploads: Vec::new(),
            upload_selected: 0,
            product_grid_selected: 0,
            product_grid_cols: 3,
            products_mode: ProductsMode::Grid,
            products_subtab: ProductsSubTab::Context,
            context_focus: ContextFocus::Images,
            session_frame_selected: 0,
            context_text: String::new(),
            text_editing: false,
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
        if let Some(confirm) = &self.delete_confirm {
            if Instant::now() >= confirm.expires_at {
                self.delete_confirm = None;
            }
        }
    }

    fn handle_delete_confirmation(
        &mut self,
        key: KeyEvent,
        command_tx: &Sender<AppCommand>,
    ) -> bool {
        let Some(confirm) = &self.delete_confirm else {
            return false;
        };
        if Instant::now() >= confirm.expires_at {
            self.delete_confirm = None;
            return false;
        }
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                let product_id = confirm.product_id.clone();
                self.delete_confirm = None;
                let _ = command_tx.send(AppCommand::Storage(StorageCommand::DeleteProduct {
                    product_id,
                }));
                self.toast("Deleting product...".to_string(), Severity::Warning);
                true
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                self.delete_confirm = None;
                self.toast("Delete canceled.".to_string(), Severity::Info);
                true
            }
            _ => {
                self.delete_confirm = None;
                false
            }
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent, command_tx: &Sender<AppCommand>) {
        if self.text_editing
            && self.active_tab == AppTab::Products
            && self.products_mode == ProductsMode::Workspace
            && self.products_subtab == ProductsSubTab::Context
            && self.context_focus == ContextFocus::Text
        {
            if self.handle_text_edit_keys(key, command_tx) {
                return;
            }
        }

        if key.code == KeyCode::Char('q') {
            self.should_quit = true;
            let _ = command_tx.send(AppCommand::Shutdown);
            return;
        }

        if key.code == KeyCode::Char('?') {
            self.help_open = !self.help_open;
            return;
        }

        if self.handle_delete_confirmation(key, command_tx) {
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

        // Tab-local actions first.
        match self.active_tab {
            AppTab::Products => self.handle_products_keys(key, command_tx),
            AppTab::Activity => {
                if key.code == KeyCode::Char('f') {
                    self.toast("Filter TODO".to_string(), Severity::Info);
                }
            }
            _ => {}
        }

        let prev_tab = self.active_tab;
        match key.code {
            KeyCode::Left if self.active_tab != AppTab::Products => self.prev_tab(),
            KeyCode::Right if self.active_tab != AppTab::Products => self.next_tab(),
            KeyCode::Char('h') => self.prev_tab(),
            KeyCode::Char('l') => self.next_tab(),
            KeyCode::Char('1') => self.active_tab = AppTab::Home,
            KeyCode::Char('2') => self.active_tab = AppTab::Products,
            KeyCode::Char('3') => self.active_tab = AppTab::Activity,
            KeyCode::Char('4') => self.active_tab = AppTab::Settings,
            _ => {}
        }
        if self.active_tab != prev_tab && self.active_tab == AppTab::Products {
            self.products_mode = if self.active_product.is_some() {
                ProductsMode::Workspace
            } else {
                ProductsMode::Grid
            };
            let _ = command_tx.send(AppCommand::Storage(StorageCommand::ListProducts));
        }
    }

    fn save_context_text(&mut self, command_tx: &Sender<AppCommand>) {
        let Some(product) = &self.active_product else {
            return;
        };
        let text = self.context_text.clone();
        let _ = command_tx.send(AppCommand::Storage(StorageCommand::SetProductContextText {
            product_id: product.product_id.clone(),
            text,
        }));
    }

    fn handle_text_edit_keys(&mut self, key: KeyEvent, command_tx: &Sender<AppCommand>) -> bool {
        if !self.text_editing {
            return false;
        }
        match key.code {
            KeyCode::Esc => {
                self.text_editing = false;
                self.save_context_text(command_tx);
                self.toast("Text saved.".to_string(), Severity::Success);
                true
            }
            KeyCode::Enter => {
                self.context_text.push('\n');
                true
            }
            KeyCode::Backspace => {
                self.context_text.pop();
                true
            }
            KeyCode::Char(c) => {
                self.context_text.push(c);
                true
            }
            KeyCode::Delete | KeyCode::Tab | KeyCode::BackTab => true,
            _ => true,
        }
    }

    fn queue_image_preview(&mut self) {
        let path = self.preview_image_path();
        self.pending_commands
            .push(AppCommand::Preview(PreviewCommand::ShowImage(path)));
    }

    fn preview_image_path(&self) -> Option<PathBuf> {
        if self.products_mode != ProductsMode::Workspace {
            return None;
        }
        if self.products_subtab != ProductsSubTab::Context {
            return None;
        }
        if self.context_focus != ContextFocus::Images {
            return None;
        }
        let session = self.active_session.as_ref()?;
        let frame = session.frames.get(self.session_frame_selected)?;
        Some(storage::session_dir(&self.captures_dir, &session.session_id).join(&frame.rel_path))
    }

    pub fn apply_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Capture(event) => self.apply_capture_event(event),
            AppEvent::Preview(event) => self.apply_preview_event(event),
            AppEvent::Storage(event) => self.apply_storage_event(event),
            AppEvent::UploadJob(job) => self.apply_upload_job(job),
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
            KeyCode::Up => {
                if self.context_focus == ContextFocus::Images && self.session_frame_selected > 0 {
                    self.session_frame_selected -= 1;
                    self.queue_image_preview();
                }
            }
            KeyCode::Down => {
                if self.context_focus == ContextFocus::Images {
                    if let Some(session) = &self.active_session {
                        if self.session_frame_selected + 1 < session.frames.len() {
                            self.session_frame_selected += 1;
                            self.queue_image_preview();
                        }
                    }
                }
            }
            KeyCode::Char('s') => {
                let enable = !self.capture_status.streaming;
                let cmd = if enable {
                    CaptureCommand::StartStream
                } else {
                    CaptureCommand::StopStream
                };
                let _ = command_tx.send(AppCommand::Capture(cmd));
                self.preview_enabled = enable;
                let _ = command_tx.send(AppCommand::Preview(
                    crate::types::PreviewCommand::SetEnabled(enable),
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
            KeyCode::Backspace | KeyCode::Delete => {
                if self.context_focus != ContextFocus::Images {
                    return;
                }
                if let Some(session) = &self.active_session {
                    if let Some(frame) = session.frames.get(self.session_frame_selected) {
                        let _ = command_tx.send(AppCommand::Storage(
                            StorageCommand::DeleteSessionFrame {
                                session_id: session.session_id.clone(),
                                frame_rel_path: frame.rel_path.clone(),
                            },
                        ));
                        self.queue_image_preview();
                    } else {
                        self.toast("No image selected.".to_string(), Severity::Warning);
                    }
                }
            }
            KeyCode::Char('n') => {
                let _ =
                    command_tx.send(AppCommand::Storage(StorageCommand::CreateProductAndSession));
            }
            KeyCode::Char('x') => {
                if let Some(session) = &self.active_session {
                    if session.picks.selected_rel_paths.is_empty()
                        && session.picks.hero_rel_path.is_none()
                        && session.picks.angle_rel_paths.is_empty()
                    {
                        self.toast(
                            "Select images in Structure (Enter) before committing.".to_string(),
                            Severity::Warning,
                        );
                        return;
                    }
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
            KeyCode::Enter | KeyCode::Char(' ') => {
                if let Some(frame) = session.frames.get(self.session_frame_selected) {
                    let _ = command_tx.send(AppCommand::Storage(
                        StorageCommand::ToggleSessionFrameSelection {
                            session_id: session.session_id.clone(),
                            frame_rel_path: frame.rel_path.clone(),
                        },
                    ));
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
                if session.picks.selected_rel_paths.is_empty()
                    && session.picks.hero_rel_path.is_none()
                    && session.picks.angle_rel_paths.is_empty()
                {
                    self.toast(
                        "Select images with Enter before committing.".to_string(),
                        Severity::Warning,
                    );
                    return;
                }
                let _ = command_tx.send(AppCommand::Storage(StorageCommand::CommitSession {
                    session_id: session.session_id.clone(),
                }));
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
            KeyCode::Char('u') => {
                let Some(product) = &self.active_product else {
                    self.toast("No active product selected.".to_string(), Severity::Warning);
                    return;
                };
                let _ = command_tx.send(AppCommand::Upload(UploadCommand::UploadProduct {
                    product_id: product.product_id.clone(),
                }));
            }
            _ => {}
        }
    }

    fn handle_products_keys(&mut self, key: KeyEvent, command_tx: &Sender<AppCommand>) {
        match self.products_mode {
            ProductsMode::Grid => {
                let product_count = self.picker.products.len();
                let cols = self.product_grid_cols.max(1);
                match key.code {
                    KeyCode::Left => {
                        if self.product_grid_selected > 0 {
                            self.product_grid_selected -= 1;
                        }
                    }
                    KeyCode::Right => {
                        if self.product_grid_selected + 1 < product_count {
                            self.product_grid_selected += 1;
                        }
                    }
                    KeyCode::Up => {
                        if self.product_grid_selected >= cols {
                            self.product_grid_selected -= cols;
                        }
                    }
                    KeyCode::Down => {
                        if self.product_grid_selected + cols < product_count {
                            self.product_grid_selected += cols;
                        }
                    }
                    KeyCode::Char('n') => {
                        let _ = command_tx
                            .send(AppCommand::Storage(StorageCommand::CreateProductAndSession));
                    }
                    KeyCode::Char('d') | KeyCode::Delete | KeyCode::Backspace => {
                        if let Some(product) = self.picker.products.get(self.product_grid_selected)
                        {
                            let active_block = self.active_session.as_ref().is_some_and(|s| {
                                s.product_id == product.product_id && s.committed_at.is_none()
                            });
                            if active_block {
                                self.toast(
                                    "Finish or abandon the active session before deleting."
                                        .to_string(),
                                    Severity::Warning,
                                );
                                return;
                            }
                            self.delete_confirm = Some(DeleteConfirm {
                                product_id: product.product_id.clone(),
                                sku_alias: product.sku_alias.clone(),
                                expires_at: Instant::now() + Duration::from_secs(6),
                            });
                            self.toast(
                                format!(
                                    "Delete {}? Press y to confirm, n to cancel.",
                                    product.sku_alias
                                ),
                                Severity::Warning,
                            );
                        } else {
                            self.toast("No products available.".to_string(), Severity::Warning);
                        }
                    }
                    KeyCode::Enter => {
                        if let Some(product) = self.picker.products.get(self.product_grid_selected)
                        {
                            let _ = command_tx.send(AppCommand::Storage(
                                StorageCommand::StartSessionForProduct {
                                    product_id: product.product_id.clone(),
                                },
                            ));
                        } else {
                            self.toast("No products available.".to_string(), Severity::Warning);
                        }
                    }
                    _ => {}
                }
            }
            ProductsMode::Workspace => {
                match key.code {
                    KeyCode::Tab => {
                        self.products_subtab = match self.products_subtab {
                            ProductsSubTab::Context => ProductsSubTab::Structure,
                            ProductsSubTab::Structure => ProductsSubTab::Listings,
                            ProductsSubTab::Listings => ProductsSubTab::Context,
                        };
                        self.queue_image_preview();
                    }
                    KeyCode::BackTab => {
                        self.products_subtab = match self.products_subtab {
                            ProductsSubTab::Context => ProductsSubTab::Listings,
                            ProductsSubTab::Structure => ProductsSubTab::Context,
                            ProductsSubTab::Listings => ProductsSubTab::Structure,
                        };
                        self.queue_image_preview();
                    }
                    KeyCode::Left => {
                        if self.products_subtab == ProductsSubTab::Context && !self.text_editing {
                            self.context_focus = ContextFocus::Images;
                            self.queue_image_preview();
                        }
                    }
                    KeyCode::Right => {
                        if self.products_subtab == ProductsSubTab::Context && !self.text_editing {
                            self.context_focus = ContextFocus::Text;
                            self.queue_image_preview();
                        }
                    }
                    KeyCode::Char('g') => {
                        self.products_mode = ProductsMode::Grid;
                        let _ = command_tx.send(AppCommand::Storage(StorageCommand::ListProducts));
                    }
                    _ => {}
                }

                if self.products_subtab == ProductsSubTab::Context {
                    if key.code == KeyCode::Enter && self.context_focus == ContextFocus::Text {
                        self.text_editing = true;
                        self.toast("Editing text (Esc to save).".to_string(), Severity::Info);
                        return;
                    }
                    if key.code == KeyCode::Char('e') && self.context_focus == ContextFocus::Text {
                        self.text_editing = true;
                        self.toast("Editing text (Esc to save).".to_string(), Severity::Info);
                        return;
                    }
                }

                match self.products_subtab {
                    ProductsSubTab::Context => self.handle_capture_keys(key, command_tx),
                    ProductsSubTab::Structure => self.handle_curate_keys(key, command_tx),
                    ProductsSubTab::Listings => self.handle_upload_keys(key, command_tx),
                }
            }
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
                if let Some(active) = &self.active_product {
                    if let Some(idx) = self
                        .picker
                        .products
                        .iter()
                        .position(|p| p.product_id == active.product_id)
                    {
                        self.product_grid_selected = idx;
                    } else {
                        self.product_grid_selected = 0;
                    }
                } else {
                    self.product_grid_selected = 0;
                }
            }
            StorageEvent::ProductSelected(product) => {
                self.active_product = Some(product);
                self.context_text = self
                    .active_product
                    .as_ref()
                    .and_then(|p| p.context_text.clone())
                    .unwrap_or_default();
                self.text_editing = false;
                self.context_focus = ContextFocus::Images;
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
                self.preview_enabled = true;
                self.pending_commands.push(AppCommand::Preview(
                    crate::types::PreviewCommand::SetEnabled(true),
                ));
                self.active_session = Some(session);
                self.session_frame_selected = 0;
                self.context_focus = ContextFocus::Images;
                self.queue_image_preview();
                self.active_tab = AppTab::Products;
                self.products_mode = ProductsMode::Workspace;
                self.products_subtab = ProductsSubTab::Context;
            }
            StorageEvent::SessionUpdated(session) => {
                let frame_len = session.frames.len();
                self.active_session = Some(session);
                if frame_len == 0 {
                    self.session_frame_selected = 0;
                } else {
                    self.session_frame_selected =
                        self.session_frame_selected.min(frame_len.saturating_sub(1));
                }
                self.queue_image_preview();
            }
            StorageEvent::CommitCompleted {
                product,
                session,
                committed_count,
            } => {
                self.active_product = Some(product.clone());
                self.active_session = Some(session);
                let mut commit_message = format!(
                    "Committed {} image(s) to {}",
                    committed_count, product.sku_alias
                );
                if committed_count > 0 {
                    self.products_subtab = ProductsSubTab::Listings;
                    if self.config.online_ready {
                        self.pending_commands.push(AppCommand::Upload(
                            UploadCommand::UploadProduct {
                                product_id: product.product_id.clone(),
                            },
                        ));
                        commit_message.push_str(" (upload queued)");
                    } else {
                        commit_message.push_str(" (upload ready via 'u')");
                    }
                }
                self.last_commit_message = Some(commit_message);
                self.toast(
                    self.last_commit_message.clone().unwrap_or_default(),
                    Severity::Success,
                );
                self.pending_commands
                    .push(AppCommand::Capture(CaptureCommand::ClearOutputDir));
            }
            StorageEvent::ProductDeleted {
                product_id,
                removed_sessions: _,
            } => {
                if self
                    .active_product
                    .as_ref()
                    .is_some_and(|p| p.product_id == product_id)
                {
                    self.active_product = None;
                }
                if self
                    .active_session
                    .as_ref()
                    .is_some_and(|s| s.product_id == product_id)
                {
                    self.active_session = None;
                    self.pending_commands
                        .push(AppCommand::Capture(CaptureCommand::ClearOutputDir));
                }
                self.products_mode = ProductsMode::Grid;
                self.products_subtab = ProductsSubTab::Context;
                self.context_text.clear();
                self.text_editing = false;
                self.context_focus = ContextFocus::Images;
                self.queue_image_preview();
                self.toast("Product deleted.".to_string(), Severity::Success);
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
                self.products_mode = ProductsMode::Grid;
                self.pending_commands
                    .push(AppCommand::Capture(CaptureCommand::ClearOutputDir));
                self.queue_image_preview();
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

    fn apply_upload_job(&mut self, job: UploadJob) {
        if let Some(existing) = self.uploads.iter_mut().find(|j| j.id == job.id) {
            *existing = job.clone();
        } else {
            self.uploads.push(job.clone());
        }
        if job.status == JobStatus::Completed {
            self.toast("Upload completed.".to_string(), Severity::Success);
        }
        if job.status == JobStatus::Failed {
            if let Some(err) = &job.last_error {
                self.last_error = Some(err.clone());
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
            AppTab::Home => AppTab::Products,
            AppTab::Products => AppTab::Activity,
            AppTab::Activity => AppTab::Settings,
            AppTab::Settings => AppTab::Home,
        };
    }

    fn prev_tab(&mut self) {
        self.active_tab = match self.active_tab {
            AppTab::Home => AppTab::Settings,
            AppTab::Products => AppTab::Home,
            AppTab::Activity => AppTab::Products,
            AppTab::Settings => AppTab::Activity,
        };
    }
}
