use std::collections::HashMap;
use std::thread;
use std::time::{Duration, Instant};

use chrono::Local;
use crossbeam_channel::{Receiver, Sender, select, tick};

use crate::types::{ActivityEntry, AppEvent, EnrichCommand, EnrichJob, JobStatus, Severity};

pub fn spawn_enrich_worker(
    cmd_rx: Receiver<EnrichCommand>,
    event_tx: Sender<AppEvent>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut jobs: HashMap<String, EnrichJob> = HashMap::new();
        let mut started_at: HashMap<String, Instant> = HashMap::new();
        let ticker = tick(Duration::from_millis(300));

        loop {
            select! {
                recv(cmd_rx) -> msg => {
                    match msg {
                        Ok(EnrichCommand::Enqueue(urls)) => {
                            let id = Local::now().format("enr-%Y%m%d-%H%M%S-%3f").to_string();
                            let job = EnrichJob {
                                id: id.clone(),
                                image_urls: urls,
                                status: JobStatus::InProgress,
                                started_at: Some(Local::now()),
                                finished_at: None,
                                usage_estimate: None,
                            };
                            jobs.insert(id.clone(), job.clone());
                            started_at.insert(id.clone(), Instant::now());
                            let _ = event_tx.send(AppEvent::EnrichJob(job));
                            let _ = event_tx.send(AppEvent::Activity(ActivityEntry {
                                at: Local::now(),
                                severity: Severity::Info,
                                message: format!("Enrich started ({id})"),
                            }));
                        }
                        Ok(EnrichCommand::RetryFailed) => {
                            for job in jobs.values_mut() {
                                if job.status == JobStatus::Failed {
                                    job.status = JobStatus::InProgress;
                                    job.started_at = Some(Local::now());
                                    job.finished_at = None;
                                    started_at.insert(job.id.clone(), Instant::now());
                                    let _ = event_tx.send(AppEvent::EnrichJob(job.clone()));
                                }
                            }
                        }
                        Ok(EnrichCommand::Cancel(id)) => {
                            if let Some(job) = jobs.get_mut(&id) {
                                job.status = JobStatus::Canceled;
                                job.finished_at = Some(Local::now());
                                started_at.remove(&id);
                                let _ = event_tx.send(AppEvent::EnrichJob(job.clone()));
                            }
                        }
                        Ok(EnrichCommand::Shutdown) | Err(_) => {
                            return;
                        }
                    }
                }
                recv(ticker) -> _ => {
                    for job in jobs.values_mut() {
                        if job.status == JobStatus::InProgress {
                            if started_at
                                .get(&job.id)
                                .map(|t| t.elapsed().as_secs() >= 2)
                                .unwrap_or(false)
                            {
                                job.status = JobStatus::Completed;
                                job.finished_at = Some(Local::now());
                                job.usage_estimate = Some("TODO".to_string());
                                started_at.remove(&job.id);
                                let _ = event_tx.send(AppEvent::Activity(ActivityEntry {
                                    at: Local::now(),
                                    severity: Severity::Success,
                                    message: format!("Enrich completed ({})", job.id),
                                }));
                            }
                            let _ = event_tx.send(AppEvent::EnrichJob(job.clone()));
                        }
                    }
                }
            }
        }
    })
}
