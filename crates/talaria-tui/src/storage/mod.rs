use std::collections::{BTreeMap, HashMap};
use std::ffi::OsStr;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub mod worker;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductImageEntry {
    pub rel_path: String,
    pub created_at: DateTime<Local>,
    pub sharpness_score: Option<f64>,
    pub uploaded_url: Option<String>,
    #[serde(default)]
    pub uploaded_media_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductManifest {
    pub product_id: String,
    pub sku_alias: String,
    pub display_name: Option<String>,
    #[serde(default)]
    pub context_text: Option<String>,
    #[serde(default)]
    pub structure_json: Option<serde_json::Value>,
    #[serde(default)]
    pub listings: HashMap<String, MarketplaceListing>,
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
    pub images: Vec<ProductImageEntry>,
    pub hero_rel_path: Option<String>,
    #[serde(default)]
    pub hero_uploaded_url: Option<String>,
    #[serde(default)]
    pub hero_media_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MarketplaceListing {
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    pub price: Option<f64>,
    pub currency: Option<String>,
    #[serde(default)]
    pub images: Vec<String>,
    pub category_id: Option<String>,
    pub category_label: Option<String>,
    pub condition: Option<String>,
    pub condition_id: Option<i32>,
    #[serde(default)]
    pub allowed_conditions: Vec<String>,
    #[serde(default)]
    pub allowed_condition_ids: Vec<i32>,
    #[serde(default)]
    pub aspects: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub aspect_specs: Vec<ListingAspectSpec>,
    pub quantity: Option<i32>,
    pub merchant_location_key: Option<String>,
    pub fulfillment_policy_id: Option<String>,
    pub payment_policy_id: Option<String>,
    pub return_policy_id: Option<String>,
    pub status: Option<String>,
    pub listing_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ListingAspectSpec {
    pub name: String,
    pub required: bool,
    #[serde(default)]
    pub samples: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionFrameEntry {
    pub rel_path: String,
    pub created_at: DateTime<Local>,
    pub sharpness_score: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionPicks {
    #[serde(default)]
    pub selected_rel_paths: Vec<String>,
    #[serde(default)]
    pub hero_rel_path: Option<String>,
    #[serde(default)]
    pub angle_rel_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionManifest {
    pub session_id: String,
    pub product_id: String,
    pub created_at: DateTime<Local>,
    pub committed_at: Option<DateTime<Local>>,
    pub frames: Vec<SessionFrameEntry>,
    pub picks: SessionPicks,
}

#[derive(Debug, Clone)]
pub struct ProductSummary {
    pub product_id: String,
    pub sku_alias: String,
    pub display_name: Option<String>,
    pub updated_at: DateTime<Local>,
    pub image_count: usize,
}

pub fn default_captures_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("TALARIA_CAPTURES_DIR") {
        return PathBuf::from(dir);
    }
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("talaria")
        .join("captures")
}

pub fn products_dir(base: &Path) -> PathBuf {
    base.join("products")
}

pub fn sessions_dir(base: &Path) -> PathBuf {
    base.join("sessions")
}

pub fn logs_dir(base: &Path) -> PathBuf {
    base.join("logs")
}

pub fn product_dir(base: &Path, product_id: &str) -> PathBuf {
    products_dir(base).join(product_id)
}

pub fn session_dir(base: &Path, session_id: &str) -> PathBuf {
    sessions_dir(base).join(session_id)
}

pub fn product_manifest_path(base: &Path, product_id: &str) -> PathBuf {
    product_dir(base, product_id).join("product.json")
}

pub fn session_manifest_path(base: &Path, session_id: &str) -> PathBuf {
    session_dir(base, session_id).join("session.json")
}

pub fn product_images_dir(base: &Path, product_id: &str) -> PathBuf {
    product_dir(base, product_id).join("images")
}

pub fn product_curated_dir(base: &Path, product_id: &str) -> PathBuf {
    product_dir(base, product_id).join("curated")
}

pub fn product_remote_dir(base: &Path, product_id: &str) -> PathBuf {
    product_dir(base, product_id).join("remote")
}

pub fn session_frames_dir(base: &Path, session_id: &str) -> PathBuf {
    session_dir(base, session_id).join("frames")
}

pub fn session_picks_dir(base: &Path, session_id: &str) -> PathBuf {
    session_dir(base, session_id).join("picks")
}

pub fn ensure_base_dirs(base: &Path) -> Result<()> {
    fs::create_dir_all(products_dir(base)).context("create products dir")?;
    fs::create_dir_all(sessions_dir(base)).context("create sessions dir")?;
    fs::create_dir_all(logs_dir(base)).context("create logs dir")?;
    Ok(())
}

pub fn new_product_id() -> String {
    Uuid::new_v4().to_string()
}

pub fn new_session_id() -> String {
    Uuid::new_v4().to_string()
}

pub fn sku_alias_for_product(product_id: &str) -> String {
    let short = product_id.split('-').next().unwrap_or(product_id);
    format!("H-{short}")
}

pub fn atomic_write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let dir = path.parent().context("missing parent directory")?;
    fs::create_dir_all(dir).context("create parent dir")?;

    let tmp = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(value).context("serialize json")?;
    {
        let mut file = fs::File::create(&tmp).context("create temp json")?;
        file.write_all(&bytes).context("write temp json")?;
        file.sync_all().ok();
    }
    fs::rename(&tmp, path).context("rename temp json")?;
    Ok(())
}

pub fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_slice(&bytes).context("parse json")
}

fn listings_from_value(value: serde_json::Value) -> HashMap<String, MarketplaceListing> {
    serde_json::from_value::<HashMap<String, MarketplaceListing>>(value).unwrap_or_default()
}

pub fn list_products(base: &Path) -> Result<Vec<ProductSummary>> {
    let mut out = Vec::new();
    let dir = products_dir(base);
    if !dir.exists() {
        return Ok(out);
    }
    for entry in fs::read_dir(dir).context("read products dir")? {
        let entry = entry?;
        let path = entry.path().join("product.json");
        if !path.exists() {
            continue;
        }
        let manifest: ProductManifest = read_json(&path)?;
        out.push(ProductSummary {
            product_id: manifest.product_id,
            sku_alias: manifest.sku_alias,
            display_name: manifest.display_name,
            updated_at: manifest.updated_at,
            image_count: manifest.images.len(),
        });
    }
    out.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(out)
}

pub fn create_product(base: &Path) -> Result<ProductManifest> {
    ensure_base_dirs(base)?;
    let product_id = new_product_id();
    let sku_alias = sku_alias_for_product(&product_id);
    let now = Local::now();
    let manifest = ProductManifest {
        product_id: product_id.clone(),
        sku_alias,
        display_name: None,
        context_text: None,
        structure_json: None,
        listings: HashMap::new(),
        created_at: now,
        updated_at: now,
        images: Vec::new(),
        hero_rel_path: None,
        hero_uploaded_url: None,
        hero_media_id: None,
    };

    fs::create_dir_all(product_images_dir(base, &product_id)).context("create product images")?;
    fs::create_dir_all(product_curated_dir(base, &product_id)).context("create product curated")?;
    atomic_write_json(&product_manifest_path(base, &product_id), &manifest)?;
    Ok(manifest)
}

pub fn set_product_image_uploaded_url(
    base: &Path,
    product_id: &str,
    rel_path: &str,
    url: String,
    media_id: Option<String>,
) -> Result<ProductManifest> {
    let path = product_manifest_path(base, product_id);
    let mut manifest: ProductManifest = read_json(&path)?;
    if let Some(img) = manifest.images.iter_mut().find(|i| i.rel_path == rel_path) {
        img.uploaded_url = Some(url);
        img.uploaded_media_id = media_id;
        manifest.updated_at = Local::now();
        atomic_write_json(&path, &manifest)?;
    }
    Ok(manifest)
}

pub fn set_product_hero_uploaded_url(
    base: &Path,
    product_id: &str,
    url: String,
    media_id: Option<String>,
) -> Result<ProductManifest> {
    let path = product_manifest_path(base, product_id);
    let mut manifest: ProductManifest = read_json(&path)?;
    manifest.hero_uploaded_url = Some(url);
    manifest.hero_media_id = media_id;
    manifest.updated_at = Local::now();
    atomic_write_json(&path, &manifest)?;
    Ok(manifest)
}

pub fn set_product_context_text(
    base: &Path,
    product_id: &str,
    text: String,
) -> Result<ProductManifest> {
    let path = product_manifest_path(base, product_id);
    let mut manifest: ProductManifest = read_json(&path)?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        manifest.context_text = None;
    } else {
        manifest.context_text = Some(text);
    }
    manifest.updated_at = Local::now();
    atomic_write_json(&path, &manifest)?;
    Ok(manifest)
}

pub fn set_product_structure_json(
    base: &Path,
    product_id: &str,
    structure_json: Option<serde_json::Value>,
) -> Result<ProductManifest> {
    let path = product_manifest_path(base, product_id);
    let mut manifest: ProductManifest = read_json(&path)?;
    manifest.structure_json = structure_json;
    manifest.updated_at = Local::now();
    atomic_write_json(&path, &manifest)?;
    Ok(manifest)
}

pub fn set_product_listings(
    base: &Path,
    product_id: &str,
    listings: HashMap<String, MarketplaceListing>,
) -> Result<ProductManifest> {
    let path = product_manifest_path(base, product_id);
    let mut manifest: ProductManifest = read_json(&path)?;
    manifest.listings = listings;
    manifest.updated_at = Local::now();
    atomic_write_json(&path, &manifest)?;
    Ok(manifest)
}

pub fn upsert_product_from_remote(
    base: &Path,
    row: &talaria_core::models::ProductRecord,
) -> Result<ProductManifest> {
    ensure_base_dirs(base)?;
    let path = product_manifest_path(base, &row.id);
    let mut manifest = if path.exists() {
        read_json(&path)?
    } else {
        ProductManifest {
            product_id: row.id.clone(),
            sku_alias: row.sku_alias.clone(),
            display_name: row.display_name.clone(),
            context_text: row.context_text.clone(),
            structure_json: row.structure_json.clone(),
            listings: listings_from_value(row.listings_json.clone()),
            created_at: row.created_at.with_timezone(&Local),
            updated_at: row.updated_at.with_timezone(&Local),
            images: Vec::new(),
            hero_rel_path: None,
            hero_uploaded_url: None,
            hero_media_id: None,
        }
    };

    manifest.sku_alias = row.sku_alias.clone();
    manifest.display_name = row.display_name.clone();
    manifest.context_text = row.context_text.clone();
    manifest.structure_json = row.structure_json.clone();
    manifest.listings = listings_from_value(row.listings_json.clone());
    manifest.updated_at = row.updated_at.with_timezone(&Local);
    if manifest.created_at < row.created_at.with_timezone(&Local) {
        manifest.created_at = row.created_at.with_timezone(&Local);
    }

    fs::create_dir_all(product_images_dir(base, &row.id)).context("create product images")?;
    fs::create_dir_all(product_curated_dir(base, &row.id)).context("create product curated")?;
    atomic_write_json(&path, &manifest)?;
    Ok(manifest)
}

pub fn create_session(base: &Path, product_id: &str) -> Result<SessionManifest> {
    ensure_base_dirs(base)?;
    let session_id = new_session_id();
    let now = Local::now();
    let manifest = SessionManifest {
        session_id: session_id.clone(),
        product_id: product_id.to_string(),
        created_at: now,
        committed_at: None,
        frames: Vec::new(),
        picks: SessionPicks::default(),
    };
    fs::create_dir_all(session_frames_dir(base, &session_id)).context("create session frames")?;
    fs::create_dir_all(session_picks_dir(base, &session_id)).context("create session picks")?;
    atomic_write_json(&session_manifest_path(base, &session_id), &manifest)?;
    Ok(manifest)
}

pub fn append_session_frame(
    base: &Path,
    session_id: &str,
    frame_rel_path: &str,
    sharpness_score: Option<f64>,
    created_at: DateTime<Local>,
) -> Result<SessionManifest> {
    let path = session_manifest_path(base, session_id);
    let mut manifest: SessionManifest = read_json(&path)?;
    manifest.frames.push(SessionFrameEntry {
        rel_path: frame_rel_path.to_string(),
        created_at,
        sharpness_score,
    });
    atomic_write_json(&path, &manifest)?;
    Ok(manifest)
}

pub fn toggle_session_frame_pick(
    base: &Path,
    session_id: &str,
    frame_rel_path: &str,
) -> Result<SessionManifest> {
    let path = session_manifest_path(base, session_id);
    let mut manifest: SessionManifest = read_json(&path)?;
    if !manifest.frames.iter().any(|f| f.rel_path == frame_rel_path) {
        return Err(anyhow::anyhow!("Frame not found in session."));
    }
    if let Some(idx) = manifest
        .picks
        .selected_rel_paths
        .iter()
        .position(|p| p == frame_rel_path)
    {
        manifest.picks.selected_rel_paths.remove(idx);
    } else {
        manifest
            .picks
            .selected_rel_paths
            .push(frame_rel_path.to_string());
    }
    atomic_write_json(&path, &manifest)?;
    Ok(manifest)
}

pub fn delete_session_frame(base: &Path, session_id: &str, frame_rel_path: &str) -> Result<()> {
    let full = session_dir(base, session_id).join(frame_rel_path);
    if full.exists() {
        fs::remove_file(&full).with_context(|| format!("remove {}", full.display()))?;
    }
    let path = session_manifest_path(base, session_id);
    let mut manifest: SessionManifest = read_json(&path)?;
    manifest.frames.retain(|f| f.rel_path != frame_rel_path);
    manifest
        .picks
        .selected_rel_paths
        .retain(|p| p != frame_rel_path);
    atomic_write_json(&path, &manifest)?;
    Ok(())
}

pub fn abandon_session(base: &Path, session_id: &str) -> Result<PathBuf> {
    let src = session_dir(base, session_id);
    let trash = sessions_dir(base).join("_trash");
    fs::create_dir_all(&trash).context("create sessions trash")?;
    let stamp = Local::now().format("%Y%m%d_%H%M%S");
    let dst = trash.join(format!("{session_id}_{stamp}"));
    fs::rename(&src, &dst)
        .with_context(|| format!("move {} -> {}", src.display(), dst.display()))?;
    Ok(dst)
}

pub fn commit_session(
    base: &Path,
    session_id: &str,
) -> Result<(ProductManifest, SessionManifest, usize)> {
    let session_path = session_manifest_path(base, session_id);
    let mut session: SessionManifest = read_json(&session_path)?;
    if session.committed_at.is_some() {
        return Ok((load_product(base, &session.product_id)?, session, 0));
    }

    let product_id = session.product_id.clone();
    let product_path = product_manifest_path(base, &product_id);
    let mut product: ProductManifest = read_json(&product_path)?;

    let mut copied = 0usize;
    let now = Local::now();

    let mut commit_paths = Vec::new();
    if !session.picks.selected_rel_paths.is_empty() {
        let selected: std::collections::HashSet<&str> = session
            .picks
            .selected_rel_paths
            .iter()
            .map(|s| s.as_str())
            .collect();
        for frame in &session.frames {
            if selected.contains(frame.rel_path.as_str()) {
                commit_paths.push(frame.rel_path.clone());
            }
        }
    } else {
        if let Some(hero) = &session.picks.hero_rel_path {
            commit_paths.push(hero.clone());
        }
        commit_paths.extend(session.picks.angle_rel_paths.iter().cloned());
    }

    if commit_paths.is_empty() {
        return Err(anyhow::anyhow!(
            "No images selected. Use Enter to select frames before committing."
        ));
    }

    for (idx, rel) in commit_paths.iter().enumerate() {
        let src = session_dir(base, session_id).join(rel);
        if !src.exists() {
            continue;
        }
        let ext = src
            .extension()
            .and_then(OsStr::to_str)
            .unwrap_or("jpg")
            .to_string();
        let filename = format!("img_{:03}_{}.{}", idx + 1, now.format("%Y%m%d_%H%M%S"), ext);
        let dst_rel = format!("images/{filename}");
        let dst = product_dir(base, &product_id).join(&dst_rel);
        fs::copy(&src, &dst)
            .with_context(|| format!("copy {} -> {}", src.display(), dst.display()))?;
        product.images.push(ProductImageEntry {
            rel_path: dst_rel.clone(),
            created_at: now,
            sharpness_score: None,
            uploaded_url: None,
            uploaded_media_id: None,
        });
        copied += 1;
    }

    if let Some(_) = &session.picks.hero_rel_path {
        let hero_src = session_picks_dir(base, session_id).join("hero.jpg");
        let hero_dst = product_curated_dir(base, &product_id).join("hero.jpg");
        if hero_src.exists() {
            fs::copy(&hero_src, &hero_dst).ok();
            product.hero_rel_path = Some("curated/hero.jpg".to_string());
        }
    }

    product.updated_at = now;
    session.committed_at = Some(now);

    atomic_write_json(&product_path, &product)?;
    atomic_write_json(&session_path, &session)?;
    Ok((product, session, copied))
}

pub fn load_product(base: &Path, product_id: &str) -> Result<ProductManifest> {
    read_json(&product_manifest_path(base, product_id))
}

pub fn load_session(base: &Path, session_id: &str) -> Result<SessionManifest> {
    read_json(&session_manifest_path(base, session_id))
}

pub fn delete_product(base: &Path, product_id: &str) -> Result<usize> {
    let product_path = product_dir(base, product_id);
    if product_path.exists() {
        fs::remove_dir_all(&product_path)
            .with_context(|| format!("remove {}", product_path.display()))?;
    }

    let mut removed_sessions = 0usize;
    let sessions_root = sessions_dir(base);
    if sessions_root.exists() {
        for entry in fs::read_dir(&sessions_root).context("read sessions dir")? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let name = entry.file_name();
            if name.to_str().map(|s| s.starts_with('_')).unwrap_or(false) {
                continue;
            }
            let manifest_path = path.join("session.json");
            if !manifest_path.exists() {
                continue;
            }
            let manifest: SessionManifest = read_json(&manifest_path)?;
            if manifest.product_id == product_id {
                fs::remove_dir_all(&path).with_context(|| format!("remove {}", path.display()))?;
                removed_sessions += 1;
            }
        }
    }

    Ok(removed_sessions)
}
