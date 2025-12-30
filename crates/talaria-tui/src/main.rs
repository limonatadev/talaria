mod camera;
mod preview;
mod types;
mod ui;
mod util;

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
use types::{CaptureCommand, PreviewCommand, UiEvent};

fn main() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let _guard = TerminalGuard;

    let (capture_cmd_tx, capture_cmd_rx) = unbounded::<CaptureCommand>();
    let (preview_cmd_tx, preview_cmd_rx) = unbounded::<PreviewCommand>();
    let (ui_event_tx, ui_event_rx) = unbounded::<UiEvent>();

    let slot = LatestFrameSlot::shared();
    let capture_handle =
        camera::spawn_capture_thread(capture_cmd_rx, ui_event_tx.clone(), slot.clone());
    let preview_handle =
        preview::spawn_preview_thread(preview_cmd_rx, ui_event_tx.clone(), slot.clone());

    let mut app = ui::AppState::new();
    let res = run_app(
        &mut terminal,
        &mut app,
        ui_event_rx,
        capture_cmd_tx.clone(),
        preview_cmd_tx.clone(),
    );

    capture_cmd_tx.send(CaptureCommand::Shutdown).ok();
    preview_cmd_tx.send(PreviewCommand::Shutdown).ok();
    let _ = capture_handle.join();
    let _ = preview_handle.join();

    res
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut ui::AppState,
    ui_event_rx: crossbeam_channel::Receiver<UiEvent>,
    capture_cmd_tx: crossbeam_channel::Sender<CaptureCommand>,
    preview_cmd_tx: crossbeam_channel::Sender<PreviewCommand>,
) -> Result<()> {
    loop {
        terminal.draw(|f| ui::draw(f, app))?;

        while let Ok(msg) = ui_event_rx.try_recv() {
            app.apply_event(msg);
        }

        if event::poll(Duration::from_millis(50))?
            && let Event::Key(key) = event::read()?
        {
            app.handle_key(key, &capture_cmd_tx, &preview_cmd_tx);
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
