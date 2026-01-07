use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::PreviewCommand;
use crate::storage;
use crate::types::{
    ActivityEntry, ActivityLog, AppCommand, AppEvent, CaptureCommand, CaptureEvent, CaptureStatus,
    JobStatus, PreviewEvent, Severity, StorageCommand, StorageEvent, UploadCommand, UploadJob,
};
use chrono::{DateTime, Local};
use crossbeam_channel::Sender;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_json::{Number, Value};
use talaria_core::config::EbaySettings;
use talaria_core::models::MarketplaceId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppTab {
    Home,
    Quickstart,
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
pub enum ContextImageEntry {
    Session {
        rel_path: String,
        sharpness_score: Option<f64>,
        created_at: DateTime<Local>,
        selected: bool,
    },
    Product {
        rel_path: String,
        created_at: DateTime<Local>,
        source: String,
        hero: bool,
    },
}

#[derive(Debug, Clone, Copy)]
pub struct ContextPipelineRequest {
    pub dry_run: bool,
    pub publish: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum PostSaveNotice {
    ContextUpdated,
    StructureUpdated,
    ListingsUpdated,
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
    pub spinner_started_at: Instant,

    pub captures_dir: PathBuf,
    pub stderr_log_path: Option<PathBuf>,

    pub camera_connected: bool,
    pub preview_enabled: bool,
    pub device_index: i32,
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
    pub listings_field_edit_name: Option<String>,
    pub listings_field_edit_image_index: Option<usize>,
    pub listings_field_edit_dimension: Option<PackageDimensionKey>,
    pub listings_field_edit_kind: ListingEditKind,
    pub listings_field_list_offset: usize,
    pub listings_editing: bool,
    pub listings_edit_buffer: String,
    pub settings_selected: usize,
    pub settings_editing: bool,
    pub settings_edit_buffer: String,
    pub pending_post_save_notice: Option<PostSaveNotice>,
    pub pending_context_pipeline: Option<ContextPipelineRequest>,
    pub products_loading: bool,
    pub product_syncing: bool,
    pub structure_inference: bool,
    pub listing_inference: bool,
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
            spinner_started_at: Instant::now(),
            captures_dir,
            stderr_log_path,
            camera_connected: false,
            preview_enabled: false,
            device_index: 0,
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
            listings_field_edit_name: None,
            listings_field_edit_image_index: None,
            listings_field_edit_dimension: None,
            listings_field_edit_kind: ListingEditKind::Text,
            listings_field_list_offset: 0,
            listings_editing: false,
            listings_edit_buffer: String::new(),
            settings_selected: 0,
            settings_editing: false,
            settings_edit_buffer: String::new(),
            pending_post_save_notice: None,
            pending_context_pipeline: None,
            products_loading: false,
            product_syncing: false,
            structure_inference: false,
            listing_inference: false,
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
        if key.code == KeyCode::BackTab {
            self.next_tab();
        }
        if self.active_tab != prev_tab && self.active_tab == AppTab::Products {
            self.products_mode = if self.active_product.is_some() {
                ProductsMode::Workspace
            } else {
                ProductsMode::Grid
            };
            self.products_loading = true;
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
        self.pending_post_save_notice = Some(PostSaveNotice::ContextUpdated);
    }

    fn handle_ctrl_save(&mut self, command_tx: &Sender<AppCommand>) {
        if self.active_tab != AppTab::Products {
            self.toast(
                "Save is available from Products.".to_string(),
                Severity::Warning,
            );
            return;
        }

        let mut ok = true;
        if self.text_editing {
            self.save_context_text(command_tx);
        }
        if self.structure_editing {
            ok &= self.save_structure_text(command_tx);
        }
        if self.structure_field_editing {
            ok &= self.save_structure_field_edit(command_tx);
        }
        if self.listings_editing {
            ok &= self.save_listings_text(command_tx);
        }
        if self.listings_field_editing {
            ok &= self.save_listings_field_edit(command_tx);
        }
        if !ok {
            return;
        }

        if let Some(session) = &self.active_session {
            if session.committed_at.is_none() && !session.frames.is_empty() {
                let _ = command_tx.send(AppCommand::Storage(StorageCommand::CommitSession {
                    session_id: session.session_id.clone(),
                }));
            }
        }

        let Some(product) = &self.active_product else {
            self.toast("No active product selected.".to_string(), Severity::Warning);
            return;
        };
        if self.config.online_ready && product.images.iter().any(|img| img.uploaded_url.is_none()) {
            let _ = command_tx.send(AppCommand::Upload(UploadCommand::UploadProduct {
                product_id: product.product_id.clone(),
            }));
        }
        let _ = command_tx.send(AppCommand::Storage(StorageCommand::SyncProductData {
            product_id: product.product_id.clone(),
        }));
        let _ = command_tx.send(AppCommand::Storage(StorageCommand::SyncProductMedia {
            product_id: product.product_id.clone(),
        }));
        self.product_syncing = true;
        self.toast("Saved + syncing...".to_string(), Severity::Info);
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
        self.pending_post_save_notice = Some(PostSaveNotice::StructureUpdated);
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
        self.pending_post_save_notice = Some(PostSaveNotice::ListingsUpdated);
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
        self.listings_field_edit_name = None;
        self.listings_field_edit_image_index = None;
        self.listings_field_edit_dimension = None;
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
        if entry.key == ListingFieldKey::Aspects {
            self.toast("Select an aspect to edit.".to_string(), Severity::Info);
            return;
        }
        if entry.key == ListingFieldKey::PackageDimensions {
            self.toast("Select a dimension to edit.".to_string(), Severity::Info);
            return;
        }
        self.listings_editing = false;
        self.listings_field_editing = true;
        self.listings_field_edit_key = Some(entry.key);
        self.listings_field_edit_name = entry.aspect_name.clone();
        self.listings_field_edit_image_index = entry.image_index;
        self.listings_field_edit_dimension = entry.dimension_key;
        self.listings_field_edit_kind = entry.kind;
        self.listings_field_edit_buffer = if entry.key == ListingFieldKey::AspectValue {
            format_aspect_values_edit_buffer(&entry.value)
        } else if entry.kind == ListingEditKind::Lines {
            format_lines_edit_buffer(&entry.value)
        } else {
            listing_edit_buffer_for_value(&entry.value)
        };
        self.toast(
            format!("Editing {} (Esc to save).", entry.label.as_str()),
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

    fn generate_listing(&mut self, dry_run: bool, publish: bool) {
        if let Some(cmd) = self.build_listing_command(dry_run, publish) {
            self.pending_commands.push(AppCommand::Storage(cmd));
            self.listing_inference = true;
            self.toast("Listing request queued.".to_string(), Severity::Info);
        }
    }

    fn build_listing_command(&mut self, dry_run: bool, publish: bool) -> Option<StorageCommand> {
        let Some(product) = &self.active_product else {
            self.toast("No active product selected.".to_string(), Severity::Warning);
            return None;
        };
        let marketplace =
            selected_marketplace(self.selected_listing_key().as_deref(), &self.ebay_settings);
        let (condition, condition_id) = self.selected_listing_condition_override();
        Some(StorageCommand::GenerateProductListing {
            product_id: product.product_id.clone(),
            sku_alias: product.sku_alias.clone(),
            marketplace,
            settings: self.ebay_settings.clone(),
            condition,
            condition_id,
            dry_run,
            publish,
        })
    }

    fn start_context_pipeline(
        &mut self,
        command_tx: &Sender<AppCommand>,
        dry_run: bool,
        publish: bool,
    ) {
        let Some(product) = &self.active_product else {
            self.toast("No active product selected.".to_string(), Severity::Warning);
            return;
        };
        self.pending_context_pipeline = Some(ContextPipelineRequest { dry_run, publish });
        let _ = command_tx.send(AppCommand::Storage(
            StorageCommand::GenerateProductStructure {
                product_id: product.product_id.clone(),
                sku_alias: product.sku_alias.clone(),
            },
        ));
        self.structure_inference = true;
        self.toast(
            "Generating structure (pipeline queued)...".to_string(),
            Severity::Info,
        );
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
                    self.listings_field_edit_name = None;
                    self.listings_field_edit_image_index = None;
                    self.listings_field_edit_dimension = None;
                    self.toast("Listing field saved.".to_string(), Severity::Success);
                    self.queue_image_preview();
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
        match self.products_subtab {
            ProductsSubTab::Context => {
                if self.context_focus != ContextFocus::Images {
                    return None;
                }
                let entry = self
                    .context_image_entries()
                    .get(self.session_frame_selected)
                    .cloned()?;
                match entry {
                    ContextImageEntry::Session { rel_path, .. } => {
                        let session = self.active_session.as_ref()?;
                        Some(
                            storage::session_dir(&self.captures_dir, &session.session_id)
                                .join(rel_path),
                        )
                    }
                    ContextImageEntry::Product { rel_path, .. } => {
                        let product = self.active_product.as_ref()?;
                        Some(
                            storage::product_dir(&self.captures_dir, &product.product_id)
                                .join(rel_path),
                        )
                    }
                }
            }
            ProductsSubTab::Listings => self.preview_listing_image_path(),
            _ => None,
        }
    }

    fn preview_listing_image_path(&self) -> Option<PathBuf> {
        let product = self.active_product.as_ref()?;
        let entries = self.listing_field_entries();
        let entry = entries.get(self.listings_field_selected)?;
        if entry.key != ListingFieldKey::ImageValue {
            return None;
        }
        let Value::String(url) = &entry.value else {
            return None;
        };
        let url_lower = url.to_ascii_lowercase();
        let filename = url
            .split('/')
            .last()
            .map(|value| value.split('?').next().unwrap_or(value))
            .unwrap_or("")
            .to_ascii_lowercase();
        for image in &product.images {
            let rel_lower = image.rel_path.to_ascii_lowercase();
            let uploaded_lower = image
                .uploaded_url
                .as_ref()
                .map(|value| value.to_ascii_lowercase());
            let rel_match = !filename.is_empty() && rel_lower.ends_with(&filename);
            let uploaded_match = uploaded_lower
                .as_ref()
                .is_some_and(|value| value == &url_lower || value.ends_with(&filename));
            if rel_match || uploaded_match {
                return Some(
                    storage::product_dir(&self.captures_dir, &product.product_id)
                        .join(&image.rel_path),
                );
            }
        }
        None
    }

    pub fn apply_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Capture(event) => self.apply_capture_event(event),
            AppEvent::Preview(event) => self.apply_preview_event(event),
            AppEvent::Storage(event) => self.apply_storage_event(event),
            AppEvent::UploadJob(job) => self.apply_upload_job(job),
            AppEvent::UploadFinished { product_id } => {
                self.product_syncing = true;
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
                let entry = self
                    .context_image_entries()
                    .get(self.session_frame_selected)
                    .cloned();
                if let Some(ContextImageEntry::Session { rel_path, .. }) = entry {
                    if let Some(session) = &self.active_session {
                        let _ = command_tx.send(AppCommand::Storage(
                            StorageCommand::ToggleSessionFrameSelection {
                                session_id: session.session_id.clone(),
                                frame_rel_path: rel_path,
                            },
                        ));
                    }
                }
            }
            KeyCode::Char('t') => {
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
            KeyCode::Char('r') => {
                self.start_context_pipeline(command_tx, true, false);
            }
            KeyCode::Char('p') => {
                self.start_context_pipeline(command_tx, false, false);
            }
            KeyCode::Char('P') => {
                self.start_context_pipeline(command_tx, false, true);
            }
            KeyCode::Backspace | KeyCode::Delete => {
                if self.context_focus != ContextFocus::Images {
                    return;
                }
                let entry = self
                    .context_image_entries()
                    .get(self.session_frame_selected)
                    .cloned();
                match entry {
                    Some(ContextImageEntry::Session { rel_path, .. }) => {
                        if let Some(session) = &self.active_session {
                            let _ = command_tx.send(AppCommand::Storage(
                                StorageCommand::DeleteSessionFrame {
                                    session_id: session.session_id.clone(),
                                    frame_rel_path: rel_path,
                                },
                            ));
                            self.queue_image_preview();
                        }
                    }
                    Some(ContextImageEntry::Product { rel_path, .. }) => {
                        if let Some(product) = &self.active_product {
                            let _ = command_tx.send(AppCommand::Storage(
                                StorageCommand::DeleteProductImage {
                                    product_id: product.product_id.clone(),
                                    rel_path,
                                },
                            ));
                        }
                    }
                    None => {}
                }
            }
            KeyCode::Char('n') => {
                let _ =
                    command_tx.send(AppCommand::Storage(StorageCommand::CreateProductAndSession));
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
                self.structure_inference = true;
                self.toast("Generating structure...".to_string(), Severity::Info);
            }
            _ => {}
        }
    }

    pub(crate) fn context_image_entries(&self) -> Vec<ContextImageEntry> {
        let mut entries = Vec::new();
        if let Some(session) = &self.active_session {
            let selected: HashSet<&str> = session
                .picks
                .selected_rel_paths
                .iter()
                .map(|s| s.as_str())
                .collect();
            for frame in &session.frames {
                entries.push(ContextImageEntry::Session {
                    rel_path: frame.rel_path.clone(),
                    sharpness_score: frame.sharpness_score,
                    created_at: frame.created_at,
                    selected: selected.contains(frame.rel_path.as_str()),
                });
            }
        }
        if let Some(product) = &self.active_product {
            let hero_rel = product.hero_rel_path.as_deref();
            let image_source = |rel_path: &str, uploaded: bool| -> String {
                if rel_path.starts_with("remote/") {
                    "remote".to_string()
                } else if uploaded {
                    "synced".to_string()
                } else {
                    "local".to_string()
                }
            };
            for image in &product.images {
                entries.push(ContextImageEntry::Product {
                    rel_path: image.rel_path.clone(),
                    created_at: image.created_at,
                    source: image_source(&image.rel_path, image.uploaded_url.is_some()),
                    hero: hero_rel == Some(image.rel_path.as_str()),
                });
            }
            if let Some(hero_rel) = hero_rel {
                let hero_missing = !product.images.iter().any(|img| img.rel_path == hero_rel);
                if hero_missing {
                    let hero_path = storage::product_dir(&self.captures_dir, &product.product_id)
                        .join(hero_rel);
                    if hero_path.exists() || product.hero_uploaded_url.is_some() {
                        entries.push(ContextImageEntry::Product {
                            rel_path: hero_rel.to_string(),
                            created_at: product.updated_at,
                            source: image_source(hero_rel, product.hero_uploaded_url.is_some()),
                            hero: true,
                        });
                    }
                }
            }
        }
        entries
    }

    pub(crate) fn context_image_count(&self) -> usize {
        self.context_image_entries().len()
    }

    fn handle_listings_keys(&mut self, key: KeyEvent, command_tx: &Sender<AppCommand>) {
        match key.code {
            KeyCode::Up => {
                if self.listings_field_selected > 0 {
                    self.listings_field_selected -= 1;
                    self.queue_image_preview();
                }
            }
            KeyCode::Down => {
                let entries = self.listing_field_entries();
                if self.listings_field_selected + 1 < entries.len() {
                    self.listings_field_selected += 1;
                    self.queue_image_preview();
                }
            }
            KeyCode::Enter | KeyCode::Char('e') => {
                self.start_listings_field_editing();
            }
            KeyCode::Char('E') => {
                self.start_listings_editing();
            }
            KeyCode::Char('r') => {
                self.generate_listing(true, false);
            }
            KeyCode::Char('p') => {
                self.generate_listing(false, false);
            }
            KeyCode::Char('P') => {
                self.generate_listing(false, true);
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
                                self.queue_image_preview();
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
                                self.queue_image_preview();
                            }
                        }
                    }
                    KeyCode::Char('S') => {
                        self.handle_ctrl_save(command_tx);
                    }
                    KeyCode::Char('g') => {
                        self.products_mode = ProductsMode::Grid;
                        self.products_loading = true;
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
                self.products_loading = false;
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
                self.product_syncing = false;
                self.structure_inference = false;
                self.listing_inference = false;
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
                self.listings_field_edit_name = None;
                self.listings_field_edit_image_index = None;
                self.listings_field_edit_dimension = None;
                self.listings_field_edit_kind = ListingEditKind::Text;
                self.listings_field_list_offset = 0;
                self.listings_editing = false;
                self.listings_edit_buffer.clear();
                self.listings_selected = 0;
                self.context_focus = ContextFocus::Images;
                self.session_frame_selected = 0;
                if let Some(notice) = self.pending_post_save_notice.take() {
                    self.emit_post_save_notice(notice);
                }
                if let Some(request) = self.pending_context_pipeline.take() {
                    if self
                        .active_product
                        .as_ref()
                        .and_then(|p| p.structure_json.as_ref())
                        .is_some()
                    {
                        if let Some(cmd) =
                            self.build_listing_command(request.dry_run, request.publish)
                        {
                            self.pending_commands.push(AppCommand::Storage(cmd));
                            self.listing_inference = true;
                            self.toast("Listing request queued.".to_string(), Severity::Info);
                        }
                    }
                }
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
                self.active_session = Some(session);
                let count = self.context_image_count();
                if count == 0 {
                    self.session_frame_selected = 0;
                } else {
                    self.session_frame_selected =
                        self.session_frame_selected.min(count.saturating_sub(1));
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
                self.pending_post_save_notice = None;
                self.pending_context_pipeline = None;
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
                self.listings_field_edit_name = None;
                self.listings_field_edit_image_index = None;
                self.listings_field_edit_dimension = None;
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
                self.pending_post_save_notice = None;
                self.pending_context_pipeline = None;
                self.products_loading = false;
                self.product_syncing = false;
                self.structure_inference = false;
                self.listing_inference = false;
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

    pub fn spinner_frame(&self) -> &'static str {
        const FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        let elapsed = self.spinner_started_at.elapsed().as_millis() / 100;
        let idx = (elapsed as usize) % FRAMES.len();
        FRAMES[idx]
    }

    fn emit_post_save_notice(&mut self, notice: PostSaveNotice) {
        let synced = self.config.hermes_api_key_present;
        let prefix = if synced {
            "Synced to Supabase. "
        } else {
            "Saved locally (Supabase sync unavailable). "
        };
        let message = match notice {
            PostSaveNotice::ContextUpdated => format!(
                "{prefix}Tip: re-run Structure (r in Structure) or Listings (r in Listings) after Structure."
            ),
            PostSaveNotice::StructureUpdated => {
                format!("{prefix}Tip: re-run Listings (r in Listings) to refresh listing fields.")
            }
            PostSaveNotice::ListingsUpdated => {
                if synced {
                    "Listings synced to Supabase.".to_string()
                } else {
                    "Listings saved locally.".to_string()
                }
            }
        };
        self.activity.push(ActivityEntry {
            at: Local::now(),
            severity: Severity::Info,
            message,
        });
    }

    fn next_tab(&mut self) {
        self.active_tab = match self.active_tab {
            AppTab::Home => AppTab::Quickstart,
            AppTab::Quickstart => AppTab::Products,
            AppTab::Products => AppTab::Activity,
            AppTab::Activity => AppTab::Settings,
            AppTab::Settings => AppTab::Home,
        };
    }

    fn prev_tab(&mut self) {
        self.active_tab = match self.active_tab {
            AppTab::Home => AppTab::Settings,
            AppTab::Quickstart => AppTab::Home,
            AppTab::Products => AppTab::Quickstart,
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
        let mut aspect_entries = Self::build_aspect_entries(&listing);
        let mut image_entries = Self::build_image_entries(&listing);
        let mut dimension_entries = Self::build_dimension_entries(&listing);
        let mut entries = Vec::new();
        for (field, label, kind) in LISTING_FIELDS {
            if *field == ListingFieldKey::Images {
                let image_count = listing.images.len();
                let label = if image_count == 0 {
                    format!("{} (none)", label)
                } else if image_count == 1 {
                    format!("{} (1 image)", label)
                } else {
                    format!("{} ({} images)", label, image_count)
                };
                entries.push(ListingFieldEntry {
                    key: *field,
                    label,
                    value: listing_field_value(&listing, *field),
                    kind: *kind,
                    indent: 0,
                    aspect_name: None,
                    image_index: None,
                    dimension_key: None,
                });
                entries.append(&mut image_entries);
                continue;
            }
            if *field == ListingFieldKey::Aspects {
                entries.push(ListingFieldEntry {
                    key: *field,
                    label: (*label).to_string(),
                    value: Value::Null,
                    kind: *kind,
                    indent: 0,
                    aspect_name: None,
                    image_index: None,
                    dimension_key: None,
                });
                entries.append(&mut aspect_entries);
                continue;
            }
            if *field == ListingFieldKey::PackageDimensions {
                entries.push(ListingFieldEntry {
                    key: *field,
                    label: (*label).to_string(),
                    value: listing_field_value(&listing, *field),
                    kind: *kind,
                    indent: 0,
                    aspect_name: None,
                    image_index: None,
                    dimension_key: None,
                });
                entries.append(&mut dimension_entries);
                continue;
            }
            entries.push(ListingFieldEntry {
                key: *field,
                label: (*label).to_string(),
                value: listing_field_value(&listing, *field),
                kind: *kind,
                indent: 0,
                aspect_name: None,
                image_index: None,
                dimension_key: None,
            });
        }
        entries
    }

    fn build_image_entries(listing: &storage::MarketplaceListing) -> Vec<ListingFieldEntry> {
        listing
            .images
            .iter()
            .enumerate()
            .map(|(idx, url)| ListingFieldEntry {
                key: ListingFieldKey::ImageValue,
                label: format!("Image {}", idx + 1),
                value: Value::String(url.clone()),
                kind: ListingEditKind::Text,
                indent: 4,
                aspect_name: None,
                image_index: Some(idx),
                dimension_key: None,
            })
            .collect()
    }

    fn build_dimension_entries(listing: &storage::MarketplaceListing) -> Vec<ListingFieldEntry> {
        let dimensions = listing
            .package
            .as_ref()
            .and_then(|pkg| pkg.dimensions.as_ref());
        let length = dimensions
            .and_then(|dims| Number::from_f64(dims.length).map(Value::Number))
            .unwrap_or(Value::Null);
        let width = dimensions
            .and_then(|dims| Number::from_f64(dims.width).map(Value::Number))
            .unwrap_or(Value::Null);
        let height = dimensions
            .and_then(|dims| Number::from_f64(dims.height).map(Value::Number))
            .unwrap_or(Value::Null);
        let unit = dimensions
            .map(|dims| Value::String(dims.unit.clone()))
            .unwrap_or(Value::Null);

        vec![
            ListingFieldEntry {
                key: ListingFieldKey::PackageDimensionValue,
                label: "Length".to_string(),
                value: length,
                kind: ListingEditKind::Number,
                indent: 4,
                aspect_name: None,
                image_index: None,
                dimension_key: Some(PackageDimensionKey::Length),
            },
            ListingFieldEntry {
                key: ListingFieldKey::PackageDimensionValue,
                label: "Width".to_string(),
                value: width,
                kind: ListingEditKind::Number,
                indent: 4,
                aspect_name: None,
                image_index: None,
                dimension_key: Some(PackageDimensionKey::Width),
            },
            ListingFieldEntry {
                key: ListingFieldKey::PackageDimensionValue,
                label: "Height".to_string(),
                value: height,
                kind: ListingEditKind::Number,
                indent: 4,
                aspect_name: None,
                image_index: None,
                dimension_key: Some(PackageDimensionKey::Height),
            },
            ListingFieldEntry {
                key: ListingFieldKey::PackageDimensionValue,
                label: "Unit".to_string(),
                value: unit,
                kind: ListingEditKind::Text,
                indent: 4,
                aspect_name: None,
                image_index: None,
                dimension_key: Some(PackageDimensionKey::Unit),
            },
        ]
    }

    fn build_aspect_entries(listing: &storage::MarketplaceListing) -> Vec<ListingFieldEntry> {
        let mut entries = Vec::new();
        let mut seen = HashSet::new();

        for spec in &listing.aspect_specs {
            let name = spec.name.trim();
            if name.is_empty() {
                continue;
            }
            let key = name.to_string();
            seen.insert(key.clone());
            let values = listing.aspects.get(name).cloned().unwrap_or_default();
            entries.push(aspect_entry(&key, &values));
        }

        let mut extras = listing
            .aspects
            .keys()
            .filter(|name| !seen.contains(*name))
            .cloned()
            .collect::<Vec<_>>();
        extras.sort();
        for name in extras {
            let values = listing.aspects.get(&name).cloned().unwrap_or_default();
            entries.push(aspect_entry(&name, &values));
        }

        entries
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
        let mut listing = product.listings.get(&key).cloned().unwrap_or_default();
        if field_key == ListingFieldKey::AspectValue {
            let Some(name) = self.listings_field_edit_name.clone() else {
                self.toast("No aspect selected.".to_string(), Severity::Warning);
                return false;
            };
            let values = match parse_aspect_values_input(&self.listings_field_edit_buffer) {
                Ok(values) => values,
                Err(err) => {
                    self.toast(format!("Invalid aspect values: {err}"), Severity::Error);
                    return false;
                }
            };
            if values.is_empty() {
                listing.aspects.remove(&name);
            } else {
                listing.aspects.insert(name, values);
            }
        } else if field_key == ListingFieldKey::ImageValue {
            let Some(index) = self.listings_field_edit_image_index else {
                self.toast("No image selected.".to_string(), Severity::Warning);
                return false;
            };
            let value = self
                .listings_field_edit_buffer
                .lines()
                .map(|line| line.trim())
                .find(|line| !line.is_empty())
                .unwrap_or("")
                .to_string();
            if index >= listing.images.len() {
                self.toast(
                    "Image selection out of range.".to_string(),
                    Severity::Warning,
                );
                return false;
            }
            if value.is_empty() {
                listing.images.remove(index);
            } else {
                listing.images[index] = value;
            }
        } else if field_key == ListingFieldKey::PackageDimensionValue {
            let Some(dimension) = self.listings_field_edit_dimension else {
                self.toast("No dimension selected.".to_string(), Severity::Warning);
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
            if let Err(err) = apply_package_dimension_value(&mut listing, dimension, &value) {
                self.toast(format!("Invalid value: {err}"), Severity::Error);
                return false;
            }
        } else {
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
            if let Err(err) = apply_listing_field_value(&mut listing, field_key, &value) {
                self.toast(format!("Invalid value: {err}"), Severity::Error);
                return false;
            }
        }

        let mut listings = product.listings.clone();
        listings.insert(key, listing);
        let _ = command_tx.send(AppCommand::Storage(StorageCommand::SetProductListings {
            product_id: product.product_id.clone(),
            listings,
        }));
        self.pending_post_save_notice = Some(PostSaveNotice::ListingsUpdated);
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
        self.pending_post_save_notice = Some(PostSaveNotice::StructureUpdated);
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
    Images,
    ImageValue,
    Title,
    Description,
    Price,
    Currency,
    CategoryLabel,
    CategoryId,
    Aspects,
    AspectValue,
    Condition,
    ConditionId,
    PackageWeight,
    PackageDimensions,
    PackageDimensionValue,
    Quantity,
    MerchantLocationKey,
    FulfillmentPolicyId,
    PaymentPolicyId,
    ReturnPolicyId,
    Status,
    ListingId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageDimensionKey {
    Length,
    Width,
    Height,
    Unit,
}

#[derive(Debug, Clone)]
pub struct ListingFieldEntry {
    pub key: ListingFieldKey,
    pub label: String,
    pub value: Value,
    pub kind: ListingEditKind,
    pub indent: usize,
    pub aspect_name: Option<String>,
    pub image_index: Option<usize>,
    pub dimension_key: Option<PackageDimensionKey>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListingEditKind {
    Text,
    Number,
    Integer,
    Json,
    Lines,
}

const LISTING_FIELDS: &[(ListingFieldKey, &str, ListingEditKind)] = &[
    (ListingFieldKey::Images, "Images", ListingEditKind::Lines),
    (ListingFieldKey::Title, "Title", ListingEditKind::Text),
    (
        ListingFieldKey::Description,
        "Description",
        ListingEditKind::Text,
    ),
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
        ListingFieldKey::Aspects,
        "Item Aspects",
        ListingEditKind::Json,
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
        ListingFieldKey::PackageWeight,
        "Package Weight",
        ListingEditKind::Text,
    ),
    (
        ListingFieldKey::PackageDimensions,
        "Package Dimensions",
        ListingEditKind::Text,
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
        ListingFieldKey::Images => {
            if listing.images.is_empty() {
                Value::Null
            } else {
                Value::Array(
                    listing
                        .images
                        .iter()
                        .map(|value| Value::String(value.clone()))
                        .collect(),
                )
            }
        }
        ListingFieldKey::ImageValue => Value::Null,
        ListingFieldKey::Title => listing
            .title
            .clone()
            .map(Value::String)
            .unwrap_or(Value::Null),
        ListingFieldKey::Description => listing
            .description
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
        ListingFieldKey::Aspects => {
            if listing.aspects.is_empty() {
                Value::Null
            } else {
                serde_json::to_value(&listing.aspects).unwrap_or(Value::Null)
            }
        }
        ListingFieldKey::AspectValue => Value::Null,
        ListingFieldKey::Condition => listing
            .condition
            .clone()
            .map(Value::String)
            .unwrap_or(Value::Null),
        ListingFieldKey::ConditionId => listing
            .condition_id
            .map(|value| Value::Number(Number::from(value)))
            .unwrap_or(Value::Null),
        ListingFieldKey::PackageWeight => listing
            .package
            .as_ref()
            .and_then(|pkg| pkg.weight.as_ref())
            .map(|weight| Value::String(format_package_weight(weight)))
            .unwrap_or(Value::Null),
        ListingFieldKey::PackageDimensions => listing
            .package
            .as_ref()
            .and_then(|pkg| pkg.dimensions.as_ref())
            .map(|dims| Value::String(format_package_dimensions(dims)))
            .unwrap_or(Value::Null),
        ListingFieldKey::PackageDimensionValue => Value::Null,
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

fn format_package_weight(weight: &storage::ListingWeight) -> String {
    format!("{} {}", weight.value, weight.unit.to_ascii_uppercase())
}

fn format_package_dimensions(dimensions: &storage::ListingDimensions) -> String {
    let length = format!("{:.1}", dimensions.length);
    let width = format!("{:.1}", dimensions.width);
    let height = format!("{:.1}", dimensions.height);
    format!(
        "{} x {} x {} {}",
        length,
        width,
        height,
        dimensions.unit.to_ascii_uppercase()
    )
}

fn aspect_entry(name: &str, values: &[String]) -> ListingFieldEntry {
    ListingFieldEntry {
        key: ListingFieldKey::AspectValue,
        label: name.to_string(),
        value: aspect_values_to_value(values),
        kind: ListingEditKind::Text,
        indent: 4,
        aspect_name: Some(name.to_string()),
        image_index: None,
        dimension_key: None,
    }
}

fn aspect_values_to_value(values: &[String]) -> Value {
    if values.is_empty() {
        return Value::Null;
    }
    Value::Array(
        values
            .iter()
            .map(|value| Value::String(value.clone()))
            .collect(),
    )
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
        ListingEditKind::Json => {
            serde_json::from_str::<Value>(buffer).map_err(|err| err.to_string())
        }
        ListingEditKind::Lines => {
            let values = parse_lines_input(buffer)?;
            if values.is_empty() {
                Ok(Value::Null)
            } else {
                Ok(Value::Array(
                    values.into_iter().map(Value::String).collect::<Vec<_>>(),
                ))
            }
        }
    }
}

fn parse_lines_input(buffer: &str) -> Result<Vec<String>, String> {
    let trimmed = buffer.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    if let Ok(json) = serde_json::from_str::<Value>(buffer) {
        return Ok(coerce_aspect_values(&json)
            .into_iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect());
    }
    let values = buffer
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    Ok(values)
}

fn format_aspect_values_edit_buffer(value: &Value) -> String {
    let values = clean_aspect_values(coerce_aspect_values(value));
    if values.is_empty() {
        String::new()
    } else {
        values.join(", ")
    }
}

fn format_lines_edit_buffer(value: &Value) -> String {
    let values = coerce_aspect_values(value)
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    if values.is_empty() {
        String::new()
    } else {
        values.join("\n")
    }
}

fn parse_aspect_values_input(buffer: &str) -> Result<Vec<String>, String> {
    let trimmed = buffer.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    if let Ok(value) = serde_json::from_str::<Value>(buffer) {
        let values = coerce_aspect_values(&value);
        return Ok(clean_aspect_values(values));
    }
    let values = trimmed
        .split(|c| c == ',' || c == '\n')
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    Ok(clean_aspect_values(values))
}

fn parse_aspects_value(value: &Value) -> Result<BTreeMap<String, Vec<String>>, String> {
    match value {
        Value::Null => Ok(BTreeMap::new()),
        Value::Object(map) => Ok(parse_aspects_object(map)),
        Value::Array(arr) => Ok(parse_aspects_array(arr)),
        _ => Err("expected a JSON object or array".to_string()),
    }
}

fn parse_aspects_object(map: &serde_json::Map<String, Value>) -> BTreeMap<String, Vec<String>> {
    let mut out = BTreeMap::new();
    for (key, value) in map {
        let values = coerce_aspect_values(value);
        upsert_aspect_values(&mut out, key, values);
    }
    out
}

fn parse_aspects_array(items: &[Value]) -> BTreeMap<String, Vec<String>> {
    let mut out = BTreeMap::new();
    for item in items {
        let Value::Object(obj) = item else {
            continue;
        };
        let name = obj
            .get("name")
            .or_else(|| obj.get("aspectName"))
            .or_else(|| obj.get("aspect"))
            .and_then(|value| value.as_str());
        let values = obj
            .get("values")
            .or_else(|| obj.get("value"))
            .or_else(|| obj.get("aspectValues"))
            .or_else(|| obj.get("aspectValue"));
        if let (Some(name), Some(values)) = (name, values) {
            let values = coerce_aspect_values(values);
            upsert_aspect_values(&mut out, name, values);
            continue;
        }
        for (key, value) in obj {
            let values = coerce_aspect_values(value);
            upsert_aspect_values(&mut out, key, values);
        }
    }
    out
}

fn coerce_aspect_values(value: &Value) -> Vec<String> {
    match value {
        Value::Null => Vec::new(),
        Value::String(text) => vec![text.trim().to_string()],
        Value::Number(num) => vec![num.to_string()],
        Value::Bool(val) => vec![val.to_string()],
        Value::Array(items) => items.iter().flat_map(coerce_aspect_values).collect(),
        Value::Object(obj) => obj
            .get("value")
            .or_else(|| obj.get("values"))
            .map(coerce_aspect_values)
            .unwrap_or_default(),
    }
}

fn clean_aspect_values(mut values: Vec<String>) -> Vec<String> {
    values.retain(|value| !value.trim().is_empty());
    values.sort();
    values.dedup();
    values
}

fn upsert_aspect_values(
    aspects: &mut BTreeMap<String, Vec<String>>,
    name: &str,
    mut values: Vec<String>,
) {
    let name = name.trim();
    if name.is_empty() {
        return;
    }
    values.retain(|value| !value.trim().is_empty());
    if values.is_empty() {
        return;
    }
    let entry = aspects.entry(name.to_string()).or_default();
    entry.append(&mut values);
    entry.sort();
    entry.dedup();
}

fn apply_package_weight(
    listing: &mut storage::MarketplaceListing,
    weight: Option<storage::ListingWeight>,
) {
    let mut package = listing.package.clone().unwrap_or_default();
    package.weight = weight;
    if package.weight.is_none() && package.dimensions.is_none() {
        listing.package = None;
    } else {
        listing.package = Some(package);
    }
}

fn apply_package_dimensions(
    listing: &mut storage::MarketplaceListing,
    dimensions: Option<storage::ListingDimensions>,
) {
    let mut package = listing.package.clone().unwrap_or_default();
    package.dimensions = dimensions;
    if package.weight.is_none() && package.dimensions.is_none() {
        listing.package = None;
    } else {
        listing.package = Some(package);
    }
}

fn apply_package_dimension_value(
    listing: &mut storage::MarketplaceListing,
    dimension: PackageDimensionKey,
    value: &Value,
) -> Result<(), String> {
    if value.is_null() {
        apply_package_dimensions(listing, None);
        return Ok(());
    }
    let mut package = listing.package.clone().unwrap_or_default();
    let mut dimensions = package
        .dimensions
        .clone()
        .unwrap_or_else(default_listing_dimensions);

    match dimension {
        PackageDimensionKey::Length => {
            dimensions.length = parse_dimension_number(value)?;
        }
        PackageDimensionKey::Width => {
            dimensions.width = parse_dimension_number(value)?;
        }
        PackageDimensionKey::Height => {
            dimensions.height = parse_dimension_number(value)?;
        }
        PackageDimensionKey::Unit => {
            let unit = parse_dimension_unit_value(value)?;
            dimensions.unit = unit.to_string();
        }
    }

    package.dimensions = Some(dimensions);
    listing.package = Some(package);
    Ok(())
}

fn default_listing_dimensions() -> storage::ListingDimensions {
    storage::ListingDimensions {
        length: 1.0,
        width: 1.0,
        height: 1.0,
        unit: "INCH".to_string(),
    }
}

fn parse_dimension_number(value: &Value) -> Result<f64, String> {
    let number = match value {
        Value::Number(num) => num
            .as_f64()
            .ok_or_else(|| "expected a number".to_string())?,
        Value::String(text) => text
            .trim()
            .parse::<f64>()
            .map_err(|_| "expected a number".to_string())?,
        _ => return Err("expected a number".to_string()),
    };
    if number <= 0.0 {
        return Err("dimension must be greater than 0".to_string());
    }
    Ok(ceil_one_decimal(number))
}

fn parse_dimension_unit_value(value: &Value) -> Result<&'static str, String> {
    let text = match value {
        Value::String(text) => text,
        Value::Number(num) => return Err(format!("unexpected unit: {}", num)),
        Value::Null => return Err("expected unit".to_string()),
        _ => return Err("expected unit".to_string()),
    };
    parse_dimension_unit(text).ok_or_else(|| "expected INCH or CENTIMETER".to_string())
}

fn parse_package_weight_value(value: &Value) -> Result<Option<storage::ListingWeight>, String> {
    match value {
        Value::Null => Ok(None),
        Value::String(text) => parse_package_weight_text(text),
        Value::Number(num) => num
            .as_f64()
            .map(|value| value.ceil())
            .map(|value| {
                Some(storage::ListingWeight {
                    value: value.max(1.0) as u32,
                    unit: "OUNCE".to_string(),
                })
            })
            .ok_or_else(|| "expected a weight like 10 OUNCE".to_string()),
        Value::Object(_) => serde_json::from_value::<storage::ListingWeight>(value.clone())
            .map(Some)
            .map_err(|err| err.to_string()),
        _ => Err("expected a weight like 10 OUNCE".to_string()),
    }
}

fn parse_package_weight_text(text: &str) -> Result<Option<storage::ListingWeight>, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if let Ok(json) = serde_json::from_str::<Value>(trimmed)
        && json.is_object()
    {
        return parse_package_weight_value(&json);
    }
    let number = extract_numbers(trimmed)
        .first()
        .copied()
        .ok_or_else(|| "expected a weight like 10 OUNCE".to_string())?;
    let unit = parse_weight_unit(trimmed).unwrap_or("OUNCE");
    let value = number.ceil().max(1.0) as u32;
    Ok(Some(storage::ListingWeight {
        value,
        unit: unit.to_string(),
    }))
}

fn parse_package_dimensions_value(
    value: &Value,
) -> Result<Option<storage::ListingDimensions>, String> {
    match value {
        Value::Null => Ok(None),
        Value::String(text) => parse_package_dimensions_text(text),
        Value::Object(_) => serde_json::from_value::<storage::ListingDimensions>(value.clone())
            .map(Some)
            .map_err(|err| err.to_string()),
        _ => Err("expected dimensions like 6.0 x 4.0 x 2.0 INCH".to_string()),
    }
}

fn parse_package_dimensions_text(text: &str) -> Result<Option<storage::ListingDimensions>, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if let Ok(json) = serde_json::from_str::<Value>(trimmed)
        && json.is_object()
    {
        return parse_package_dimensions_value(&json);
    }
    let numbers = extract_numbers(trimmed);
    if numbers.len() < 3 {
        return Err("expected 3 dimension values (L x W x H)".to_string());
    }
    let unit = parse_dimension_unit(trimmed).unwrap_or("INCH");
    let length = ceil_one_decimal(numbers[0]);
    let width = ceil_one_decimal(numbers[1]);
    let height = ceil_one_decimal(numbers[2]);
    if length <= 0.0 || width <= 0.0 || height <= 0.0 {
        return Err("dimensions must be greater than 0".to_string());
    }
    Ok(Some(storage::ListingDimensions {
        length,
        width,
        height,
        unit: unit.to_string(),
    }))
}

fn parse_weight_unit(text: &str) -> Option<&'static str> {
    for token in unit_tokens(text) {
        match token.as_str() {
            "lb" | "lbs" | "pound" | "pounds" => return Some("POUND"),
            "oz" | "ounce" | "ounces" => return Some("OUNCE"),
            _ => {}
        }
    }
    None
}

fn parse_dimension_unit(text: &str) -> Option<&'static str> {
    for token in unit_tokens(text) {
        match token.as_str() {
            "in" | "inch" | "inches" => return Some("INCH"),
            "cm" | "centimeter" | "centimeters" => return Some("CENTIMETER"),
            _ => {}
        }
    }
    None
}

fn unit_tokens(text: &str) -> Vec<String> {
    text.split(|ch: char| !ch.is_ascii_alphabetic())
        .filter(|token| !token.is_empty())
        .map(|token| token.to_ascii_lowercase())
        .collect()
}

fn extract_numbers(text: &str) -> Vec<f64> {
    let mut out = Vec::new();
    let mut buffer = String::new();
    for ch in text.chars() {
        if ch.is_ascii_digit() || ch == '.' {
            buffer.push(ch);
        } else if !buffer.is_empty() {
            if let Ok(value) = buffer.parse::<f64>() {
                out.push(value);
            }
            buffer.clear();
        }
    }
    if !buffer.is_empty() {
        if let Ok(value) = buffer.parse::<f64>() {
            out.push(value);
        }
    }
    out
}

fn ceil_one_decimal(value: f64) -> f64 {
    (value * 10.0).ceil() / 10.0
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
    let string_list_value = |value: &Value| match value {
        Value::Null => Ok(Vec::new()),
        Value::Array(_) | Value::String(_) => Ok(coerce_aspect_values(value)
            .into_iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect()),
        _ => Err("expected a list of strings".to_string()),
    };

    match key {
        ListingFieldKey::Images => listing.images = string_list_value(value)?,
        ListingFieldKey::Title => listing.title = text_value(value)?,
        ListingFieldKey::Description => listing.description = text_value(value)?,
        ListingFieldKey::Price => listing.price = number_value(value)?,
        ListingFieldKey::Currency => listing.currency = text_value(value)?,
        ListingFieldKey::CategoryLabel => listing.category_label = text_value(value)?,
        ListingFieldKey::CategoryId => listing.category_id = text_value(value)?,
        ListingFieldKey::Aspects => listing.aspects = parse_aspects_value(value)?,
        ListingFieldKey::Condition => listing.condition = text_value(value)?,
        ListingFieldKey::ConditionId => listing.condition_id = integer_value(value)?,
        ListingFieldKey::PackageWeight => {
            let weight = parse_package_weight_value(value)?;
            apply_package_weight(listing, weight);
        }
        ListingFieldKey::PackageDimensions => {
            let dimensions = parse_package_dimensions_value(value)?;
            apply_package_dimensions(listing, dimensions);
        }
        ListingFieldKey::Quantity => listing.quantity = integer_value(value)?,
        ListingFieldKey::MerchantLocationKey => listing.merchant_location_key = text_value(value)?,
        ListingFieldKey::FulfillmentPolicyId => listing.fulfillment_policy_id = text_value(value)?,
        ListingFieldKey::PaymentPolicyId => listing.payment_policy_id = text_value(value)?,
        ListingFieldKey::ReturnPolicyId => listing.return_policy_id = text_value(value)?,
        ListingFieldKey::Status => listing.status = text_value(value)?,
        ListingFieldKey::ListingId => listing.listing_id = text_value(value)?,
        ListingFieldKey::AspectValue => {
            return Err("use aspect editor to update values".to_string());
        }
        ListingFieldKey::ImageValue => {
            return Err("use image editor to update values".to_string());
        }
        ListingFieldKey::PackageDimensionValue => {
            return Err("use dimension editor to update values".to_string());
        }
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
