use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::thread;

use chrono::Local;
use crossbeam_channel::{Receiver, Sender};
use reqwest::header::HeaderMap;
use tokio::runtime::Runtime;

use crate::storage;
use crate::types::{ActivityEntry, AppEvent, JobStatus, Severity, UploadCommand, UploadJob};

pub fn spawn_upload_worker(
    captures_dir: PathBuf,
    hermes: Option<talaria_core::client::HermesClient>,
    cmd_rx: Receiver<UploadCommand>,
    event_tx: Sender<AppEvent>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let rt = Runtime::new().expect("tokio runtime");
        let mut jobs: HashMap<String, UploadJob> = HashMap::new();
        let upload_http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .expect("upload http client");

        loop {
            let cmd = match cmd_rx.recv() {
                Ok(cmd) => cmd,
                Err(_) => return,
            };
            match cmd {
                UploadCommand::UploadProduct { product_id } => {
                    if hermes.is_none() {
                        let _ = event_tx.send(AppEvent::Activity(ActivityEntry {
                            at: Local::now(),
                            severity: Severity::Warning,
                            message: "Hermes config missing; upload skipped (offline mode)."
                                .to_string(),
                        }));
                        continue;
                    }
                    let hermes = hermes.clone().unwrap();
                    if !hermes.has_api_key() {
                        let _ = event_tx.send(AppEvent::Activity(ActivityEntry {
                            at: Local::now(),
                            severity: Severity::Warning,
                            message: "HERMES_API_KEY missing; upload skipped (offline mode)."
                                .to_string(),
                        }));
                        continue;
                    }

                    let product = match storage::load_product(&captures_dir, &product_id) {
                        Ok(p) => p,
                        Err(err) => {
                            let _ = event_tx.send(AppEvent::Activity(ActivityEntry {
                                at: Local::now(),
                                severity: Severity::Error,
                                message: format!("Load product failed: {err}"),
                            }));
                            continue;
                        }
                    };

                    let mut targets = Vec::new();
                    if let Some(rel) = &product.hero_rel_path {
                        targets.push(rel.clone());
                    }
                    for img in &product.images {
                        if img.uploaded_url.is_none() {
                            targets.push(img.rel_path.clone());
                        }
                    }

                    if targets.is_empty() {
                        let _ = event_tx.send(AppEvent::Activity(ActivityEntry {
                            at: Local::now(),
                            severity: Severity::Info,
                            message: "Nothing to upload (all URLs present).".to_string(),
                        }));
                        continue;
                    }

                    for rel in targets {
                        let abs = storage::product_dir(&captures_dir, &product_id).join(&rel);
                        if !abs.exists() {
                            let _ = event_tx.send(AppEvent::Activity(ActivityEntry {
                                at: Local::now(),
                                severity: Severity::Warning,
                                message: format!("Missing file: {}", abs.display()),
                            }));
                            continue;
                        }

                        let id = format!(
                            "upl-{}-{}",
                            &product_id[..8.min(product_id.len())],
                            file_id(&rel)
                        );
                        let mut job = UploadJob {
                            id: id.clone(),
                            status: JobStatus::InProgress,
                            progress: 0.0,
                            last_error: None,
                        };
                        jobs.insert(id.clone(), job.clone());
                        let _ = event_tx.send(AppEvent::UploadJob(job.clone()));

                        let result = rt.block_on(upload_one(
                            &hermes,
                            &upload_http,
                            &product_id,
                            product.hero_rel_path.as_deref(),
                            &rel,
                            &abs,
                        ));
                        match result {
                            Ok(uploaded) => {
                                if rel == product.hero_rel_path.clone().unwrap_or_default() {
                                    let _ = storage::set_product_hero_uploaded_url(
                                        &captures_dir,
                                        &product_id,
                                        uploaded.url.clone(),
                                        Some(uploaded.media_id.clone()),
                                    );
                                } else {
                                    let _ = storage::set_product_image_uploaded_url(
                                        &captures_dir,
                                        &product_id,
                                        &rel,
                                        uploaded.url.clone(),
                                        Some(uploaded.media_id.clone()),
                                    );
                                }
                                job.status = JobStatus::Completed;
                                job.progress = 1.0;
                                jobs.insert(id.clone(), job.clone());
                                let _ = event_tx.send(AppEvent::UploadJob(job));
                                let _ = event_tx.send(AppEvent::Activity(ActivityEntry {
                                    at: Local::now(),
                                    severity: Severity::Success,
                                    message: format!(
                                        "Uploaded {} -> {}",
                                        short_name(&rel),
                                        uploaded.url
                                    ),
                                }));
                            }
                            Err(err) => {
                                job.status = JobStatus::Failed;
                                job.last_error = Some(err.to_string());
                                jobs.insert(id.clone(), job.clone());
                                let _ = event_tx.send(AppEvent::UploadJob(job.clone()));
                                let _ = event_tx.send(AppEvent::Activity(ActivityEntry {
                                    at: Local::now(),
                                    severity: Severity::Error,
                                    message: format!("Upload failed for {}: {}", rel, err),
                                }));
                            }
                        }
                    }
                    let _ = event_tx.send(AppEvent::UploadFinished {
                        product_id: product_id.clone(),
                    });
                }
                UploadCommand::Shutdown => return,
            }
        }
    })
}

fn short_name(rel: &str) -> String {
    Path::new(rel)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(rel)
        .to_string()
}

fn file_id(rel: &str) -> String {
    short_name(rel)
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

#[derive(Debug, Clone)]
struct UploadedAsset {
    url: String,
    media_id: String,
}

async fn upload_one(
    hermes: &talaria_core::client::HermesClient,
    upload_http: &reqwest::Client,
    product_id: &str,
    hero_rel_path: Option<&str>,
    rel: &str,
    abs: &Path,
) -> anyhow::Result<UploadedAsset> {
    let filename = abs
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("image.jpg")
        .to_string();
    let content_type = guess_content_type(abs);
    let content_length = std::fs::metadata(abs).ok().map(|m| m.len() as i64);

    let purpose = if hero_rel_path.is_some_and(|h| h == rel) {
        talaria_core::models::MediaPurpose::Hero
    } else {
        talaria_core::models::MediaPurpose::ProductImage
    };

    let create = talaria_core::models::CreateUploadRequest {
        content_length,
        content_type: Some(content_type.to_string()),
        filename,
        metadata: None,
        product_id: Some(product_id.to_string()),
        purpose: Some(purpose),
        session_id: None,
        sha256: None,
    };

    let session = hermes.create_media_upload(&create).await?;
    let mut headers = HeaderMap::new();
    if let Some(h) = &session.headers {
        for (k, v) in h {
            headers.insert(
                reqwest::header::HeaderName::from_bytes(k.as_bytes())?,
                reqwest::header::HeaderValue::from_str(v)?,
            );
        }
    }
    headers.insert(
        reqwest::header::CONTENT_TYPE,
        reqwest::header::HeaderValue::from_static(content_type),
    );

    let body = tokio::fs::read(abs).await?;
    let put = upload_http
        .put(&session.upload_url)
        .headers(headers)
        .body(body)
        .send()
        .await?;
    if !put.status().is_success() {
        let status = put.status();
        let text = put.text().await.unwrap_or_default();
        let _ = hermes.abort_media_upload(&session.upload_id).await;
        return Err(anyhow::anyhow!("upload PUT failed: {status} {text}"));
    }

    let etag = put
        .headers()
        .get(reqwest::header::ETAG)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim_matches('"').to_string());

    let complete = talaria_core::models::CompleteUploadRequest { etag, sha256: None };
    let done = hermes
        .complete_media_upload(&session.upload_id, Some(&complete))
        .await?;

    Ok(UploadedAsset {
        url: done.media.url,
        media_id: done.media.media_id,
    })
}

fn guess_content_type(path: &Path) -> &'static str {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase());
    match ext.as_deref() {
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        Some("bmp") => "image/bmp",
        _ => "application/octet-stream",
    }
}
