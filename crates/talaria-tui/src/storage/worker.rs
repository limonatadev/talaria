use std::path::PathBuf;
use std::thread;

use anyhow::Result;
use chrono::Local;
use crossbeam_channel::{Receiver, Sender};

use crate::storage;
use crate::types::{ActivityEntry, AppEvent, Severity, StorageCommand, StorageEvent};

pub fn spawn_storage_worker(
    base_dir: PathBuf,
    cmd_rx: Receiver<StorageCommand>,
    event_tx: Sender<AppEvent>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let base = base_dir;
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
                    let products = storage::list_products(&base)?;
                    let _ =
                        event_tx.send(AppEvent::Storage(StorageEvent::ProductsListed(products)));
                    Ok(())
                }
                StorageCommand::StartSessionForProduct { product_id } => {
                    let product = storage::load_product(&base, &product_id)?;
                    let session = storage::create_session(&base, &product_id)?;
                    let _ =
                        event_tx.send(AppEvent::Storage(StorageEvent::ProductSelected(product)));
                    let _ = event_tx.send(AppEvent::Storage(StorageEvent::SessionStarted(session)));
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
                StorageCommand::SetHeroPick {
                    session_id,
                    frame_rel_path,
                } => {
                    let session =
                        storage::set_session_hero_pick(&base, &session_id, &frame_rel_path)?;
                    let _ = event_tx.send(AppEvent::Storage(StorageEvent::SessionUpdated(session)));
                    Ok(())
                }
                StorageCommand::AddAnglePick {
                    session_id,
                    frame_rel_path,
                } => {
                    let session =
                        storage::add_session_angle_pick(&base, &session_id, &frame_rel_path)?;
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
                StorageCommand::Shutdown => Ok(()),
            })();

            if let Err(err) = res {
                let _ = event_tx.send(AppEvent::Storage(StorageEvent::Error(err.to_string())));
                let _ = event_tx.send(AppEvent::Activity(ActivityEntry {
                    at: Local::now(),
                    severity: Severity::Error,
                    message: err.to_string(),
                }));
            }
        }
    })
}
