use std::collections::HashMap;
use std::thread;
use std::time::Duration;

use chrono::Local;
use crossbeam_channel::{Receiver, Sender, select, tick};

use crate::types::{ActivityEntry, AppEvent, JobStatus, ListingDraft, ListingsCommand, Severity};

pub fn spawn_listings_worker(
    cmd_rx: Receiver<ListingsCommand>,
    event_tx: Sender<AppEvent>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut drafts: HashMap<String, ListingDraft> = HashMap::new();
        let ticker = tick(Duration::from_millis(400));

        loop {
            select! {
                recv(cmd_rx) -> msg => {
                    match msg {
                        Ok(ListingsCommand::CreateDraft { marketplace }) => {
                            let id = Local::now().format("lst-%Y%m%d-%H%M%S-%3f").to_string();
                            let draft = ListingDraft {
                                id: id.clone(),
                                marketplace,
                                status: JobStatus::InProgress,
                                last_error: None,
                            };
                            drafts.insert(id.clone(), draft.clone());
                            let _ = event_tx.send(AppEvent::ListingDraft(draft));
                            let _ = event_tx.send(AppEvent::Activity(ActivityEntry {
                                at: Local::now(),
                                severity: Severity::Info,
                                message: format!("Listing draft started ({id})"),
                            }));
                        }
                        Ok(ListingsCommand::PushLive(id)) => {
                            if let Some(draft) = drafts.get_mut(&id) {
                                draft.status = JobStatus::Completed;
                                let _ = event_tx.send(AppEvent::ListingDraft(draft.clone()));
                                let _ = event_tx.send(AppEvent::Activity(ActivityEntry {
                                    at: Local::now(),
                                    severity: Severity::Success,
                                    message: format!("Listing pushed live ({})", draft.id),
                                }));
                            }
                        }
                        Ok(ListingsCommand::ExportJson(_id)) => {
                            // TODO: implement export using Hermes API types when available.
                        }
                        Ok(ListingsCommand::Shutdown) | Err(_) => {
                            return;
                        }
                    }
                }
                recv(ticker) -> _ => {
                    for draft in drafts.values_mut() {
                        if draft.status == JobStatus::InProgress {
                            draft.status = JobStatus::Completed;
                            let _ = event_tx.send(AppEvent::ListingDraft(draft.clone()));
                        }
                    }
                }
            }
        }
    })
}
