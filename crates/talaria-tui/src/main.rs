mod app;
mod camera;
mod event_bus;
mod preview;
mod storage;
mod types;
mod ui;
mod util;
mod workers;

use std::io;
use std::time::Duration;

use anyhow::Result;
use crossbeam_channel::unbounded;
use crossterm::cursor::Show;
use crossterm::event::{self, Event, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use camera::LatestFrameSlot;
use event_bus::EventBus;
use talaria_core::client::HermesClient;
use talaria_core::config::{Config, EbaySettings};
use types::{AppCommand, AppEvent, CaptureCommand, PreviewCommand, StorageCommand};

fn main() -> Result<()> {
    let captures_dir = storage::default_captures_dir();
    storage::ensure_base_dirs(&captures_dir)?;
    let stderr_log = captures_dir.join("logs").join(format!(
        "talaria-tui-{}.stderr.log",
        chrono::Local::now().format("%Y%m%d_%H%M%S")
    ));
    let stderr_path = util::log_redirect::redirect_stderr_to_file(&stderr_log).ok();

    let mut startup_warnings = Vec::new();
    let mut config_info = app::ConfigInfo {
        preview_height_pct: talaria_core::config::DEFAULT_TUI_PREVIEW_HEIGHT_PCT,
        ..app::ConfigInfo::default()
    };
    let mut ebay_settings = EbaySettings::default();
    let hermes = match Config::load() {
        Ok(cfg) => {
            config_info.base_url = Some(cfg.base_url.clone());
            config_info.hermes_api_key_present = cfg.api_key.is_some();
            config_info.preview_height_pct = cfg
                .tui_preview_height_pct
                .unwrap_or(talaria_core::config::DEFAULT_TUI_PREVIEW_HEIGHT_PCT);
            ebay_settings = cfg.ebay.clone();
            if cfg.api_key.is_none() {
                startup_warnings.push(
                    "HERMES_API_KEY missing; online features disabled (filesystem-only mode)."
                        .to_string(),
                );
            }
            match HermesClient::new(cfg) {
                Ok(client) => {
                    config_info.online_ready = client.has_api_key();
                    Some(client)
                }
                Err(err) => {
                    startup_warnings.push(format!(
                        "Hermes client unavailable; online features disabled: {err}"
                    ));
                    None
                }
            }
        }
        Err(err) => {
            startup_warnings.push(format!("Config load failed (offline mode): {err}"));
            None
        }
    };

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let _guard = TerminalGuard;

    let terminal_preview = app::detect_terminal_preview();

    let bus = EventBus::new();
    let (capture_cmd_tx, capture_cmd_rx) = unbounded::<CaptureCommand>();
    let (preview_cmd_tx, preview_cmd_rx) = unbounded::<PreviewCommand>();
    let (upload_cmd_tx, upload_cmd_rx) = unbounded();
    let (storage_cmd_tx, storage_cmd_rx) = unbounded::<StorageCommand>();

    let slot = LatestFrameSlot::shared();
    let capture_handle =
        camera::spawn_capture_thread(capture_cmd_rx, bus.event_tx.clone(), slot.clone());
    let preview_handle = if terminal_preview.is_some() {
        None
    } else {
        Some(preview::spawn_preview_thread(
            preview_cmd_rx,
            bus.event_tx.clone(),
            slot.clone(),
        ))
    };
    let upload_handle = workers::upload::spawn_upload_worker(
        captures_dir.clone(),
        hermes.clone(),
        upload_cmd_rx,
        bus.event_tx.clone(),
    );
    let storage_handle = storage::worker::spawn_storage_worker(
        captures_dir.clone(),
        hermes.clone(),
        storage_cmd_rx,
        bus.event_tx.clone(),
    );

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
                AppCommand::Storage(cmd) => {
                    let _ = storage_cmd_tx.send(cmd);
                }
                AppCommand::Shutdown => {
                    let _ = capture_cmd_tx.send(CaptureCommand::Shutdown);
                    let _ = preview_cmd_tx.send(PreviewCommand::Shutdown);
                    let _ = upload_cmd_tx.send(crate::types::UploadCommand::Shutdown);
                    let _ = storage_cmd_tx.send(crate::types::StorageCommand::Shutdown);
                    break;
                }
            }
        }
    });

    let mut app = app::AppState::new(
        captures_dir,
        stderr_path,
        config_info,
        ebay_settings,
        startup_warnings,
        slot.clone(),
        terminal_preview,
    );
    let command_tx = bus.command_tx.clone();
    let res = run_app(&mut terminal, &mut app, bus.event_rx, command_tx);

    let _ = bus.command_tx.send(AppCommand::Shutdown);
    let _ = capture_handle.join();
    if let Some(handle) = preview_handle {
        let _ = handle.join();
    }
    let _ = upload_handle.join();
    let _ = storage_handle.join();
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
        for cmd in app.drain_pending_commands() {
            let _ = command_tx.send(cmd);
        }

        if event::poll(Duration::from_millis(50))?
            && let Event::Key(key) = event::read()?
        {
            if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
                app.handle_key(key, &command_tx);
                for cmd in app.drain_pending_commands() {
                    let _ = command_tx.send(cmd);
                }
            }
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
