use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use chrono::Local;
use crossbeam_channel::{Receiver, Sender};
use serde_json::Value;
use tokio::runtime::Runtime;

use crate::storage;
use crate::types::{ActivityEntry, AppEvent, Severity, StorageCommand, StorageEvent};
use talaria_core::client::HermesClient;
use talaria_core::models::{
    CategorySelectionInput, HsufEnrichRequest, ImagesSource, JobState, ListingResponse,
    MarketplaceId, ProductCreateRequest, ProductRecord, ProductUpdateRequest, PublicListingRequest,
    PublicPipelineOverrides,
};

pub fn spawn_storage_worker(
    base_dir: PathBuf,
    hermes: Option<HermesClient>,
    cmd_rx: Receiver<StorageCommand>,
    event_tx: Sender<AppEvent>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let base = base_dir;
        let rt = Runtime::new().expect("tokio runtime");

        loop {
            let cmd = match cmd_rx.recv() {
                Ok(cmd) => cmd,
                Err(_) => return,
            };

            if matches!(cmd, StorageCommand::Shutdown) {
                return;
            }

            let res: Result<()> = (|| match cmd {
                StorageCommand::CreateProductAndSession => {
                    if let Some(hermes) = hermes.as_ref().filter(|h| h.has_api_key()) {
                        let req = ProductCreateRequest::default();
                        let row = rt.block_on(hermes.create_product(&req))?;
                        let product = storage::upsert_product_from_remote(&base, &row)?;
                        let session = storage::create_session(&base, &product.product_id)?;
                        let _ = event_tx.send(AppEvent::Storage(StorageEvent::ProductSelected(
                            product.clone(),
                        )));
                        let _ =
                            event_tx.send(AppEvent::Storage(StorageEvent::SessionStarted(session)));
                        let _ = event_tx.send(AppEvent::Activity(ActivityEntry {
                            at: Local::now(),
                            severity: Severity::Success,
                            message: format!("New product created: {}", product.sku_alias),
                        }));
                        return Ok(());
                    }

                    let product = storage::create_product(&base)?;
                    let session = storage::create_session(&base, &product.product_id)?;
                    let _ = event_tx.send(AppEvent::Storage(StorageEvent::ProductSelected(
                        product.clone(),
                    )));
                    let _ = event_tx.send(AppEvent::Storage(StorageEvent::SessionStarted(session)));
                    let _ = event_tx.send(AppEvent::Activity(ActivityEntry {
                        at: Local::now(),
                        severity: Severity::Success,
                        message: format!("New product created: {}", product.sku_alias),
                    }));
                    Ok(())
                }
                StorageCommand::ListProducts => {
                    if let Some(hermes) = hermes.as_ref().filter(|h| h.has_api_key()) {
                        let rows = rt.block_on(hermes.list_products())?;
                        let products = rows
                            .iter()
                            .map(|row| product_summary_from_record(&base, row))
                            .collect::<Vec<_>>();
                        let _ = event_tx
                            .send(AppEvent::Storage(StorageEvent::ProductsListed(products)));
                        return Ok(());
                    }
                    let products = storage::list_products(&base)?;
                    let _ =
                        event_tx.send(AppEvent::Storage(StorageEvent::ProductsListed(products)));
                    Ok(())
                }
                StorageCommand::StartSessionForProduct { product_id } => {
                    if let Some(hermes) = hermes.as_ref().filter(|h| h.has_api_key()) {
                        let row = rt.block_on(hermes.get_product(&product_id))?;
                        let product = storage::upsert_product_from_remote(&base, &row)?;
                        let product = match sync_product_media(&rt, hermes, &base, &product_id) {
                            Ok(updated) => updated,
                            Err(err) => {
                                let _ = event_tx.send(AppEvent::Activity(ActivityEntry {
                                    at: Local::now(),
                                    severity: Severity::Warning,
                                    message: format!("Media sync failed: {err}"),
                                }));
                                product
                            }
                        };
                        let session = storage::create_session(&base, &product_id)?;
                        let _ = event_tx
                            .send(AppEvent::Storage(StorageEvent::ProductSelected(product)));
                        let _ =
                            event_tx.send(AppEvent::Storage(StorageEvent::SessionStarted(session)));
                        return Ok(());
                    }
                    let product = storage::load_product(&base, &product_id)?;
                    let session = storage::create_session(&base, &product_id)?;
                    let _ =
                        event_tx.send(AppEvent::Storage(StorageEvent::ProductSelected(product)));
                    let _ = event_tx.send(AppEvent::Storage(StorageEvent::SessionStarted(session)));
                    Ok(())
                }
                StorageCommand::DeleteProduct { product_id } => {
                    let sku = storage::load_product(&base, &product_id)
                        .ok()
                        .map(|p| p.sku_alias);
                    if let Some(hermes) = hermes.as_ref().filter(|h| h.has_api_key()) {
                        rt.block_on(hermes.delete_product(&product_id))?;
                    }
                    let removed_sessions = storage::delete_product(&base, &product_id)?;
                    let _ = event_tx.send(AppEvent::Storage(StorageEvent::ProductDeleted {
                        product_id: product_id.clone(),
                        removed_sessions,
                    }));
                    if let Some(hermes) = hermes.as_ref().filter(|h| h.has_api_key()) {
                        let rows = rt.block_on(hermes.list_products())?;
                        let products = rows
                            .iter()
                            .map(|row| product_summary_from_record(&base, row))
                            .collect::<Vec<_>>();
                        let _ = event_tx
                            .send(AppEvent::Storage(StorageEvent::ProductsListed(products)));
                    } else {
                        let products = storage::list_products(&base)?;
                        let _ = event_tx
                            .send(AppEvent::Storage(StorageEvent::ProductsListed(products)));
                    }
                    let mut message = match sku {
                        Some(sku) => format!("Deleted product {sku}"),
                        None => format!("Deleted product {product_id}"),
                    };
                    if removed_sessions > 0 {
                        message.push_str(&format!(" ({} session(s) removed)", removed_sessions));
                    }
                    let _ = event_tx.send(AppEvent::Activity(ActivityEntry {
                        at: Local::now(),
                        severity: Severity::Warning,
                        message,
                    }));
                    Ok(())
                }
                StorageCommand::SetProductContextText { product_id, text } => {
                    if let Some(hermes) = hermes.as_ref().filter(|h| h.has_api_key()) {
                        let trimmed = text.trim();
                        let update = ProductUpdateRequest {
                            context_text: if trimmed.is_empty() {
                                None
                            } else {
                                Some(text.clone())
                            },
                            ..Default::default()
                        };
                        let row = rt.block_on(hermes.update_product(&product_id, &update))?;
                        let updated = storage::upsert_product_from_remote(&base, &row)?;
                        let _ = event_tx
                            .send(AppEvent::Storage(StorageEvent::ProductSelected(updated)));
                        return Ok(());
                    }
                    let updated = storage::set_product_context_text(&base, &product_id, text)?;
                    let _ =
                        event_tx.send(AppEvent::Storage(StorageEvent::ProductSelected(updated)));
                    Ok(())
                }
                StorageCommand::SetProductStructureJson {
                    product_id,
                    structure_json,
                } => {
                    if let Some(hermes) = hermes.as_ref().filter(|h| h.has_api_key()) {
                        let update = ProductUpdateRequest {
                            structure_json: Some(structure_json.clone()),
                            ..Default::default()
                        };
                        let row = rt.block_on(hermes.update_product(&product_id, &update))?;
                        let updated = storage::upsert_product_from_remote(&base, &row)?;
                        let _ = event_tx
                            .send(AppEvent::Storage(StorageEvent::ProductSelected(updated)));
                        return Ok(());
                    }
                    let updated = storage::set_product_structure_json(
                        &base,
                        &product_id,
                        Some(structure_json),
                    )?;
                    let _ =
                        event_tx.send(AppEvent::Storage(StorageEvent::ProductSelected(updated)));
                    Ok(())
                }
                StorageCommand::SetProductListings {
                    product_id,
                    listings,
                } => {
                    if let Some(hermes) = hermes.as_ref().filter(|h| h.has_api_key()) {
                        let listings_json = serde_json::to_value(&listings)?;
                        let update = ProductUpdateRequest {
                            listings_json: Some(listings_json),
                            ..Default::default()
                        };
                        let row = rt.block_on(hermes.update_product(&product_id, &update))?;
                        let updated = storage::upsert_product_from_remote(&base, &row)?;
                        let _ = event_tx
                            .send(AppEvent::Storage(StorageEvent::ProductSelected(updated)));
                        return Ok(());
                    }
                    let updated = storage::set_product_listings(&base, &product_id, listings)?;
                    let _ =
                        event_tx.send(AppEvent::Storage(StorageEvent::ProductSelected(updated)));
                    Ok(())
                }
                StorageCommand::GenerateProductStructure {
                    product_id,
                    sku_alias,
                } => {
                    let hermes = hermes
                        .as_ref()
                        .context("Hermes client unavailable for structure")?;
                    if !hermes.has_api_key() {
                        return Err(anyhow::anyhow!(
                            "HERMES_API_KEY missing; structure generation requires Hermes."
                        ));
                    }
                    let images = rt.block_on(fetch_product_images(hermes, &product_id))?;
                    if images.is_empty() {
                        return Err(anyhow::anyhow!("No uploaded images found for product."));
                    }
                    let enrich = HsufEnrichRequest {
                        images,
                        sku: Some(sku_alias),
                    };
                    let response = rt.block_on(hermes.hsuf_enrich(&enrich, false))?;
                    let structure_json = serde_json::to_value(&response.product)?;
                    if hermes.has_api_key() {
                        let update = ProductUpdateRequest {
                            structure_json: Some(structure_json),
                            ..Default::default()
                        };
                        let row = rt.block_on(hermes.update_product(&product_id, &update))?;
                        let updated = storage::upsert_product_from_remote(&base, &row)?;
                        let _ = event_tx
                            .send(AppEvent::Storage(StorageEvent::ProductSelected(updated)));
                        let _ = event_tx.send(AppEvent::Activity(ActivityEntry {
                            at: Local::now(),
                            severity: Severity::Success,
                            message: "Structure generated.".to_string(),
                        }));
                        return Ok(());
                    }

                    let updated = storage::set_product_structure_json(
                        &base,
                        &product_id,
                        Some(structure_json),
                    )?;
                    let _ =
                        event_tx.send(AppEvent::Storage(StorageEvent::ProductSelected(updated)));
                    let _ = event_tx.send(AppEvent::Activity(ActivityEntry {
                        at: Local::now(),
                        severity: Severity::Success,
                        message: "Structure generated.".to_string(),
                    }));
                    Ok(())
                }
                StorageCommand::GenerateProductListing {
                    product_id,
                    sku_alias,
                    marketplace,
                    settings,
                    condition,
                    condition_id,
                    dry_run,
                    publish,
                } => {
                    let hermes = hermes
                        .as_ref()
                        .context("Hermes client unavailable for listings")?;
                    if !hermes.has_api_key() {
                        return Err(anyhow::anyhow!(
                            "HERMES_API_KEY missing; listing generation requires Hermes."
                        ));
                    }
                    let images = rt.block_on(fetch_product_images(hermes, &product_id))?;
                    if images.is_empty() {
                        return Err(anyhow::anyhow!("No uploaded images found for product."));
                    }

                    let mut structure_json = None;
                    let mut listings = None;
                    if hermes.has_api_key() {
                        let row = rt.block_on(hermes.get_product(&product_id))?;
                        structure_json = row.structure_json.clone();
                        listings = serde_json::from_value(row.listings_json).ok();
                    }
                    if structure_json.is_none() {
                        if let Ok(local) = storage::load_product(&base, &product_id) {
                            structure_json = local.structure_json.clone();
                            listings = Some(local.listings.clone());
                        }
                    }

                    let structure_json =
                        structure_json.context("Structure missing; generate it first.")?;

                    let Some(merchant_location_key) = settings.merchant_location_key.clone() else {
                        return Err(anyhow::anyhow!("Missing eBay merchant location key."));
                    };
                    let Some(fulfillment_policy_id) = settings.fulfillment_policy_id.clone() else {
                        return Err(anyhow::anyhow!("Missing eBay fulfillment policy id."));
                    };
                    let Some(payment_policy_id) = settings.payment_policy_id.clone() else {
                        return Err(anyhow::anyhow!("Missing eBay payment policy id."));
                    };
                    let Some(return_policy_id) = settings.return_policy_id.clone() else {
                        return Err(anyhow::anyhow!("Missing eBay return policy id."));
                    };

                    let overrides = PublicPipelineOverrides {
                        resolved_images: Some(images.clone()),
                        category: None,
                        condition,
                        condition_id,
                        product: Some(structure_json),
                    };
                    let marketplace_key = marketplace_key(marketplace.clone());
                    let req = PublicListingRequest {
                        dry_run: Some(dry_run),
                        fulfillment_policy_id,
                        images_source: ImagesSource::Multiple(images),
                        marketplace: Some(marketplace),
                        merchant_location_key,
                        overrides: Some(overrides),
                        payment_policy_id,
                        publish: Some(publish),
                        return_policy_id,
                        sku: Some(sku_alias),
                        use_signed_urls: None,
                    };
                    let job = rt.block_on(hermes.enqueue_listing(&req))?;
                    let job_id = job.job_id;
                    let _ = event_tx.send(AppEvent::Activity(ActivityEntry {
                        at: Local::now(),
                        severity: Severity::Info,
                        message: format!("Listing job queued: {job_id}"),
                    }));

                    let deadline = Instant::now() + Duration::from_secs(180);
                    let (job_request, resp) = loop {
                        let info = rt.block_on(hermes.get_job_status(&job_id))?;
                        match info.state {
                            JobState::Queued {} | JobState::Running {} => {}
                            JobState::Completed { result } => break (info.request, result),
                            JobState::Failed { error, stage } => {
                                let detail = stage
                                    .as_deref()
                                    .map(|s| format!(" (stage: {s})"))
                                    .unwrap_or_default();
                                return Err(anyhow::anyhow!("Listing job failed{detail}: {error}"));
                            }
                        }
                        if Instant::now() >= deadline {
                            let _ = event_tx.send(AppEvent::Activity(ActivityEntry {
                                at: Local::now(),
                                severity: Severity::Warning,
                                message: format!("Listing job {job_id} still running after 180s."),
                            }));
                            return Ok(());
                        }
                        thread::sleep(Duration::from_secs(2));
                    };
                    let mut listings_map = listings.unwrap_or_else(std::collections::HashMap::new);
                    let listing = listing_from_response(
                        &resp,
                        Some(&job_request),
                        &settings,
                        dry_run,
                        publish,
                    )?;
                    listings_map.insert(marketplace_key, listing);

                    if hermes.has_api_key() {
                        let listings_json = serde_json::to_value(&listings_map)?;
                        let update = ProductUpdateRequest {
                            listings_json: Some(listings_json),
                            ..Default::default()
                        };
                        let row = rt.block_on(hermes.update_product(&product_id, &update))?;
                        let updated = storage::upsert_product_from_remote(&base, &row)?;
                        let _ = event_tx
                            .send(AppEvent::Storage(StorageEvent::ProductSelected(updated)));
                        let _ = event_tx.send(AppEvent::Activity(ActivityEntry {
                            at: Local::now(),
                            severity: Severity::Success,
                            message: "Listing draft generated.".to_string(),
                        }));
                        return Ok(());
                    }

                    let updated = storage::set_product_listings(&base, &product_id, listings_map)?;
                    let _ =
                        event_tx.send(AppEvent::Storage(StorageEvent::ProductSelected(updated)));
                    let _ = event_tx.send(AppEvent::Activity(ActivityEntry {
                        at: Local::now(),
                        severity: Severity::Success,
                        message: "Listing draft generated.".to_string(),
                    }));
                    Ok(())
                }
                StorageCommand::AbandonSession { session_id } => {
                    let moved = storage::abandon_session(&base, &session_id)?;
                    let _ = event_tx.send(AppEvent::Storage(StorageEvent::SessionAbandoned {
                        session_id,
                        moved_to: moved.to_string_lossy().to_string(),
                    }));
                    Ok(())
                }
                StorageCommand::CommitSession { session_id } => {
                    let (product, session, committed_count) =
                        storage::commit_session(&base, &session_id)?;
                    let _ = event_tx.send(AppEvent::Storage(StorageEvent::CommitCompleted {
                        product,
                        session,
                        committed_count,
                    }));
                    Ok(())
                }
                StorageCommand::AppendSessionFrame {
                    session_id,
                    frame_rel_path,
                    created_at,
                    sharpness_score,
                } => {
                    let session = storage::append_session_frame(
                        &base,
                        &session_id,
                        &frame_rel_path,
                        sharpness_score,
                        created_at,
                    )?;
                    let _ = event_tx.send(AppEvent::Storage(StorageEvent::SessionUpdated(session)));
                    Ok(())
                }
                StorageCommand::ToggleSessionFrameSelection {
                    session_id,
                    frame_rel_path,
                } => {
                    let session =
                        storage::toggle_session_frame_pick(&base, &session_id, &frame_rel_path)?;
                    let _ = event_tx.send(AppEvent::Storage(StorageEvent::SessionUpdated(session)));
                    Ok(())
                }
                StorageCommand::DeleteSessionFrame {
                    session_id,
                    frame_rel_path,
                } => {
                    storage::delete_session_frame(&base, &session_id, &frame_rel_path)?;
                    let session = storage::load_session(&base, &session_id)?;
                    let _ = event_tx.send(AppEvent::Storage(StorageEvent::SessionUpdated(session)));
                    Ok(())
                }
                StorageCommand::DeleteProductImage {
                    product_id,
                    rel_path,
                } => {
                    let product = storage::load_product(&base, &product_id)?;
                    let mut uploaded_url = None;
                    let mut media_id = None;
                    let mut needs_remote = false;

                    if let Some(img) = product.images.iter().find(|img| img.rel_path == rel_path) {
                        uploaded_url = img.uploaded_url.clone();
                        media_id = img.uploaded_media_id.clone();
                        needs_remote = uploaded_url.is_some() || media_id.is_some();
                    }
                    if product.hero_rel_path.as_deref() == Some(rel_path.as_str()) {
                        if media_id.is_none() {
                            media_id = product.hero_media_id.clone();
                        }
                        if uploaded_url.is_none() {
                            uploaded_url = product.hero_uploaded_url.clone();
                        }
                        needs_remote = needs_remote || uploaded_url.is_some() || media_id.is_some();
                    }

                    if needs_remote {
                        let hermes = hermes
                            .as_ref()
                            .context("Hermes client unavailable for delete")?;
                        if !hermes.has_api_key() {
                            return Err(anyhow::anyhow!(
                                "HERMES_API_KEY missing; delete requires Hermes."
                            ));
                        }
                        if media_id.is_none() {
                            if let Some(url) = &uploaded_url {
                                let response =
                                    rt.block_on(hermes.list_product_media(&product_id))?;
                                media_id = response
                                    .items
                                    .into_iter()
                                    .find(|item| item.url == *url)
                                    .map(|item| item.media_id);
                            }
                        }
                        let Some(media_id) = media_id else {
                            return Err(anyhow::anyhow!(
                                "Missing media id for synced image; re-sync and try again."
                            ));
                        };
                        rt.block_on(hermes.delete_media(&media_id))?;
                    }

                    let updated =
                        storage::delete_product_image(&base, &product_id, rel_path.as_str())?;
                    let _ =
                        event_tx.send(AppEvent::Storage(StorageEvent::ProductSelected(updated)));
                    let name = Path::new(&rel_path)
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or(rel_path.as_str());
                    let _ = event_tx.send(AppEvent::Activity(ActivityEntry {
                        at: Local::now(),
                        severity: Severity::Success,
                        message: format!("Deleted image {name}."),
                    }));
                    Ok(())
                }
                StorageCommand::SyncProductMedia { product_id } => {
                    let hermes = hermes
                        .as_ref()
                        .context("Hermes client unavailable for sync")?;
                    if !hermes.has_api_key() {
                        return Err(anyhow::anyhow!(
                            "HERMES_API_KEY missing; sync requires Hermes."
                        ));
                    }
                    let updated = sync_product_media(&rt, hermes, &base, &product_id)?;
                    let _ =
                        event_tx.send(AppEvent::Storage(StorageEvent::ProductSelected(updated)));
                    let _ = event_tx.send(AppEvent::Activity(ActivityEntry {
                        at: Local::now(),
                        severity: Severity::Success,
                        message: "Product media synced.".to_string(),
                    }));
                    Ok(())
                }
                StorageCommand::SyncProductData { product_id } => {
                    let hermes = hermes
                        .as_ref()
                        .context("Hermes client unavailable for sync")?;
                    if !hermes.has_api_key() {
                        return Err(anyhow::anyhow!(
                            "HERMES_API_KEY missing; sync requires Hermes."
                        ));
                    }
                    let updated = sync_product_data(&rt, hermes, &base, &product_id)?;
                    let _ =
                        event_tx.send(AppEvent::Storage(StorageEvent::ProductSelected(updated)));
                    let _ = event_tx.send(AppEvent::Activity(ActivityEntry {
                        at: Local::now(),
                        severity: Severity::Success,
                        message: "Product data synced.".to_string(),
                    }));
                    Ok(())
                }
                StorageCommand::Shutdown => Ok(()),
            })();

            if let Err(err) = res {
                let message = format!("{err:#}");
                let _ = event_tx.send(AppEvent::Storage(StorageEvent::Error(message.clone())));
                let _ = event_tx.send(AppEvent::Activity(ActivityEntry {
                    at: Local::now(),
                    severity: Severity::Error,
                    message,
                }));
            }
        }
    })
}

fn product_summary_from_record(base: &Path, row: &ProductRecord) -> storage::ProductSummary {
    let image_count = storage::load_product(base, &row.id)
        .map(|p| p.images.len())
        .unwrap_or(0);
    let listings = serde_json::from_value::<
        std::collections::HashMap<String, storage::MarketplaceListing>,
    >(row.listings_json.clone())
    .unwrap_or_default();
    let mut marketplace_statuses = listings
        .iter()
        .map(|(marketplace, listing)| storage::MarketplaceStatus {
            marketplace: marketplace.clone(),
            published: listing.status.as_deref() == Some("published"),
        })
        .collect::<Vec<_>>();
    marketplace_statuses.sort_by(|a, b| a.marketplace.cmp(&b.marketplace));
    storage::ProductSummary {
        product_id: row.id.clone(),
        sku_alias: row.sku_alias.clone(),
        display_name: row.display_name.clone(),
        updated_at: row.updated_at.with_timezone(&Local),
        image_count,
        has_structure: row.structure_json.is_some(),
        marketplace_statuses,
    }
}

fn sync_product_media(
    rt: &Runtime,
    hermes: &HermesClient,
    base: &Path,
    product_id: &str,
) -> Result<storage::ProductManifest> {
    let response = rt.block_on(hermes.list_product_media(product_id))?;
    let mut manifest = storage::load_product(base, product_id)?;
    let remote_dir = storage::product_remote_dir(base, product_id);
    std::fs::create_dir_all(&remote_dir).context("create product remote dir")?;

    let mut remote_by_url = HashMap::new();
    for media in response.items {
        remote_by_url.insert(media.url.clone(), media);
    }
    let remote_urls: HashSet<String> = remote_by_url.keys().cloned().collect();

    manifest.images.retain(|img| match &img.uploaded_url {
        Some(url) => remote_urls.contains(url),
        None => true,
    });

    let mut existing_by_url = HashMap::new();
    for (idx, img) in manifest.images.iter().enumerate() {
        if let Some(url) = &img.uploaded_url {
            existing_by_url.insert(url.clone(), idx);
        }
    }

    let mut used_filenames = HashSet::new();
    for img in &manifest.images {
        if let Some(name) = img.rel_path.strip_prefix("remote/") {
            used_filenames.insert(name.to_string());
        }
    }

    let download_client = reqwest::Client::new();
    let mut hero = None::<(chrono::DateTime<chrono::Utc>, String, String, String)>;

    for media in remote_by_url.values() {
        let url = media.url.clone();
        let rel_path = if let Some(&idx) = existing_by_url.get(&url) {
            let entry = &mut manifest.images[idx];
            entry.uploaded_url = Some(url.clone());
            entry.uploaded_media_id = Some(media.media_id.clone());
            entry.rel_path.clone()
        } else {
            let raw_name = media
                .filename
                .as_deref()
                .or_else(|| media.object_key.rsplit('/').next())
                .unwrap_or("image");
            let mut filename = safe_filename(raw_name);
            if used_filenames.contains(&filename) {
                filename = disambiguate_filename(&filename, &used_filenames);
            }
            used_filenames.insert(filename.clone());
            let rel_path = format!("remote/{filename}");
            let created_at = media.created_at.with_timezone(&Local);
            manifest.images.push(storage::ProductImageEntry {
                rel_path: rel_path.clone(),
                created_at,
                sharpness_score: None,
                uploaded_url: Some(url.clone()),
                uploaded_media_id: Some(media.media_id.clone()),
            });
            rel_path
        };

        if matches!(
            media.purpose,
            Some(talaria_core::models::MediaPurpose::Hero)
        ) {
            let candidate = (
                media.created_at,
                url.clone(),
                media.media_id.clone(),
                rel_path,
            );
            if hero.as_ref().map(|h| candidate.0 < h.0).unwrap_or(true) {
                hero = Some(candidate);
            }
        }
    }

    for img in &manifest.images {
        let Some(url) = img.uploaded_url.as_deref() else {
            continue;
        };
        let Some(name) = img.rel_path.strip_prefix("remote/") else {
            continue;
        };
        let target = remote_dir.join(name);
        if target.exists() {
            continue;
        }
        download_media(rt, &download_client, url, &target)?;
    }

    if let Some((_, url, media_id, rel_path)) = hero {
        manifest.hero_uploaded_url = Some(url);
        manifest.hero_media_id = Some(media_id);
        manifest.hero_rel_path = Some(rel_path);
    } else if manifest
        .hero_uploaded_url
        .as_ref()
        .is_some_and(|u| !remote_urls.contains(u))
    {
        manifest.hero_uploaded_url = None;
        manifest.hero_media_id = None;
        if manifest
            .hero_rel_path
            .as_ref()
            .is_some_and(|p| p.starts_with("remote/"))
        {
            manifest.hero_rel_path = None;
        }
    }

    let keep_remote: HashSet<String> = manifest
        .images
        .iter()
        .filter_map(|img| img.rel_path.strip_prefix("remote/"))
        .map(|s| s.to_string())
        .collect();
    if let Ok(entries) = std::fs::read_dir(&remote_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
                continue;
            };
            if !keep_remote.contains(name) {
                let _ = std::fs::remove_file(&path);
            }
        }
    }

    manifest.updated_at = Local::now();
    storage::atomic_write_json(&storage::product_manifest_path(base, product_id), &manifest)?;
    Ok(manifest)
}

fn sync_product_data(
    rt: &Runtime,
    hermes: &HermesClient,
    base: &Path,
    product_id: &str,
) -> Result<storage::ProductManifest> {
    let row = rt.block_on(hermes.get_product(product_id))?;
    let updated = storage::upsert_product_from_remote(base, &row)?;
    Ok(updated)
}

fn safe_filename(raw: &str) -> String {
    let sanitized: String = raw
        .chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '-' | '_' => c,
            _ => '_',
        })
        .collect();
    if sanitized.is_empty() {
        "image".to_string()
    } else {
        sanitized
    }
}

fn disambiguate_filename(base: &str, used: &HashSet<String>) -> String {
    if !used.contains(base) {
        return base.to_string();
    }
    let (stem, ext) = match base.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() && !ext.is_empty() => (stem, Some(ext)),
        _ => (base, None),
    };
    for idx in 1..=9999 {
        let candidate = match ext {
            Some(ext) => format!("{stem}_{idx}.{ext}"),
            None => format!("{stem}_{idx}"),
        };
        if !used.contains(&candidate) {
            return candidate;
        }
    }
    format!("{base}_dup")
}

fn download_media(rt: &Runtime, client: &reqwest::Client, url: &str, path: &Path) -> Result<()> {
    let bytes = rt.block_on(async {
        let resp = client.get(url).send().await?;
        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("download failed: {status} {text}"));
        }
        Ok(resp.bytes().await?)
    })?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).context("create media cache dir")?;
    }
    std::fs::write(path, &bytes).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

async fn fetch_product_images(hermes: &HermesClient, product_id: &str) -> Result<Vec<String>> {
    let response = hermes.list_product_media(product_id).await?;
    let mut items = response.items;
    items.sort_by(|a, b| {
        let rank_a = a.rank.unwrap_or(i32::MAX);
        let rank_b = b.rank.unwrap_or(i32::MAX);
        rank_a
            .cmp(&rank_b)
            .then_with(|| a.created_at.cmp(&b.created_at))
    });

    let mut hero = Vec::new();
    let mut images = Vec::new();
    for media in items {
        match media.purpose {
            Some(talaria_core::models::MediaPurpose::Hero) => hero.push(media.url),
            Some(talaria_core::models::MediaPurpose::ProductImage) | None => images.push(media.url),
            _ => {}
        }
    }
    hero.extend(images);
    Ok(hero)
}

fn listing_from_response(
    resp: &ListingResponse,
    request: Option<&PublicListingRequest>,
    settings: &talaria_core::config::EbaySettings,
    dry_run: bool,
    publish: bool,
) -> Result<storage::MarketplaceListing> {
    let category = stage_output(resp, "category")
        .and_then(|output| output.get("selected"))
        .and_then(|value| serde_json::from_value::<CategorySelectionInput>(value.clone()).ok());

    let (condition, condition_id) = stage_output(resp, "prepare_conditions")
        .and_then(|output| {
            let label = output
                .get("default")
                .and_then(|v| v.as_str())
                .map(|v| v.to_string());
            let id = output
                .get("default_condition_id")
                .and_then(|v| v.as_i64())
                .and_then(|v| i32::try_from(v).ok());
            if label.is_none() && id.is_none() {
                None
            } else {
                Some((label, id))
            }
        })
        .unwrap_or((None, None));
    let (allowed_conditions, allowed_condition_ids) = stage_output(resp, "prepare_conditions")
        .map(|output| {
            let labels = output
                .get("allowed")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let ids = output
                .get("allowed_condition_ids")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_i64().and_then(|id| i32::try_from(id).ok()))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            (labels, ids)
        })
        .unwrap_or_default();

    let build =
        stage_output_any(resp, &["listing", "build_listing"]).context("listing stage missing")?;
    let title = build
        .get("title")
        .and_then(|v| v.as_str())
        .map(|v| v.to_string());
    let description = request_description(request)
        .or_else(|| {
            build
                .get("description")
                .and_then(|v| v.as_str())
                .map(|v| v.to_string())
        })
        .or_else(|| {
            stage_output(resp, "push_inventory")
                .and_then(|output| output.get("inventory_request"))
                .and_then(|request| request.get("product"))
                .and_then(|product| product.get("description"))
                .and_then(|v| v.as_str())
                .map(|v| v.to_string())
        })
        .and_then(|text| clean_text_value(&text));
    let price = build.get("price").and_then(|v| v.as_f64());
    let currency = build
        .get("currency")
        .and_then(|v| v.as_str())
        .map(|v| v.to_string());
    let images = request_images(request)
        .map(clean_string_list)
        .filter(|items| !items.is_empty())
        .or_else(|| {
            build
                .get("images")
                .or_else(|| build.get("media"))
                .map(string_list_from_value)
                .or_else(|| {
                    stage_output(resp, "push_inventory")
                        .and_then(|output| output.get("inventory_request"))
                        .and_then(|request| request.get("product"))
                        .and_then(|product| product.get("image_urls"))
                        .map(string_list_from_value)
                })
                .map(normalize_strings)
        })
        .unwrap_or_default();
    let condition_label = build
        .get("condition")
        .and_then(|v| v.as_str())
        .map(|v| v.to_string())
        .or(condition.clone());
    let aspects = build
        .get("aspects")
        .and_then(|value| {
            serde_json::from_value::<BTreeMap<String, Vec<String>>>(value.clone()).ok()
        })
        .or_else(|| {
            stage_output(resp, "push_inventory")
                .and_then(|output| output.get("inventory_request"))
                .and_then(|request| request.get("product"))
                .and_then(|product| product.get("aspects"))
                .and_then(|value| {
                    serde_json::from_value::<BTreeMap<String, Vec<String>>>(value.clone()).ok()
                })
        })
        .map(normalize_aspects)
        .unwrap_or_default();
    let package = build
        .get("package_weight_and_size")
        .or_else(|| build.get("packageWeightAndSize"))
        .or_else(|| build.get("package"))
        .and_then(parse_listing_package)
        .or_else(|| {
            stage_output(resp, "push_inventory")
                .and_then(|output| output.get("inventory_request"))
                .and_then(|request| {
                    request
                        .get("packageWeightAndSize")
                        .or_else(|| request.get("package_weight_and_size"))
                })
                .and_then(parse_listing_package)
        });
    let aspect_specs = stage_output(resp, "taxonomy")
        .and_then(|output| {
            output
                .get("aspects")
                .or_else(|| output.get("sample_aspects"))
        })
        .and_then(|value| {
            serde_json::from_value::<Vec<storage::ListingAspectSpec>>(value.clone()).ok()
        })
        .unwrap_or_default();

    let status = if publish {
        "published"
    } else if dry_run {
        "draft"
    } else {
        "ready"
    }
    .to_string();

    Ok(storage::MarketplaceListing {
        title,
        description,
        price,
        currency,
        images,
        category_id: category.as_ref().map(|c| c.id.clone()),
        category_label: category.as_ref().map(|c| c.label.clone()),
        condition: condition_label,
        condition_id,
        allowed_conditions,
        allowed_condition_ids,
        aspects,
        aspect_specs,
        quantity: Some(1),
        merchant_location_key: settings.merchant_location_key.clone(),
        fulfillment_policy_id: settings.fulfillment_policy_id.clone(),
        payment_policy_id: settings.payment_policy_id.clone(),
        return_policy_id: settings.return_policy_id.clone(),
        package,
        status: Some(status),
        listing_id: Some(resp.listing_id.clone()),
    })
}

fn request_description(request: Option<&PublicListingRequest>) -> Option<String> {
    let product = request
        .and_then(|req| req.overrides.as_ref())
        .and_then(|overrides| overrides.product.as_ref())?;
    product
        .get("description")
        .and_then(|value| value.as_str())
        .and_then(clean_text_value)
}

fn request_images(request: Option<&PublicListingRequest>) -> Option<Vec<String>> {
    let req = request?;
    if let Some(overrides) = req.overrides.as_ref() {
        if let Some(resolved) = overrides.resolved_images.as_ref() {
            return Some(resolved.clone());
        }
    }
    match &req.images_source {
        ImagesSource::Single(url) => Some(vec![url.clone()]),
        ImagesSource::Multiple(urls) => Some(urls.clone()),
    }
}

fn stage_output<'a>(resp: &'a ListingResponse, name: &str) -> Option<&'a serde_json::Value> {
    resp.stages
        .iter()
        .find(|s| s.name == name)
        .map(|s| &s.output)
}

fn stage_output_any<'a>(
    resp: &'a ListingResponse,
    names: &[&str],
) -> Option<&'a serde_json::Value> {
    names.iter().find_map(|name| stage_output(resp, name))
}

fn normalize_aspects(aspects: BTreeMap<String, Vec<String>>) -> BTreeMap<String, Vec<String>> {
    let mut normalized: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (key, values) in aspects {
        let name = key.trim();
        if name.is_empty() {
            continue;
        }
        let mut cleaned = values
            .into_iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();
        if cleaned.is_empty() {
            continue;
        }
        let entry = normalized.entry(name.to_string()).or_default();
        entry.append(&mut cleaned);
        entry.sort();
        entry.dedup();
    }
    normalized
}

fn parse_listing_package(value: &Value) -> Option<storage::ListingPackage> {
    if value.is_null() {
        return None;
    }
    serde_json::from_value::<storage::ListingPackage>(value.clone()).ok()
}

fn string_list_from_value(value: &serde_json::Value) -> Vec<String> {
    match value {
        serde_json::Value::Array(items) => items
            .iter()
            .filter_map(|item| item.as_str().map(|s| s.to_string()))
            .collect(),
        serde_json::Value::String(text) => vec![text.clone()],
        _ => Vec::new(),
    }
}

fn normalize_strings(values: Vec<String>) -> Vec<String> {
    let mut cleaned = values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty() && !is_redacted_token(value))
        .collect::<Vec<_>>();
    cleaned.sort();
    cleaned.dedup();
    cleaned
}

fn clean_string_list(values: Vec<String>) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for value in values {
        let trimmed = value.trim();
        if trimmed.is_empty() || is_redacted_token(trimmed) {
            continue;
        }
        let entry = trimmed.to_string();
        if seen.insert(entry.clone()) {
            out.push(entry);
        }
    }
    out
}

fn clean_text_value(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() || is_redacted_token(trimmed) {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn is_redacted_token(value: &str) -> bool {
    matches!(value, "[redacted-numeric]" | "[redacted-email]")
}

fn marketplace_key(marketplace: MarketplaceId) -> String {
    match marketplace {
        MarketplaceId::EbayUs => "EBAY_US".to_string(),
        MarketplaceId::EbayUk => "EBAY_UK".to_string(),
        MarketplaceId::EbayDe => "EBAY_DE".to_string(),
    }
}
