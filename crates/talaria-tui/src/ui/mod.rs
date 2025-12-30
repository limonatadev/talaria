use chrono::{DateTime, Local};
use crossbeam_channel::Sender;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::types::{CaptureCommand, CaptureEvent, CaptureStatus, PreviewCommand, UiEvent};

pub struct AppState {
    pub should_quit: bool,
    pub help: bool,
    pub device_index: i32,
    pub preview_enabled: bool,
    pub burst_count: usize,
    pub status: CaptureStatus,
    pub last_capture_path: Option<String>,
    pub last_burst_best: Option<String>,
    pub last_error: Option<UiError>,
}

pub struct UiError {
    pub message: String,
    pub at: DateTime<Local>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            should_quit: false,
            help: false,
            device_index: 0,
            preview_enabled: false,
            burst_count: 10,
            status: CaptureStatus {
                streaming: false,
                device_index: 0,
                fps: 0.0,
                dropped_frames: 0,
                frame_size: None,
            },
            last_capture_path: None,
            last_burst_best: None,
            last_error: None,
        }
    }

    pub fn handle_key(
        &mut self,
        key: KeyEvent,
        capture_tx: &Sender<CaptureCommand>,
        preview_tx: &Sender<PreviewCommand>,
    ) {
        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('h') => self.help = !self.help,
            KeyCode::Char('s') => {
                if self.status.streaming {
                    let _ = capture_tx.send(CaptureCommand::StopStream);
                } else {
                    let _ = capture_tx.send(CaptureCommand::StartStream);
                }
            }
            KeyCode::Char('p') => {
                self.preview_enabled = !self.preview_enabled;
                let _ = preview_tx.send(PreviewCommand::SetEnabled(self.preview_enabled));
            }
            KeyCode::Char('d') => {
                self.device_index = (self.device_index - 1).max(0);
                let _ = capture_tx.send(CaptureCommand::SetDevice {
                    index: self.device_index,
                });
            }
            KeyCode::Char('D') => {
                self.device_index += 1;
                let _ = capture_tx.send(CaptureCommand::SetDevice {
                    index: self.device_index,
                });
            }
            KeyCode::Char('c') => {
                let _ = capture_tx.send(CaptureCommand::CaptureOne);
            }
            KeyCode::Char('b') => {
                let _ = capture_tx.send(CaptureCommand::CaptureBurst {
                    n: self.burst_count,
                });
            }
            _ => {}
        }
    }

    pub fn apply_event(&mut self, event: UiEvent) {
        match event {
            UiEvent::Capture(capture_event) => self.apply_capture_event(capture_event),
            UiEvent::Preview(preview_event) => self.apply_preview_event(preview_event),
        }
    }

    fn apply_capture_event(&mut self, event: CaptureEvent) {
        match event {
            CaptureEvent::Status(status) => {
                let device_index = status.device_index;
                self.status = status;
                self.device_index = device_index;
            }
            CaptureEvent::Error(message) => {
                self.last_error = Some(UiError {
                    message,
                    at: Local::now(),
                });
            }
            CaptureEvent::CaptureCompleted { path } => {
                self.last_capture_path = Some(path);
            }
            CaptureEvent::BurstCompleted { best_path, .. } => {
                self.last_burst_best = Some(best_path);
            }
        }
    }

    fn apply_preview_event(&mut self, event: crate::types::PreviewEvent) {
        match event {
            crate::types::PreviewEvent::Error(message) => {
                self.last_error = Some(UiError {
                    message,
                    at: Local::now(),
                });
            }
            crate::types::PreviewEvent::RoiSelected(_) => {
                // TODO: forward ROI selection to capture thread.
            }
        }
    }
}

pub fn draw(frame: &mut Frame, app: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),
            Constraint::Min(5),
            Constraint::Length(5),
        ])
        .split(frame.area());

    let status = render_status(app);
    frame.render_widget(status, chunks[0]);

    let info = render_info(app);
    frame.render_widget(info, chunks[1]);

    let footer = render_footer(app);
    frame.render_widget(footer, chunks[2]);
}

fn render_status(app: &AppState) -> Paragraph<'_> {
    let stream_label = if app.status.streaming { "ON" } else { "OFF" };
    let preview_label = if app.preview_enabled { "ON" } else { "OFF" };
    let resolution = app
        .status
        .frame_size
        .map(|(w, h)| format!("{w}x{h}"))
        .unwrap_or_else(|| "n/a".to_string());

    let text = format!(
        "Device: {} (name TODO)\nStream: {}  Preview: {}\nFPS: {:.1}  Dropped: {}\nResolution: {}",
        app.device_index,
        stream_label,
        preview_label,
        app.status.fps,
        app.status.dropped_frames,
        resolution
    );

    Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title("Status"))
        .wrap(Wrap { trim: true })
}

fn render_info(app: &AppState) -> Paragraph<'_> {
    let mut lines = Vec::new();
    lines.push("Captures are saved to ./captures.".to_string());
    lines.push(format!("Burst count: {}", app.burst_count));
    if let Some(path) = &app.last_capture_path {
        lines.push(format!("Last capture: {path}"));
    }
    if let Some(path) = &app.last_burst_best {
        lines.push(format!("Best burst frame: {path}"));
    }
    if let Some(error) = &app.last_error {
        lines.push(format!(
            "Last error: {} ({})",
            error.message,
            error.at.format("%H:%M:%S")
        ));
    }
    lines.push("TODO: upload saved images to Supabase bucket".to_string());
    lines.push("TODO: call Hermes API with saved images".to_string());

    Paragraph::new(lines.join("\n"))
        .block(Block::default().borders(Borders::ALL).title("Info"))
        .wrap(Wrap { trim: true })
}

fn render_footer(app: &AppState) -> Paragraph<'_> {
    let mut text = String::new();
    if app.help {
        text.push_str(
            "q quit | s start/stop | p preview | d/D device | c capture | b burst | h help\n",
        );
        text.push_str("Preview runs in an OpenCV window; close it or press p to stop.");
    } else {
        text.push_str("Press h for help.");
    }

    Paragraph::new(text)
        .block(
            Block::default().borders(Borders::ALL).title("Keys").style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        )
        .wrap(Wrap { trim: true })
}
