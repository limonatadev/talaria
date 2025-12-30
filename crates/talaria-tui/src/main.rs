mod app;
mod camera;
mod event_bus;
mod preview;
mod types;
mod ui;
mod util;
mod workers;

use std::io;
use std::time::Duration;

use anyhow::Result;
use crossbeam_channel::unbounded;
use crossterm::cursor::Show;
use crossterm::event::{self, Event};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use camera::LatestFrameSlot;
use event_bus::EventBus;
use types::{AppCommand, AppEvent, CaptureCommand, PreviewCommand};

fn main() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let _guard = TerminalGuard;

    let bus = EventBus::new();
    let (capture_cmd_tx, capture_cmd_rx) = unbounded::<CaptureCommand>();
    let (preview_cmd_tx, preview_cmd_rx) = unbounded::<PreviewCommand>();
    let (upload_cmd_tx, upload_cmd_rx) = unbounded();
    let (enrich_cmd_tx, enrich_cmd_rx) = unbounded();
    let (listings_cmd_tx, listings_cmd_rx) = unbounded();

    let slot = LatestFrameSlot::shared();
    let capture_handle =
        camera::spawn_capture_thread(capture_cmd_rx, bus.event_tx.clone(), slot.clone());
    let preview_handle =
        preview::spawn_preview_thread(preview_cmd_rx, bus.event_tx.clone(), slot.clone());
    let upload_handle = workers::upload::spawn_upload_worker(upload_cmd_rx, bus.event_tx.clone());
    let enrich_handle = workers::enrich::spawn_enrich_worker(enrich_cmd_rx, bus.event_tx.clone());
    let listings_handle =
        workers::listings::spawn_listings_worker(listings_cmd_rx, bus.event_tx.clone());
    let router_handle = std::thread::spawn(move || {
        while let Ok(cmd) = bus.command_rx.recv() {
            match cmd {
                AppCommand::Capture(cmd) => {
                    let _ = capture_cmd_tx.send(cmd);
                }
                AppCommand::Preview(cmd) => {
                    let _ = preview_cmd_tx.send(cmd);
                }
                AppCommand::Upload(cmd) => {
                    let _ = upload_cmd_tx.send(cmd);
                }
                AppCommand::Enrich(cmd) => {
                    let _ = enrich_cmd_tx.send(cmd);
                }
                AppCommand::Listings(cmd) => {
                    let _ = listings_cmd_tx.send(cmd);
                }
                AppCommand::Shutdown => {
                    let _ = capture_cmd_tx.send(CaptureCommand::Shutdown);
                    let _ = preview_cmd_tx.send(PreviewCommand::Shutdown);
                    let _ = upload_cmd_tx.send(crate::types::UploadCommand::Shutdown);
                    let _ = enrich_cmd_tx.send(crate::types::EnrichCommand::Shutdown);
                    let _ = listings_cmd_tx.send(crate::types::ListingsCommand::Shutdown);
                    break;
                }
            }
        }
    });

    let mut app = app::AppState::new();
    let command_tx = bus.command_tx.clone();
    let res = run_app(&mut terminal, &mut app, bus.event_rx, command_tx);

    let _ = bus.command_tx.send(AppCommand::Shutdown);
    let _ = capture_handle.join();
    let _ = preview_handle.join();
    let _ = upload_handle.join();
    let _ = enrich_handle.join();
    let _ = listings_handle.join();
    let _ = router_handle.join();

    res
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut app::AppState,
    app_event_rx: crossbeam_channel::Receiver<AppEvent>,
    command_tx: crossbeam_channel::Sender<AppCommand>,
) -> Result<()> {
    loop {
        terminal.draw(|f| ui::draw(f, app))?;

        while let Ok(msg) = app_event_rx.try_recv() {
            app.apply_event(msg);
        }

        if event::poll(Duration::from_millis(50))?
            && let Event::Key(key) = event::read()?
        {
            app.handle_key(key, &command_tx);
        }

        if app.should_quit {
            break;
        }
    }
    Ok(())
}

struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, Show);
    }
}
