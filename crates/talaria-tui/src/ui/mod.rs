mod layout;
mod theme;

use std::path::Path;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{
    Block, Borders, Clear, Gauge, List, ListItem, Paragraph, Row, Table, TableState, Tabs, Wrap,
};

use crate::app::{AppState, AppTab};
use crate::storage;
use crate::types::Severity;

use self::layout::{centered_rect, main_chunks};
use self::theme::Theme;

pub fn draw(frame: &mut Frame, app: &mut AppState) {
    app.prune_toast();
    let theme = Theme::default();
    let chunks = main_chunks(frame.area());

    render_tabs(frame, app, &theme, chunks[0]);
    render_body(frame, app, &theme, chunks[1]);
    render_footer(frame, app, &theme, chunks[2]);

    if app.help_open {
        render_help(frame, &theme);
    }
    if app.picker.open {
        render_product_picker(frame, app, &theme);
    }
}

fn render_tabs(frame: &mut Frame, app: &AppState, theme: &Theme, area: Rect) {
    let titles = [
        " Home ",
        " Capture ",
        " Curate ",
        " Upload ",
        " Enrich ",
        " Products ",
        " Activity ",
        " Settings ",
    ]
    .iter()
    .map(|t| Line::from(*t))
    .collect::<Vec<_>>();

    let selected = app.active_tab as usize;
    let tabs = Tabs::new(titles)
        .select(selected)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Talaria Mission Control"),
        )
        .highlight_style(
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )
        .divider(" ");
    frame.render_widget(tabs, area);
}

fn render_body(frame: &mut Frame, app: &AppState, theme: &Theme, area: Rect) {
    match app.active_tab {
        AppTab::Home => render_home(frame, app, theme, area),
        AppTab::Capture => render_capture(frame, app, theme, area),
        AppTab::Curate => render_curate(frame, app, theme, area),
        AppTab::Upload => render_placeholder(frame, "Upload (TODO wiring)", area),
        AppTab::Enrich => render_placeholder(frame, "Enrich (TODO wiring)", area),
        AppTab::Products => render_products(frame, app, theme, area),
        AppTab::Activity => render_activity(frame, app, theme, area),
        AppTab::Settings => render_settings(frame, app, theme, area),
    }
}

fn render_home(frame: &mut Frame, app: &AppState, theme: &Theme, area: Rect) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(area);

    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(columns[0]);

    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(columns[1]);

    frame.render_widget(
        Paragraph::new(system_status_text(app))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("System Status"),
            )
            .wrap(Wrap { trim: true }),
        left[0],
    );

    let current_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(left[1]);

    frame.render_widget(
        Paragraph::new(current_focus_text(app))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Target + Session"),
            )
            .wrap(Wrap { trim: true }),
        current_chunks[0],
    );

    let progress = session_progress(app);
    frame.render_widget(
        Gauge::default()
            .block(Block::default().borders(Borders::ALL).title("Progress"))
            .gauge_style(Style::default().fg(theme.accent))
            .label(format!("{progress}%"))
            .percent(progress),
        current_chunks[1],
    );

    frame.render_widget(
        Paragraph::new(alerts_text(app))
            .block(Block::default().borders(Borders::ALL).title("Alerts"))
            .wrap(Wrap { trim: true }),
        right[0],
    );

    frame.render_widget(
        Paragraph::new("TODO: queue summaries, credits/usage, marketplace connections")
            .block(Block::default().borders(Borders::ALL).title("Pipeline"))
            .wrap(Wrap { trim: true }),
        right[1],
    );
}

fn render_capture(frame: &mut Frame, app: &AppState, theme: &Theme, area: Rect) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(36),
            Constraint::Percentage(34),
            Constraint::Percentage(30),
        ])
        .split(area);

    // Left column: why (target/session/actions)
    let left_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(40),
            Constraint::Percentage(35),
            Constraint::Percentage(25),
        ])
        .split(columns[0]);

    frame.render_widget(
        Paragraph::new(target_product_text(app))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Target Product"),
            )
            .wrap(Wrap { trim: true }),
        left_rows[0],
    );

    frame.render_widget(
        Paragraph::new(session_text(app))
            .block(Block::default().borders(Borders::ALL).title("Session"))
            .wrap(Wrap { trim: true }),
        left_rows[1],
    );

    frame.render_widget(
        Paragraph::new(actions_text_capture(app))
            .block(Block::default().borders(Borders::ALL).title("Actions"))
            .wrap(Wrap { trim: true }),
        left_rows[2],
    );

    // Middle column: how (camera controls)
    frame.render_widget(
        Paragraph::new(camera_controls_text(app))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Camera Controls"),
            )
            .wrap(Wrap { trim: true }),
        columns[1],
    );

    // Right column: what happened (stats + last result)
    let right_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(columns[2]);

    frame.render_widget(
        Paragraph::new(live_stats_text(app))
            .block(Block::default().borders(Borders::ALL).title("Live Stats"))
            .wrap(Wrap { trim: true }),
        right_rows[0],
    );

    frame.render_widget(
        Paragraph::new(last_result_text(app, theme))
            .block(Block::default().borders(Borders::ALL).title("Last Result"))
            .wrap(Wrap { trim: true }),
        right_rows[1],
    );
}

fn render_curate(frame: &mut Frame, app: &AppState, _theme: &Theme, area: Rect) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
        .split(area);

    let Some(session) = &app.active_session else {
        let empty = Paragraph::new(
            "No active session.\n\nStart a new product (n) or pick a product (Enter) to begin capturing.",
        )
        .block(Block::default().borders(Borders::ALL).title("Session Frames"))
        .wrap(Wrap { trim: true });
        frame.render_widget(empty, columns[0]);

        let hint = Paragraph::new("Keys:\n n new product\n Enter pick product")
            .block(Block::default().borders(Borders::ALL).title("Actions"))
            .wrap(Wrap { trim: true });
        frame.render_widget(hint, columns[1]);
        return;
    };

    let rows = session
        .frames
        .iter()
        .enumerate()
        .map(|(idx, f)| {
            let name = Path::new(&f.rel_path)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("frame.jpg");
            let sharp = f
                .sharpness_score
                .map(|s| format!("{s:.1}"))
                .unwrap_or_else(|| "n/a".to_string());
            let created = f.created_at.format("%H:%M:%S").to_string();
            Row::new(vec![format!("{idx:02}"), name.to_string(), sharp, created])
        })
        .collect::<Vec<_>>();

    let mut state = TableState::default();
    if !session.frames.is_empty() {
        state.select(Some(
            app.session_frame_selected.min(session.frames.len() - 1),
        ));
    }

    let table = Table::new(
        rows,
        [
            Constraint::Length(4),
            Constraint::Percentage(50),
            Constraint::Length(10),
            Constraint::Length(10),
        ],
    )
    .header(
        Row::new(vec!["#", "Filename", "Sharp", "Time"])
            .style(Style::default().add_modifier(Modifier::BOLD)),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title("Session Frames"),
    )
    .row_highlight_style(Style::default().add_modifier(Modifier::BOLD));

    frame.render_stateful_widget(table, columns[0], &mut state);

    frame.render_widget(
        Paragraph::new(curate_details_text(app, session))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Details + Actions"),
            )
            .wrap(Wrap { trim: true }),
        columns[1],
    );
}

fn render_products(frame: &mut Frame, app: &AppState, _theme: &Theme, area: Rect) {
    let sku = app
        .active_product
        .as_ref()
        .map(|p| p.sku_alias.as_str())
        .unwrap_or("none");
    let text = format!(
        "Active SKUs (Products)\n\nSelected: {sku}\n\nPress Enter from Capture to open product picker.\nTODO: richer product list view"
    );
    frame.render_widget(
        Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL).title("Products"))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn render_activity(frame: &mut Frame, app: &AppState, _theme: &Theme, area: Rect) {
    let items = app
        .activity
        .entries
        .iter()
        .rev()
        .take(200)
        .map(|entry| {
            let ts = entry.at.format("%H:%M:%S");
            let label = severity_label(entry.severity);
            ListItem::new(format!("[{ts}] {label} {}", entry.message))
        })
        .collect::<Vec<_>>();

    frame.render_widget(
        List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Activity"))
            .highlight_style(Style::default().add_modifier(Modifier::BOLD)),
        area,
    );
}

fn render_settings(frame: &mut Frame, app: &AppState, _theme: &Theme, area: Rect) {
    let stderr = app
        .stderr_log_path
        .as_ref()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "not redirected".to_string());
    let text = format!(
        "captures dir: {}\nlog stderr: {}\n\nTALARIA_CAPTURES_DIR overrides base capture path.\n\nTODO: show supabase bucket + Hermes base URL from config when wired",
        app.captures_dir.display(),
        stderr,
    );
    frame.render_widget(
        Paragraph::new(text)
            .block(Block::default().borders(Borders::ALL).title("Settings"))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn render_placeholder(frame: &mut Frame, title: &str, area: Rect) {
    frame.render_widget(
        Paragraph::new(title)
            .block(Block::default().borders(Borders::ALL).title(title))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn render_footer(frame: &mut Frame, app: &AppState, theme: &Theme, area: Rect) {
    let mut spans = Vec::new();
    spans.push(Span::styled(
        footer_hints(app),
        Style::default().fg(theme.subtle),
    ));
    if let Some(toast) = &app.toast {
        spans.push(Span::raw("  |  "));
        spans.push(Span::styled(
            &toast.message,
            toast_style(theme, toast.severity),
        ));
    }

    frame.render_widget(
        Paragraph::new(Line::from(spans))
            .block(Block::default().borders(Borders::ALL).title("Keys"))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn render_help(frame: &mut Frame, theme: &Theme) {
    let area = centered_rect(70, 70, frame.area());
    frame.render_widget(Clear, area);
    let text = [
        "Navigation:",
        "  ←/→: switch tabs",
        "  h/l: switch tabs (except Curate where h=hero)",
        "  1..8: jump to tab",
        "  ?: help",
        "  q: quit",
        "",
        "Capture (session-first):",
        "  n new product + session",
        "  Enter product picker",
        "  s stream | p preview | d/D device | c capture | b burst",
        "  x commit session | Esc abandon session",
        "",
        "Curate (session-first):",
        "  ↑/↓ select frame",
        "  h set hero pick | a add angle pick | d delete frame",
        "  x commit session",
    ]
    .join("\n");

    frame.render_widget(
        Paragraph::new(text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(Span::styled("Help", theme.title())),
            )
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn render_product_picker(frame: &mut Frame, app: &mut AppState, _theme: &Theme) {
    let area = centered_rect(80, 70, frame.area());
    frame.render_widget(Clear, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(2),
        ])
        .split(area);

    let header = Paragraph::new(format!("Search: {}", app.picker.search)).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Select Product"),
    );
    frame.render_widget(header, chunks[0]);

    let filtered = app.filtered_products();
    let rows = filtered
        .iter()
        .map(|p| {
            let name = p
                .display_name
                .clone()
                .unwrap_or_else(|| "(unnamed)".to_string());
            let updated = p.updated_at.format("%Y-%m-%d %H:%M").to_string();
            Row::new(vec![
                p.sku_alias.clone(),
                name,
                updated,
                p.image_count.to_string(),
            ])
        })
        .collect::<Vec<_>>();

    let mut state = TableState::default();
    if !filtered.is_empty() {
        state.select(Some(app.picker.selected.min(filtered.len() - 1)));
    }

    let table = Table::new(
        rows,
        [
            Constraint::Length(16),
            Constraint::Percentage(40),
            Constraint::Length(18),
            Constraint::Length(8),
        ],
    )
    .header(
        Row::new(vec!["SKU", "Name", "Updated", "Images"])
            .style(Style::default().add_modifier(Modifier::BOLD)),
    )
    .block(Block::default().borders(Borders::ALL).title("Products"))
    .row_highlight_style(Style::default().add_modifier(Modifier::BOLD));
    frame.render_stateful_widget(table, chunks[1], &mut state);

    let footer = Paragraph::new("Type to filter | ↑/↓ select | Enter choose | Esc cancel")
        .block(Block::default().borders(Borders::ALL));
    frame.render_widget(footer, chunks[2]);
}

fn system_status_text(app: &AppState) -> String {
    let camera = if app.camera_connected {
        "connected"
    } else {
        "disconnected"
    };
    let stream = if app.capture_status.streaming {
        "streaming"
    } else {
        "idle"
    };
    format!(
        "Camera: {camera}\nStream: {stream}  FPS: {:.1}  Dropped: {}\nPreview: {}\nCaptures: {}",
        app.capture_status.fps,
        app.capture_status.dropped_frames,
        if app.preview_enabled { "ON" } else { "OFF" },
        app.captures_dir.display()
    )
}

fn current_focus_text(app: &AppState) -> String {
    let product = app
        .active_product
        .as_ref()
        .map(|p| format!("{} ({})", p.sku_alias, p.product_id))
        .unwrap_or_else(|| "(new product)".to_string());
    let session = app
        .active_session
        .as_ref()
        .map(|s| s.session_id.clone())
        .unwrap_or_else(|| "none".to_string());
    format!("Product: {product}\nSession: {session}")
}

fn alerts_text(app: &AppState) -> String {
    let mut lines = Vec::new();
    for entry in app
        .activity
        .entries
        .iter()
        .rev()
        .filter(|e| matches!(e.severity, Severity::Warning | Severity::Error))
        .take(3)
    {
        lines.push(format!(
            "{}: {}",
            severity_label(entry.severity),
            entry.message
        ));
    }
    if lines.is_empty() {
        lines.push("No alerts".to_string());
    }
    lines.join("\n")
}

fn session_progress(app: &AppState) -> u16 {
    let Some(session) = &app.active_session else {
        return 0;
    };
    if session.committed_at.is_some() {
        return 100;
    }
    if session.picks.hero_rel_path.is_some() {
        return 70;
    }
    if !session.frames.is_empty() {
        return 40;
    }
    10
}

fn target_product_text(app: &AppState) -> String {
    let (sku, _product_id, display_name, image_count, product_path) = match &app.active_product {
        Some(p) => (
            p.sku_alias.clone(),
            p.product_id.clone(),
            p.display_name
                .clone()
                .unwrap_or_else(|| "(unnamed)".to_string()),
            p.images.len(),
            storage::product_dir(&app.captures_dir, &p.product_id),
        ),
        None => (
            "(new product)".to_string(),
            "-".to_string(),
            "-".to_string(),
            0,
            storage::products_dir(&app.captures_dir),
        ),
    };

    let stage = if app.active_session.is_some() {
        "Capturing/Curating"
    } else {
        "Idle"
    };

    format!(
        "SKU: {sku}\nName: {display_name}\nStage: {stage}\nImages: {image_count}\nDir: {}",
        shorten_path(&product_path, 36),
    )
}

fn session_text(app: &AppState) -> String {
    let Some(session) = &app.active_session else {
        return "Session: none\n\nStart capturing by creating a new product (n) or selecting one (Enter)."
            .to_string();
    };
    let frames_dir = storage::session_frames_dir(&app.captures_dir, &session.session_id);
    let picks_dir = storage::session_picks_dir(&app.captures_dir, &session.session_id);
    let uncommitted = session.committed_at.is_none()
        && (session.picks.hero_rel_path.is_some() || !session.picks.angle_rel_paths.is_empty());
    let warn = if uncommitted { "YES" } else { "no" };
    format!(
        "Session ID: {}\nFrames: {}\nPicks: hero={} angles={}\nFrames dir: {}\nPicks dir: {}\nUncommitted picks: {}",
        session.session_id,
        session.frames.len(),
        if session.picks.hero_rel_path.is_some() {
            "set"
        } else {
            "none"
        },
        session.picks.angle_rel_paths.len(),
        shorten_path(&frames_dir, 34),
        shorten_path(&picks_dir, 34),
        warn
    )
}

fn actions_text_capture(_app: &AppState) -> String {
    [
        "n = New product",
        "Enter = Select product…",
        "x = Commit session",
        "Esc = Abandon session",
    ]
    .join("\n")
}

fn camera_controls_text(app: &AppState) -> String {
    let resolution = app
        .capture_status
        .frame_size
        .map(|(w, h)| format!("{w}x{h}"))
        .unwrap_or_else(|| "n/a".to_string());
    format!(
        "Device: {} (name TODO)\nStream: {}\nPreview: {}\nResolution: {}\nBurst: {}\nROI: TODO\nExposure: TODO\nFocus: TODO",
        app.device_index,
        if app.capture_status.streaming {
            "ON"
        } else {
            "OFF"
        },
        if app.preview_enabled { "ON" } else { "OFF" },
        resolution,
        app.burst_count
    )
}

fn live_stats_text(app: &AppState) -> String {
    let resolution = app
        .capture_status
        .frame_size
        .map(|(w, h)| format!("{w}x{h}"))
        .unwrap_or_else(|| "n/a".to_string());
    format!(
        "FPS: {:.1}\nDropped: {}\nResolution: {}",
        app.capture_status.fps, app.capture_status.dropped_frames, resolution
    )
}

fn last_result_text(app: &AppState, _theme: &Theme) -> Text<'static> {
    let last_capture = app
        .last_capture_rel
        .as_ref()
        .map(|s| s.as_str())
        .unwrap_or("none");
    let last_commit = app
        .last_commit_message
        .as_ref()
        .map(|s| s.as_str())
        .unwrap_or("none");
    let hero = app
        .active_product
        .as_ref()
        .and_then(|p| p.hero_rel_path.as_ref())
        .map(|s| s.as_str())
        .unwrap_or("none");
    let err = app
        .last_error
        .as_ref()
        .map(|s| truncate(s, 80))
        .unwrap_or_else(|| "none".to_string());

    Text::from(vec![
        Line::from(format!("Last capture: {last_capture}")),
        Line::from(format!("Last commit: {last_commit}")),
        Line::from(format!("Hero: {hero}")),
        Line::from(format!("Last error: {err}")),
    ])
}

fn curate_details_text(app: &AppState, session: &storage::SessionManifest) -> String {
    let selected = session.frames.get(app.session_frame_selected);
    let selected_name = selected
        .map(|f| Path::new(&f.rel_path).display().to_string())
        .unwrap_or_else(|| "none".to_string());
    let sharp = selected
        .and_then(|f| f.sharpness_score)
        .map(|s| format!("{s:.1}"))
        .unwrap_or_else(|| "n/a".to_string());
    let hero = session.picks.hero_rel_path.as_deref().unwrap_or("none");
    format!(
        "Selected: {}\nSharpness: {}\nHero pick: {}\nAngle picks: {}\n\nActions:\n h hero pick\n a add angle\n d delete frame\n x commit session",
        selected_name,
        sharp,
        hero,
        session.picks.angle_rel_paths.len(),
    )
}

fn footer_hints(app: &AppState) -> String {
    let base = "←/→ tabs | 1..8 | ? help | q quit";
    match app.active_tab {
        AppTab::Capture => format!(
            "{base} | s start/stop | p preview | d/D device | c capture | b burst | n new | Enter pick | x commit | Esc abandon"
        ),
        AppTab::Curate => format!(
            "{base} | ↑/↓ select | h hero | a angle | d delete | x commit | n new | Enter pick"
        ),
        _ => base.to_string(),
    }
}

fn severity_label(sev: Severity) -> &'static str {
    match sev {
        Severity::Info => "INFO",
        Severity::Success => "OK",
        Severity::Warning => "WARN",
        Severity::Error => "ERR",
    }
}

fn toast_style(theme: &Theme, sev: Severity) -> Style {
    match sev {
        Severity::Info => theme.subtle(),
        Severity::Success => theme.ok(),
        Severity::Warning => theme.warn(),
        Severity::Error => theme.err(),
    }
}

fn shorten_path(path: &Path, max: usize) -> String {
    let s = path.to_string_lossy().to_string();
    if s.len() <= max {
        return s;
    }
    let keep = max.saturating_sub(3);
    format!("…{}", &s[s.len().saturating_sub(keep)..])
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    format!("{}…", &s[..max.saturating_sub(1)])
}
