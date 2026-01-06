use std::collections::HashSet;
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
use serde_json::{Number, Value};
use talaria_core::config::EbaySettings;
use talaria_core::models::MarketplaceId;

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
    pub ebay_settings: EbaySettings,

    pub uploads: Vec<UploadJob>,
    pub product_grid_selected: usize,
    pub product_grid_cols: usize,
    pub products_mode: ProductsMode,
    pub products_subtab: ProductsSubTab,
    pub context_focus: ContextFocus,

    pub session_frame_selected: usize,
    pub context_text: String,
    pub text_editing: bool,
    pub structure_text: String,
    pub structure_editing: bool,
    pub structure_field_selected: usize,
    pub structure_field_editing: bool,
    pub structure_field_edit_buffer: String,
    pub structure_field_edit_path: Option<String>,
    pub structure_field_edit_kind: StructureEditKind,
    pub structure_list_offset: usize,
    pub listings_selected: usize,
    pub listings_field_selected: usize,
    pub listings_field_editing: bool,
    pub listings_field_edit_buffer: String,
    pub listings_field_edit_key: Option<ListingFieldKey>,
    pub listings_field_edit_kind: ListingEditKind,
    pub listings_field_list_offset: usize,
    pub listings_editing: bool,
    pub listings_edit_buffer: String,
    pub settings_selected: usize,
    pub settings_editing: bool,
    pub settings_edit_buffer: String,
    pub pending_commands: Vec<AppCommand>,
}

impl AppState {
    pub fn new(
        captures_dir: PathBuf,
        stderr_log_path: Option<PathBuf>,
        config: ConfigInfo,
        ebay_settings: EbaySettings,
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
            ebay_settings,
            uploads: Vec::new(),
            product_grid_selected: 0,
            product_grid_cols: 3,
            products_mode: ProductsMode::Grid,
            products_subtab: ProductsSubTab::Context,
            context_focus: ContextFocus::Images,
            session_frame_selected: 0,
            context_text: String::new(),
            text_editing: false,
            structure_text: String::new(),
            structure_editing: false,
            structure_field_selected: 0,
            structure_field_editing: false,
            structure_field_edit_buffer: String::new(),
            structure_field_edit_path: None,
            structure_field_edit_kind: StructureEditKind::Text,
            structure_list_offset: 0,
            listings_selected: 0,
            listings_field_selected: 0,
            listings_field_editing: false,
            listings_field_edit_buffer: String::new(),
            listings_field_edit_key: None,
            listings_field_edit_kind: ListingEditKind::Text,
            listings_field_list_offset: 0,
            listings_editing: false,
            listings_edit_buffer: String::new(),
            settings_selected: 0,
            settings_editing: false,
            settings_edit_buffer: String::new(),
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
        if self.structure_editing
            && self.active_tab == AppTab::Products
            && self.products_mode == ProductsMode::Workspace
            && self.products_subtab == ProductsSubTab::Structure
        {
            if self.handle_structure_edit_keys(key, command_tx) {
                return;
            }
        }
        if self.structure_field_editing
            && self.active_tab == AppTab::Products
            && self.products_mode == ProductsMode::Workspace
            && self.products_subtab == ProductsSubTab::Structure
        {
            if self.handle_structure_field_edit_keys(key, command_tx) {
                return;
            }
        }
        if self.listings_field_editing
            && self.active_tab == AppTab::Products
            && self.products_mode == ProductsMode::Workspace
            && self.products_subtab == ProductsSubTab::Listings
        {
            if self.handle_listings_field_edit_keys(key, command_tx) {
                return;
            }
        }
        if self.listings_editing
            && self.active_tab == AppTab::Products
            && self.products_mode == ProductsMode::Workspace
            && self.products_subtab == ProductsSubTab::Listings
        {
            if self.handle_listings_edit_keys(key, command_tx) {
                return;
            }
        }
        if self.settings_editing && self.active_tab == AppTab::Settings {
            if self.handle_settings_edit_keys(key) {
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
            AppTab::Settings => self.handle_settings_keys(key),
            _ => {}
        }

        let prev_tab = self.active_tab;
        match key.code {
            KeyCode::Left if self.active_tab != AppTab::Products => self.prev_tab(),
            KeyCode::Right if self.active_tab != AppTab::Products => self.next_tab(),
            KeyCode::Char('h') => self.prev_tab(),
            KeyCode::Char('l') => self.next_tab(),
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

    fn save_structure_text(&mut self, command_tx: &Sender<AppCommand>) -> bool {
        let Some(product) = &self.active_product else {
            return false;
        };
        let parsed = match serde_json::from_str::<serde_json::Value>(&self.structure_text) {
            Ok(value) => value,
            Err(err) => {
                self.toast(format!("Invalid JSON: {err}"), Severity::Error);
                return false;
            }
        };
        let _ = command_tx.send(AppCommand::Storage(
            StorageCommand::SetProductStructureJson {
                product_id: product.product_id.clone(),
                structure_json: parsed,
            },
        ));
        true
    }

    fn save_listings_text(&mut self, command_tx: &Sender<AppCommand>) -> bool {
        let Some(product) = &self.active_product else {
            return false;
        };
        let Some(key) = self.selected_listing_key() else {
            self.toast("No listing selected.".to_string(), Severity::Warning);
            return false;
        };
        let parsed =
            match serde_json::from_str::<storage::MarketplaceListing>(&self.listings_edit_buffer) {
                Ok(value) => value,
                Err(err) => {
                    self.toast(format!("Invalid listing JSON: {err}"), Severity::Error);
                    return false;
                }
            };
        let mut listings = product.listings.clone();
        listings.insert(key, parsed);
        let _ = command_tx.send(AppCommand::Storage(StorageCommand::SetProductListings {
            product_id: product.product_id.clone(),
            listings,
        }));
        true
    }

    fn start_structure_editing(&mut self) {
        if self.structure_editing {
            return;
        }
        self.structure_field_editing = false;
        self.structure_field_edit_path = None;
        self.structure_field_edit_buffer.clear();
        if self.structure_text.trim().is_empty() {
            if let Some(product) = &self.active_product {
                if let Some(json) = &product.structure_json {
                    self.structure_text = serde_json::to_string_pretty(json).unwrap_or_default();
                }
            }
        }
        if self.structure_text.trim().is_empty() {
            self.structure_text = "{\n}".to_string();
        }
        self.structure_editing = true;
        self.toast(
            "Editing structure (Esc to save).".to_string(),
            Severity::Info,
        );
    }

    fn start_structure_field_editing(&mut self) {
        if self.structure_field_editing {
            return;
        }
        let Some(product) = &self.active_product else {
            self.toast("No active product selected.".to_string(), Severity::Warning);
            return;
        };
        let entries = self.structure_entries();
        if entries.is_empty() {
            self.toast(
                "No structure fields available.".to_string(),
                Severity::Warning,
            );
            return;
        }
        if self.structure_field_selected >= entries.len() {
            self.structure_field_selected = entries.len().saturating_sub(1);
        }
        let entry = &entries[self.structure_field_selected];
        if product.structure_json.is_none() && self.structure_text.trim().is_empty() {
            self.structure_text = "{}".to_string();
        }
        self.structure_editing = false;
        self.structure_field_editing = true;
        self.structure_field_edit_path = Some(entry.path.clone());
        self.structure_field_edit_kind = edit_kind_for_value(&entry.value);
        self.structure_field_edit_buffer = edit_buffer_for_value(&entry.value);
        self.toast(
            format!("Editing {} (Esc to save).", entry.path),
            Severity::Info,
        );
    }

    fn start_listings_editing(&mut self) {
        if self.listings_editing {
            return;
        }
        self.listings_field_editing = false;
        self.listings_field_edit_buffer.clear();
        self.listings_field_edit_key = None;
        let key = self.selected_listing_key().unwrap_or_else(|| {
            let fallback = marketplace_key_from_settings(&self.ebay_settings);
            self.listings_selected = 0;
            fallback
        });
        let mut payload = serde_json::json!({});
        if let Some(product) = &self.active_product {
            if let Some(listing) = product.listings.get(&key) {
                if let Ok(value) = serde_json::to_value(listing) {
                    payload = value;
                }
            }
        }
        self.listings_edit_buffer = serde_json::to_string_pretty(&payload).unwrap_or_default();
        self.listings_editing = true;
        self.toast("Editing listing (Esc to save).".to_string(), Severity::Info);
    }

    fn start_listings_field_editing(&mut self) {
        if self.listings_field_editing {
            return;
        }
        let entries = self.listing_field_entries();
        if entries.is_empty() {
            self.toast(
                "No listing fields available.".to_string(),
                Severity::Warning,
            );
            return;
        }
        if self.listings_field_selected >= entries.len() {
            self.listings_field_selected = entries.len().saturating_sub(1);
        }
        let entry = &entries[self.listings_field_selected];
        self.listings_editing = false;
        self.listings_field_editing = true;
        self.listings_field_edit_key = Some(entry.key);
        self.listings_field_edit_kind = entry.kind;
        self.listings_field_edit_buffer = listing_edit_buffer_for_value(&entry.value);
        self.toast(
            format!("Editing {} (Esc to save).", entry.label),
            Severity::Info,
        );
    }

    pub fn listing_keys(&self) -> Vec<String> {
        let mut keys = self
            .active_product
            .as_ref()
            .map(|p| p.listings.keys().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        if keys.is_empty() {
            keys.push(marketplace_key_from_settings(&self.ebay_settings));
        }
        keys.sort();
        keys
    }

    fn selected_listing_key(&self) -> Option<String> {
        let keys = self.listing_keys();
        let idx = self.listings_selected.min(keys.len().saturating_sub(1));
        keys.get(idx).cloned()
    }

    fn selected_listing_condition_override(&self) -> (Option<String>, Option<i32>) {
        let Some(product) = &self.active_product else {
            return (None, None);
        };
        let Some(key) = self.selected_listing_key() else {
            return (None, None);
        };
        let Some(listing) = product.listings.get(&key) else {
            return (None, None);
        };
        (listing.condition.clone(), listing.condition_id)
    }

    fn generate_listing(&mut self, command_tx: &Sender<AppCommand>, dry_run: bool, publish: bool) {
        let Some(product) = &self.active_product else {
            self.toast("No active product selected.".to_string(), Severity::Warning);
            return;
        };
        let marketplace =
            selected_marketplace(self.selected_listing_key().as_deref(), &self.ebay_settings);
        let (condition, condition_id) = self.selected_listing_condition_override();
        let _ = command_tx.send(AppCommand::Storage(
            StorageCommand::GenerateProductListing {
                product_id: product.product_id.clone(),
                sku_alias: product.sku_alias.clone(),
                marketplace,
                settings: self.ebay_settings.clone(),
                condition,
                condition_id,
                dry_run,
                publish,
            },
        ));
        self.toast("Listing request queued.".to_string(), Severity::Info);
    }

    fn save_settings_buffer(&mut self) -> bool {
        let value = self.settings_edit_buffer.trim().to_string();
        let fields = settings_fields();
        if self.settings_selected >= fields.len() {
            self.settings_selected = fields.len().saturating_sub(1);
        }
        match fields[self.settings_selected] {
            SettingsField::Marketplace => {
                self.ebay_settings.marketplace = non_empty(value);
            }
            SettingsField::MerchantLocation => {
                self.ebay_settings.merchant_location_key = non_empty(value);
            }
            SettingsField::FulfillmentPolicy => {
                self.ebay_settings.fulfillment_policy_id = non_empty(value);
            }
            SettingsField::PaymentPolicy => {
                self.ebay_settings.payment_policy_id = non_empty(value);
            }
            SettingsField::ReturnPolicy => {
                self.ebay_settings.return_policy_id = non_empty(value);
            }
        }
        let mut cfg = match talaria_core::config::Config::load() {
            Ok(cfg) => cfg,
            Err(err) => {
                self.toast(format!("Config load failed: {err}"), Severity::Error);
                return false;
            }
        };
        cfg.ebay = self.ebay_settings.clone();
        if let Err(err) = cfg.save() {
            self.toast(format!("Config save failed: {err}"), Severity::Error);
            return false;
        }
        true
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

    fn handle_structure_edit_keys(
        &mut self,
        key: KeyEvent,
        command_tx: &Sender<AppCommand>,
    ) -> bool {
        if !self.structure_editing {
            return false;
        }
        match key.code {
            KeyCode::Esc => {
                if self.save_structure_text(command_tx) {
                    self.structure_editing = false;
                    self.toast("Structure saved.".to_string(), Severity::Success);
                }
                true
            }
            KeyCode::Enter => {
                self.structure_text.push('\n');
                true
            }
            KeyCode::Backspace => {
                self.structure_text.pop();
                true
            }
            KeyCode::Char(c) => {
                self.structure_text.push(c);
                true
            }
            KeyCode::Delete | KeyCode::Tab | KeyCode::BackTab => true,
            _ => true,
        }
    }

    fn handle_structure_field_edit_keys(
        &mut self,
        key: KeyEvent,
        command_tx: &Sender<AppCommand>,
    ) -> bool {
        if !self.structure_field_editing {
            return false;
        }
        match key.code {
            KeyCode::Esc => {
                if self.save_structure_field_edit(command_tx) {
                    self.structure_field_editing = false;
                    self.structure_field_edit_buffer.clear();
                    self.structure_field_edit_path = None;
                    self.toast("Structure field saved.".to_string(), Severity::Success);
                }
                true
            }
            KeyCode::Enter => {
                self.structure_field_edit_buffer.push('\n');
                true
            }
            KeyCode::Backspace => {
                self.structure_field_edit_buffer.pop();
                true
            }
            KeyCode::Char(c) => {
                self.structure_field_edit_buffer.push(c);
                true
            }
            KeyCode::Delete | KeyCode::Tab | KeyCode::BackTab => true,
            _ => true,
        }
    }

    fn handle_listings_field_edit_keys(
        &mut self,
        key: KeyEvent,
        command_tx: &Sender<AppCommand>,
    ) -> bool {
        if !self.listings_field_editing {
            return false;
        }
        match key.code {
            KeyCode::Esc => {
                if self.save_listings_field_edit(command_tx) {
                    self.listings_field_editing = false;
                    self.listings_field_edit_buffer.clear();
                    self.listings_field_edit_key = None;
                    self.toast("Listing field saved.".to_string(), Severity::Success);
                }
                true
            }
            KeyCode::Enter => {
                self.listings_field_edit_buffer.push('\n');
                true
            }
            KeyCode::Backspace => {
                self.listings_field_edit_buffer.pop();
                true
            }
            KeyCode::Char(c) => {
                self.listings_field_edit_buffer.push(c);
                true
            }
            KeyCode::Delete | KeyCode::Tab | KeyCode::BackTab => true,
            _ => true,
        }
    }

    fn handle_listings_edit_keys(
        &mut self,
        key: KeyEvent,
        command_tx: &Sender<AppCommand>,
    ) -> bool {
        if !self.listings_editing {
            return false;
        }
        match key.code {
            KeyCode::Esc => {
                if self.save_listings_text(command_tx) {
                    self.listings_editing = false;
                    self.toast("Listing saved.".to_string(), Severity::Success);
                }
                true
            }
            KeyCode::Enter => {
                self.listings_edit_buffer.push('\n');
                true
            }
            KeyCode::Backspace => {
                self.listings_edit_buffer.pop();
                true
            }
            KeyCode::Char(c) => {
                self.listings_edit_buffer.push(c);
                true
            }
            KeyCode::Delete | KeyCode::Tab | KeyCode::BackTab => true,
            _ => true,
        }
    }

    fn handle_settings_edit_keys(&mut self, key: KeyEvent) -> bool {
        if !self.settings_editing {
            return false;
        }
        match key.code {
            KeyCode::Esc => {
                self.settings_editing = false;
                self.settings_edit_buffer.clear();
                self.toast("Edit canceled.".to_string(), Severity::Info);
                true
            }
            KeyCode::Enter => {
                if self.save_settings_buffer() {
                    self.settings_editing = false;
                    self.settings_edit_buffer.clear();
                    self.toast("Settings saved.".to_string(), Severity::Success);
                }
                true
            }
            KeyCode::Backspace => {
                self.settings_edit_buffer.pop();
                true
            }
            KeyCode::Char(c) => {
                self.settings_edit_buffer.push(c);
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
        if self.context_images_from_session() {
            let session = self.active_session.as_ref()?;
            let frame = session.frames.get(self.session_frame_selected)?;
            Some(
                storage::session_dir(&self.captures_dir, &session.session_id).join(&frame.rel_path),
            )
        } else {
            let product = self.active_product.as_ref()?;
            let image = product.images.get(self.session_frame_selected)?;
            Some(
                storage::product_dir(&self.captures_dir, &product.product_id).join(&image.rel_path),
            )
        }
    }

    pub fn apply_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Capture(event) => self.apply_capture_event(event),
            AppEvent::Preview(event) => self.apply_preview_event(event),
            AppEvent::Storage(event) => self.apply_storage_event(event),
            AppEvent::UploadJob(job) => self.apply_upload_job(job),
            AppEvent::UploadFinished { product_id } => {
                self.pending_commands
                    .push(AppCommand::Storage(StorageCommand::SyncProductMedia {
                        product_id,
                    }));
            }
            AppEvent::Activity(entry) => self.activity.push(entry),
        }
    }

    fn apply_preview_event(&mut self, event: PreviewEvent) {
        match event {
            PreviewEvent::Unavailable(message) | PreviewEvent::Error(message) => {
                self.preview_enabled = false;
                self.toast(message, Severity::Warning);
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
                    let count = self.context_image_count();
                    if self.session_frame_selected + 1 < count {
                        self.session_frame_selected += 1;
                        self.queue_image_preview();
                    }
                }
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                if self.context_focus != ContextFocus::Images {
                    return;
                }
                if !self.context_images_from_session() {
                    return;
                }
                if let Some(session) = &self.active_session {
                    if let Some(frame) = session.frames.get(self.session_frame_selected) {
                        let _ = command_tx.send(AppCommand::Storage(
                            StorageCommand::ToggleSessionFrameSelection {
                                session_id: session.session_id.clone(),
                                frame_rel_path: frame.rel_path.clone(),
                            },
                        ));
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
                if !self.context_images_from_session() {
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

    fn handle_structure_keys(&mut self, key: KeyEvent, command_tx: &Sender<AppCommand>) {
        match key.code {
            KeyCode::Up => {
                if self.structure_field_selected > 0 {
                    self.structure_field_selected -= 1;
                }
            }
            KeyCode::Down => {
                let entries = self.structure_entries();
                if self.structure_field_selected + 1 < entries.len() {
                    self.structure_field_selected += 1;
                }
            }
            KeyCode::Enter | KeyCode::Char('e') => {
                self.start_structure_field_editing();
            }
            KeyCode::Char('E') => {
                self.start_structure_editing();
            }
            KeyCode::Char('r') => {
                let Some(product) = &self.active_product else {
                    self.toast("No active product selected.".to_string(), Severity::Warning);
                    return;
                };
                let _ = command_tx.send(AppCommand::Storage(
                    StorageCommand::GenerateProductStructure {
                        product_id: product.product_id.clone(),
                        sku_alias: product.sku_alias.clone(),
                    },
                ));
                self.toast("Generating structure...".to_string(), Severity::Info);
            }
            _ => {}
        }
    }

    pub(crate) fn context_images_from_session(&self) -> bool {
        self.active_session
            .as_ref()
            .is_some_and(|session| !session.frames.is_empty())
    }

    pub(crate) fn context_image_count(&self) -> usize {
        if self.context_images_from_session() {
            self.active_session
                .as_ref()
                .map(|session| session.frames.len())
                .unwrap_or(0)
        } else {
            self.active_product
                .as_ref()
                .map(|product| product.images.len())
                .unwrap_or(0)
        }
    }

    fn handle_listings_keys(&mut self, key: KeyEvent, command_tx: &Sender<AppCommand>) {
        match key.code {
            KeyCode::Up => {
                if self.listings_field_selected > 0 {
                    self.listings_field_selected -= 1;
                }
            }
            KeyCode::Down => {
                let entries = self.listing_field_entries();
                if self.listings_field_selected + 1 < entries.len() {
                    self.listings_field_selected += 1;
                }
            }
            KeyCode::Enter | KeyCode::Char('e') => {
                self.start_listings_field_editing();
            }
            KeyCode::Char('E') => {
                self.start_listings_editing();
            }
            KeyCode::Char('r') => {
                self.generate_listing(command_tx, true, false);
            }
            KeyCode::Char('p') => {
                self.generate_listing(command_tx, false, false);
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
                        if self.products_subtab == ProductsSubTab::Listings
                            && !self.listings_editing
                            && !self.listings_field_editing
                        {
                            if self.listings_selected > 0 {
                                self.listings_selected -= 1;
                                self.listings_field_list_offset = 0;
                            }
                        }
                    }
                    KeyCode::Right => {
                        if self.products_subtab == ProductsSubTab::Context && !self.text_editing {
                            self.context_focus = ContextFocus::Text;
                            self.queue_image_preview();
                        }
                        if self.products_subtab == ProductsSubTab::Listings
                            && !self.listings_editing
                            && !self.listings_field_editing
                        {
                            let keys = self.listing_keys();
                            if self.listings_selected + 1 < keys.len() {
                                self.listings_selected += 1;
                                self.listings_field_list_offset = 0;
                            }
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
                    ProductsSubTab::Structure => self.handle_structure_keys(key, command_tx),
                    ProductsSubTab::Listings => self.handle_listings_keys(key, command_tx),
                }
            }
        }
    }

    fn handle_settings_keys(&mut self, key: KeyEvent) {
        if self.settings_editing {
            return;
        }
        match key.code {
            KeyCode::Up => {
                if self.settings_selected > 0 {
                    self.settings_selected -= 1;
                }
            }
            KeyCode::Down => {
                if self.settings_selected + 1 < settings_fields().len() {
                    self.settings_selected += 1;
                }
            }
            KeyCode::Enter | KeyCode::Char('e') | KeyCode::Char('E') => {
                self.settings_editing = true;
                self.settings_edit_buffer = match settings_fields()[self.settings_selected] {
                    SettingsField::Marketplace => {
                        self.ebay_settings.marketplace.clone().unwrap_or_default()
                    }
                    SettingsField::MerchantLocation => self
                        .ebay_settings
                        .merchant_location_key
                        .clone()
                        .unwrap_or_default(),
                    SettingsField::FulfillmentPolicy => self
                        .ebay_settings
                        .fulfillment_policy_id
                        .clone()
                        .unwrap_or_default(),
                    SettingsField::PaymentPolicy => self
                        .ebay_settings
                        .payment_policy_id
                        .clone()
                        .unwrap_or_default(),
                    SettingsField::ReturnPolicy => self
                        .ebay_settings
                        .return_policy_id
                        .clone()
                        .unwrap_or_default(),
                };
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
                self.structure_text = self
                    .active_product
                    .as_ref()
                    .and_then(|p| p.structure_json.clone())
                    .and_then(|v| serde_json::to_string_pretty(&v).ok())
                    .unwrap_or_default();
                self.text_editing = false;
                self.structure_editing = false;
                self.structure_field_editing = false;
                self.structure_field_edit_buffer.clear();
                self.structure_field_edit_path = None;
                self.structure_field_selected = 0;
                self.structure_list_offset = 0;
                self.listings_field_selected = 0;
                self.listings_field_editing = false;
                self.listings_field_edit_buffer.clear();
                self.listings_field_edit_key = None;
                self.listings_field_edit_kind = ListingEditKind::Text;
                self.listings_field_list_offset = 0;
                self.listings_editing = false;
                self.listings_edit_buffer.clear();
                self.listings_selected = 0;
                self.context_focus = ContextFocus::Images;
                self.session_frame_selected = 0;
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
                removed_sessions,
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
                self.structure_text.clear();
                self.structure_editing = false;
                self.listings_editing = false;
                self.listings_edit_buffer.clear();
                self.listings_selected = 0;
                self.listings_field_selected = 0;
                self.listings_field_editing = false;
                self.listings_field_edit_buffer.clear();
                self.listings_field_edit_key = None;
                self.listings_field_edit_kind = ListingEditKind::Text;
                self.listings_field_list_offset = 0;
                self.context_focus = ContextFocus::Images;
                self.queue_image_preview();
                let mut message = "Product deleted.".to_string();
                if removed_sessions > 0 {
                    message.push_str(&format!(" ({removed_sessions} session(s) removed)"));
                }
                self.toast(message, Severity::Success);
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

    pub fn listing_field_entries(&self) -> Vec<ListingFieldEntry> {
        let Some(product) = &self.active_product else {
            return Vec::new();
        };
        let Some(key) = self.selected_listing_key() else {
            return Vec::new();
        };
        let listing = product.listings.get(&key).cloned().unwrap_or_default();
        LISTING_FIELDS
            .iter()
            .map(|(field, label, kind)| ListingFieldEntry {
                key: *field,
                label: *label,
                value: listing_field_value(&listing, *field),
                kind: *kind,
            })
            .collect()
    }

    pub fn structure_entries(&self) -> Vec<StructureFieldEntry> {
        let root = self
            .active_product
            .as_ref()
            .and_then(|p| p.structure_json.clone())
            .unwrap_or_else(|| serde_json::json!({}));

        let mut entries = Vec::new();
        let mut seen = HashSet::new();

        for path in STRUCTURE_CORE_FIELDS {
            let value = get_json_path(&root, path).unwrap_or(Value::Null);
            entries.push(StructureFieldEntry {
                path: path.to_string(),
                value,
            });
            seen.insert(path.to_string());
        }

        let mut extra = Vec::new();
        flatten_json(&root, "", &mut extra);
        extra.sort_by(|a, b| a.path.cmp(&b.path));
        for entry in extra {
            if !seen.contains(&entry.path) {
                entries.push(entry);
            }
        }
        entries
    }

    fn save_listings_field_edit(&mut self, command_tx: &Sender<AppCommand>) -> bool {
        let Some(product) = &self.active_product else {
            self.toast("No active product selected.".to_string(), Severity::Warning);
            return false;
        };
        let Some(key) = self.selected_listing_key() else {
            self.toast("No listing selected.".to_string(), Severity::Warning);
            return false;
        };
        let Some(field_key) = self.listings_field_edit_key else {
            self.toast("No listing field selected.".to_string(), Severity::Warning);
            return false;
        };
        let value = match parse_listing_edit_buffer(
            &self.listings_field_edit_buffer,
            self.listings_field_edit_kind,
        ) {
            Ok(value) => value,
            Err(err) => {
                self.toast(format!("Invalid value: {err}"), Severity::Error);
                return false;
            }
        };

        let mut listing = product.listings.get(&key).cloned().unwrap_or_default();
        if let Err(err) = apply_listing_field_value(&mut listing, field_key, &value) {
            self.toast(format!("Invalid value: {err}"), Severity::Error);
            return false;
        }

        let mut listings = product.listings.clone();
        listings.insert(key, listing);
        let _ = command_tx.send(AppCommand::Storage(StorageCommand::SetProductListings {
            product_id: product.product_id.clone(),
            listings,
        }));
        true
    }

    fn save_structure_field_edit(&mut self, command_tx: &Sender<AppCommand>) -> bool {
        let Some(product) = &self.active_product else {
            self.toast("No active product selected.".to_string(), Severity::Warning);
            return false;
        };
        let Some(path) = self.structure_field_edit_path.clone() else {
            self.toast(
                "No structure field selected.".to_string(),
                Severity::Warning,
            );
            return false;
        };

        let value = match parse_edit_buffer(
            &self.structure_field_edit_buffer,
            self.structure_field_edit_kind,
        ) {
            Ok(value) => value,
            Err(err) => {
                self.toast(format!("Invalid value: {err}"), Severity::Error);
                return false;
            }
        };

        let mut root = product
            .structure_json
            .clone()
            .unwrap_or_else(|| serde_json::json!({}));
        set_json_path(&mut root, &path, value);

        let _ = command_tx.send(AppCommand::Storage(
            StorageCommand::SetProductStructureJson {
                product_id: product.product_id.clone(),
                structure_json: root,
            },
        ));
        true
    }
}

#[derive(Debug, Clone)]
pub struct StructureFieldEntry {
    pub path: String,
    pub value: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructureEditKind {
    Text,
    Number,
    Bool,
    Json,
}

const STRUCTURE_CORE_FIELDS: &[&str] = &[
    "name",
    "brand.name",
    "description",
    "color",
    "material",
    "mpn",
    "sku",
    "size",
    "offers.price",
    "offers.price_currency",
    "offers.price_specification.price",
    "offers.price_specification.price_currency",
    "weight.value",
    "weight.unit_text",
    "height.value",
    "height.unit_text",
    "width.value",
    "width.unit_text",
    "depth.value",
    "depth.unit_text",
    "image",
];

fn edit_kind_for_value(value: &Value) -> StructureEditKind {
    match value {
        Value::String(_) | Value::Null => StructureEditKind::Text,
        Value::Number(_) => StructureEditKind::Number,
        Value::Bool(_) => StructureEditKind::Bool,
        Value::Array(_) | Value::Object(_) => StructureEditKind::Json,
    }
}

fn edit_buffer_for_value(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Number(num) => num.to_string(),
        Value::Bool(val) => val.to_string(),
        Value::Null => String::new(),
        Value::Array(_) | Value::Object(_) => {
            serde_json::to_string_pretty(value).unwrap_or_default()
        }
    }
}

fn parse_edit_buffer(buffer: &str, kind: StructureEditKind) -> Result<Value, String> {
    let trimmed = buffer.trim();
    if trimmed.is_empty() {
        return Ok(Value::Null);
    }
    match kind {
        StructureEditKind::Text => Ok(Value::String(buffer.to_string())),
        StructureEditKind::Number => trimmed
            .parse::<f64>()
            .ok()
            .and_then(|v| serde_json::Number::from_f64(v))
            .map(Value::Number)
            .ok_or_else(|| "expected a number".to_string()),
        StructureEditKind::Bool => match trimmed.to_ascii_lowercase().as_str() {
            "true" => Ok(Value::Bool(true)),
            "false" => Ok(Value::Bool(false)),
            _ => Err("expected true/false".to_string()),
        },
        StructureEditKind::Json => {
            serde_json::from_str::<Value>(buffer).map_err(|err| err.to_string())
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListingFieldKey {
    Title,
    Price,
    Currency,
    CategoryLabel,
    CategoryId,
    Condition,
    ConditionId,
    Quantity,
    MerchantLocationKey,
    FulfillmentPolicyId,
    PaymentPolicyId,
    ReturnPolicyId,
    Status,
    ListingId,
}

#[derive(Debug, Clone)]
pub struct ListingFieldEntry {
    pub key: ListingFieldKey,
    pub label: &'static str,
    pub value: Value,
    pub kind: ListingEditKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListingEditKind {
    Text,
    Number,
    Integer,
}

const LISTING_FIELDS: &[(ListingFieldKey, &str, ListingEditKind)] = &[
    (ListingFieldKey::Title, "Title", ListingEditKind::Text),
    (ListingFieldKey::Price, "Price", ListingEditKind::Number),
    (ListingFieldKey::Currency, "Currency", ListingEditKind::Text),
    (
        ListingFieldKey::CategoryLabel,
        "Category Label",
        ListingEditKind::Text,
    ),
    (
        ListingFieldKey::CategoryId,
        "Category ID",
        ListingEditKind::Text,
    ),
    (
        ListingFieldKey::Condition,
        "Condition",
        ListingEditKind::Text,
    ),
    (
        ListingFieldKey::ConditionId,
        "Condition ID",
        ListingEditKind::Integer,
    ),
    (
        ListingFieldKey::Quantity,
        "Quantity",
        ListingEditKind::Integer,
    ),
    (
        ListingFieldKey::MerchantLocationKey,
        "Merchant Location Key",
        ListingEditKind::Text,
    ),
    (
        ListingFieldKey::FulfillmentPolicyId,
        "Fulfillment Policy ID",
        ListingEditKind::Text,
    ),
    (
        ListingFieldKey::PaymentPolicyId,
        "Payment Policy ID",
        ListingEditKind::Text,
    ),
    (
        ListingFieldKey::ReturnPolicyId,
        "Return Policy ID",
        ListingEditKind::Text,
    ),
    (ListingFieldKey::Status, "Status", ListingEditKind::Text),
    (
        ListingFieldKey::ListingId,
        "Listing ID",
        ListingEditKind::Text,
    ),
];

fn listing_field_value(listing: &storage::MarketplaceListing, key: ListingFieldKey) -> Value {
    match key {
        ListingFieldKey::Title => listing
            .title
            .clone()
            .map(Value::String)
            .unwrap_or(Value::Null),
        ListingFieldKey::Price => listing
            .price
            .and_then(|value| Number::from_f64(value))
            .map(Value::Number)
            .unwrap_or(Value::Null),
        ListingFieldKey::Currency => listing
            .currency
            .clone()
            .map(Value::String)
            .unwrap_or(Value::Null),
        ListingFieldKey::CategoryLabel => listing
            .category_label
            .clone()
            .map(Value::String)
            .unwrap_or(Value::Null),
        ListingFieldKey::CategoryId => listing
            .category_id
            .clone()
            .map(Value::String)
            .unwrap_or(Value::Null),
        ListingFieldKey::Condition => listing
            .condition
            .clone()
            .map(Value::String)
            .unwrap_or(Value::Null),
        ListingFieldKey::ConditionId => listing
            .condition_id
            .map(|value| Value::Number(Number::from(value)))
            .unwrap_or(Value::Null),
        ListingFieldKey::Quantity => listing
            .quantity
            .map(|value| Value::Number(Number::from(value)))
            .unwrap_or(Value::Null),
        ListingFieldKey::MerchantLocationKey => listing
            .merchant_location_key
            .clone()
            .map(Value::String)
            .unwrap_or(Value::Null),
        ListingFieldKey::FulfillmentPolicyId => listing
            .fulfillment_policy_id
            .clone()
            .map(Value::String)
            .unwrap_or(Value::Null),
        ListingFieldKey::PaymentPolicyId => listing
            .payment_policy_id
            .clone()
            .map(Value::String)
            .unwrap_or(Value::Null),
        ListingFieldKey::ReturnPolicyId => listing
            .return_policy_id
            .clone()
            .map(Value::String)
            .unwrap_or(Value::Null),
        ListingFieldKey::Status => listing
            .status
            .clone()
            .map(Value::String)
            .unwrap_or(Value::Null),
        ListingFieldKey::ListingId => listing
            .listing_id
            .clone()
            .map(Value::String)
            .unwrap_or(Value::Null),
    }
}

fn listing_edit_buffer_for_value(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(text) => text.clone(),
        Value::Number(num) => num.to_string(),
        Value::Bool(val) => val.to_string(),
        Value::Array(_) | Value::Object(_) => {
            serde_json::to_string_pretty(value).unwrap_or_default()
        }
    }
}

fn parse_listing_edit_buffer(buffer: &str, kind: ListingEditKind) -> Result<Value, String> {
    let trimmed = buffer.trim();
    if trimmed.is_empty() {
        return Ok(Value::Null);
    }
    match kind {
        ListingEditKind::Text => Ok(Value::String(buffer.to_string())),
        ListingEditKind::Number => trimmed
            .parse::<f64>()
            .ok()
            .and_then(Number::from_f64)
            .map(Value::Number)
            .ok_or_else(|| "expected a number".to_string()),
        ListingEditKind::Integer => trimmed
            .parse::<i32>()
            .ok()
            .map(|value| Value::Number(Number::from(value)))
            .ok_or_else(|| "expected an integer".to_string()),
    }
}

fn apply_listing_field_value(
    listing: &mut storage::MarketplaceListing,
    key: ListingFieldKey,
    value: &Value,
) -> Result<(), String> {
    let text_value = |value: &Value| match value {
        Value::Null => Ok(None),
        Value::String(text) => {
            if text.trim().is_empty() {
                Ok(None)
            } else {
                Ok(Some(text.clone()))
            }
        }
        _ => Err("expected text".to_string()),
    };
    let number_value = |value: &Value| match value {
        Value::Null => Ok(None),
        Value::Number(num) => num
            .as_f64()
            .ok_or_else(|| "expected a number".to_string())
            .map(Some),
        _ => Err("expected a number".to_string()),
    };
    let integer_value = |value: &Value| match value {
        Value::Null => Ok(None),
        Value::Number(num) => num
            .as_i64()
            .and_then(|val| i32::try_from(val).ok())
            .ok_or_else(|| "expected an integer".to_string())
            .map(Some),
        _ => Err("expected an integer".to_string()),
    };

    match key {
        ListingFieldKey::Title => listing.title = text_value(value)?,
        ListingFieldKey::Price => listing.price = number_value(value)?,
        ListingFieldKey::Currency => listing.currency = text_value(value)?,
        ListingFieldKey::CategoryLabel => listing.category_label = text_value(value)?,
        ListingFieldKey::CategoryId => listing.category_id = text_value(value)?,
        ListingFieldKey::Condition => listing.condition = text_value(value)?,
        ListingFieldKey::ConditionId => listing.condition_id = integer_value(value)?,
        ListingFieldKey::Quantity => listing.quantity = integer_value(value)?,
        ListingFieldKey::MerchantLocationKey => listing.merchant_location_key = text_value(value)?,
        ListingFieldKey::FulfillmentPolicyId => listing.fulfillment_policy_id = text_value(value)?,
        ListingFieldKey::PaymentPolicyId => listing.payment_policy_id = text_value(value)?,
        ListingFieldKey::ReturnPolicyId => listing.return_policy_id = text_value(value)?,
        ListingFieldKey::Status => listing.status = text_value(value)?,
        ListingFieldKey::ListingId => listing.listing_id = text_value(value)?,
    }
    Ok(())
}

fn get_json_path(root: &Value, path: &str) -> Option<Value> {
    if path.is_empty() {
        return Some(root.clone());
    }
    let mut current = root;
    for part in path.split('.') {
        let Value::Object(map) = current else {
            return None;
        };
        current = map.get(part)?;
    }
    Some(current.clone())
}

fn set_json_path(root: &mut Value, path: &str, value: Value) {
    if path.is_empty() {
        *root = value;
        return;
    }
    let mut current = root;
    let parts: Vec<&str> = path.split('.').collect();
    for (idx, part) in parts.iter().enumerate() {
        let last = idx == parts.len().saturating_sub(1);
        if last {
            if let Value::Object(map) = current {
                map.insert(part.to_string(), value);
            } else {
                *current = serde_json::json!({ part.to_string(): value });
            }
            return;
        }
        if !current.is_object() {
            *current = Value::Object(serde_json::Map::new());
        }
        if let Value::Object(map) = current {
            current = map
                .entry(part.to_string())
                .or_insert_with(|| Value::Object(serde_json::Map::new()));
        }
    }
}

fn flatten_json(root: &Value, prefix: &str, out: &mut Vec<StructureFieldEntry>) {
    match root {
        Value::Object(map) => {
            let mut keys = map.keys().cloned().collect::<Vec<_>>();
            keys.sort();
            for key in keys {
                let next_prefix = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                if let Some(value) = map.get(&key) {
                    match value {
                        Value::Object(_) => flatten_json(value, &next_prefix, out),
                        _ => out.push(StructureFieldEntry {
                            path: next_prefix,
                            value: value.clone(),
                        }),
                    }
                }
            }
        }
        _ => {
            if !prefix.is_empty() {
                out.push(StructureFieldEntry {
                    path: prefix.to_string(),
                    value: root.clone(),
                });
            }
        }
    }
}

#[derive(Clone, Copy)]
pub enum SettingsField {
    Marketplace,
    MerchantLocation,
    FulfillmentPolicy,
    PaymentPolicy,
    ReturnPolicy,
}

pub fn settings_fields() -> [SettingsField; 5] {
    [
        SettingsField::Marketplace,
        SettingsField::MerchantLocation,
        SettingsField::FulfillmentPolicy,
        SettingsField::PaymentPolicy,
        SettingsField::ReturnPolicy,
    ]
}

fn non_empty(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn marketplace_key_from_settings(settings: &EbaySettings) -> String {
    settings
        .marketplace
        .clone()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "EBAY_US".to_string())
        .to_uppercase()
}

fn selected_marketplace(selected_key: Option<&str>, settings: &EbaySettings) -> MarketplaceId {
    let key = selected_key
        .map(|k| k.to_string())
        .unwrap_or_else(|| marketplace_key_from_settings(settings));
    parse_marketplace(&key)
}

fn parse_marketplace(input: &str) -> MarketplaceId {
    match input.trim().to_uppercase().as_str() {
        "EBAY_UK" | "EBAY_GB" => MarketplaceId::EbayUk,
        "EBAY_DE" => MarketplaceId::EbayDe,
        _ => MarketplaceId::EbayUs,
    }
}
