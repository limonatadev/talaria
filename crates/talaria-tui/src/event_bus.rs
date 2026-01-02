use crossbeam_channel::{Receiver, Sender, unbounded};

use crate::types::{AppCommand, AppEvent};

pub struct EventBus {
    pub command_tx: Sender<AppCommand>,
    pub command_rx: Receiver<AppCommand>,
    pub event_tx: Sender<AppEvent>,
    pub event_rx: Receiver<AppEvent>,
}

impl EventBus {
    pub fn new() -> Self {
        let (command_tx, command_rx) = unbounded();
        let (event_tx, event_rx) = unbounded();
        Self {
            command_tx,
            command_rx,
            event_tx,
            event_rx,
        }
    }
}
