use std::collections::HashMap;
use std::thread;
use std::time::Duration;

use chrono::Local;
use crossbeam_channel::{Receiver, Sender, select, tick};

use crate::types::{ActivityEntry, AppEvent, JobStatus, Severity, UploadCommand, UploadJob};

pub fn spawn_upload_worker(
    cmd_rx: Receiver<UploadCommand>,
    event_tx: Sender<AppEvent>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut jobs: HashMap<String, UploadJob> = HashMap::new();
        let ticker = tick(Duration::from_millis(200));

        loop {
            select! {
                recv(cmd_rx) -> msg => {
                    match msg {
                        Ok(UploadCommand::Enqueue(path)) => {
                            let id = Local::now().format("upl-%Y%m%d-%H%M%S-%3f").to_string();
                            let job = UploadJob {
                                id: id.clone(),
                                path,
                                status: JobStatus::InProgress,
                                progress: 0.0,
                                retries: 0,
                                last_error: None,
                            };
                            jobs.insert(id.clone(), job.clone());
                            let _ = event_tx.send(AppEvent::UploadJob(job));
                            let _ = event_tx.send(AppEvent::Activity(ActivityEntry {
                                at: Local::now(),
                                severity: Severity::Info,
                                message: format!("Upload enqueued ({id})"),
                            }));
                        }
                        Ok(UploadCommand::RetryFailed) => {
                            for job in jobs.values_mut() {
                                if job.status == JobStatus::Failed {
                                    job.status = JobStatus::InProgress;
                                    job.progress = 0.0;
                                    job.retries += 1;
                                    let _ = event_tx.send(AppEvent::UploadJob(job.clone()));
                                }
                            }
                        }
                        Ok(UploadCommand::Cancel(id)) => {
                            if let Some(job) = jobs.get_mut(&id) {
                                job.status = JobStatus::Canceled;
                                let _ = event_tx.send(AppEvent::UploadJob(job.clone()));
                            }
                        }
                        Ok(UploadCommand::EnqueueAllCurrent) => {
                            // TODO: expand in UI; worker does not know CurrentItem.
                        }
                        Ok(UploadCommand::Shutdown) | Err(_) => {
                            return;
                        }
                    }
                }
                recv(ticker) -> _ => {
                    for job in jobs.values_mut() {
                        if job.status == JobStatus::InProgress {
                            job.progress = (job.progress + 0.08).min(1.0);
                            if job.progress >= 1.0 {
                                job.status = JobStatus::Completed;
                                let _ = event_tx.send(AppEvent::Activity(ActivityEntry {
                                    at: Local::now(),
                                    severity: Severity::Success,
                                    message: format!("Upload completed ({})", job.id),
                                }));
                            }
                            let _ = event_tx.send(AppEvent::UploadJob(job.clone()));
                        }
                    }
                }
            }
        }
    })
}
