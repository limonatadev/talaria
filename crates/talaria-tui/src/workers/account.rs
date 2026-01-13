use std::thread;

use anyhow::Result;
use crossbeam_channel::{Receiver, Sender};
use tokio::runtime::Runtime;

use crate::types::{AccountCommand, AccountEvent, AppEvent, CreditsSnapshot};
use talaria_core::client::HermesClient;

pub fn spawn_account_worker(
    hermes: Option<HermesClient>,
    cmd_rx: Receiver<AccountCommand>,
    event_tx: Sender<AppEvent>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let rt = Runtime::new().expect("tokio runtime");

        loop {
            let cmd = match cmd_rx.recv() {
                Ok(cmd) => cmd,
                Err(_) => return,
            };

            if matches!(cmd, AccountCommand::Shutdown) {
                return;
            }

            let res: Result<()> = (|| match cmd {
                AccountCommand::FetchCredits => {
                    let Some(hermes) = hermes.as_ref().filter(|h| h.has_api_key()) else {
                        let _ = event_tx.send(AppEvent::Account(AccountEvent::CreditsError(
                            "Hermes API key missing.".to_string(),
                        )));
                        return Ok(());
                    };
                    let rows = rt.block_on(hermes.usage(None, None, None))?;
                    let Some(summary) = rows.first() else {
                        let _ = event_tx.send(AppEvent::Account(AccountEvent::CreditsError(
                            "No usage data returned.".to_string(),
                        )));
                        return Ok(());
                    };
                    let balance = summary
                        .tiered
                        .as_ref()
                        .map(|t| t.credit_balance_cents)
                        .unwrap_or(0);
                    let snapshot = CreditsSnapshot {
                        balance,
                        credits_used: summary.counters.credits_consumed,
                        listings_run: summary.counters.listings_run,
                        window_from: summary.window_from.map(|d| d.to_rfc3339()),
                        window_to: summary.window_to.map(|d| d.to_rfc3339()),
                    };
                    let _ = event_tx
                        .send(AppEvent::Account(AccountEvent::CreditsUpdated(snapshot)));
                    Ok(())
                }
                AccountCommand::Shutdown => Ok(()),
            })();

            if let Err(err) = res {
                let _ = event_tx.send(AppEvent::Account(AccountEvent::CreditsError(
                    err.to_string(),
                )));
            }
        }
    })
}
