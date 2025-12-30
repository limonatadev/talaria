mod layout;
mod theme;

use std::path::Path;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Clear, Gauge, List, ListItem, Paragraph, Row, Table, TableState, Tabs, Wrap,
};

use crate::app::{ActivityFilter, AppState, AppTab};
use crate::types::{JobStatus, PipelineStage, Severity};

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
}

fn render_tabs(frame: &mut Frame, app: &AppState, theme: &Theme, area: Rect) {
    let titles = [
        " Home ",
        " Capture ",
        " Curate ",
        " Upload ",
        " Enrich ",
        " Listings ",
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
        AppTab::Upload => render_upload(frame, app, theme, area),
        AppTab::Enrich => render_enrich(frame, app, theme, area),
        AppTab::Listings => render_listings(frame, app, theme, area),
        AppTab::Activity => render_activity(frame, app, theme, area),
        AppTab::Settings => render_settings(frame, app, theme, area),
    }
}

fn render_home(frame: &mut Frame, app: &AppState, theme: &Theme, area: Rect) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(columns[0]);

    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(columns[1]);

    let system_status = Paragraph::new(system_status_text(app))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("System Status"),
        )
        .wrap(Wrap { trim: true });
    frame.render_widget(system_status, left[0]);

    let current_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(left[1]);
    let current_item = render_current_item(app);
    frame.render_widget(current_item, current_chunks[0]);
    let progress = stage_progress(app.current_item.stage);
    let gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title("Pipeline"))
        .gauge_style(Style::default().fg(theme.accent))
        .label(format!("{progress}%"))
        .percent(progress);
    frame.render_widget(gauge, current_chunks[1]);

    let metrics = Paragraph::new(today_metrics_text(app))
        .block(Block::default().borders(Borders::ALL).title("Today"))
        .wrap(Wrap { trim: true });
    frame.render_widget(metrics, right[0]);

    let alerts = Paragraph::new(alerts_text(app))
        .block(Block::default().borders(Borders::ALL).title("Alerts"))
        .wrap(Wrap { trim: true });
    frame.render_widget(alerts, right[1]);
}

fn render_current_item(app: &AppState) -> Paragraph<'static> {
    let stage = stage_label(app.current_item.stage);
    let hero = app
        .current_item
        .selected_hero
        .as_ref()
        .and_then(|p| p.file_name())
        .and_then(|f| f.to_str())
        .unwrap_or("none");
    let uploaded = app
        .current_item
        .uploaded_images
        .first()
        .map(|img| img.url.as_str())
        .unwrap_or("none");

    let mut lines = Vec::new();
    lines.push(format!("Item: {}", app.current_item.id));
    lines.push(format!("Stage: {stage}"));
    lines.push(format!("Hero: {hero}"));
    lines.push(format!("Uploaded: {uploaded}"));
    lines.push("TODO: Hermes enrichment + listing summary".to_string());

    let text = lines.join("\n");
    Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title("Current Item"))
        .wrap(Wrap { trim: true })
        .wrap(Wrap { trim: true })
}

fn render_capture(frame: &mut Frame, app: &AppState, _theme: &Theme, area: Rect) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(area);

    let controls = Paragraph::new(capture_controls_text(app))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Camera Controls"),
        )
        .wrap(Wrap { trim: true });
    frame.render_widget(controls, columns[0]);

    let stats = Paragraph::new(capture_stats_text(app))
        .block(Block::default().borders(Borders::ALL).title("Live Stats"))
        .wrap(Wrap { trim: true });
    frame.render_widget(stats, columns[1]);
}

fn render_curate(frame: &mut Frame, app: &AppState, _theme: &Theme, area: Rect) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
        .split(area);

    let rows = app
        .current_item
        .local_images
        .iter()
        .enumerate()
        .map(|(idx, img)| {
            let name = img
                .path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown");
            let sharp = img
                .sharpness_score
                .map(|s| format!("{s:.2}"))
                .unwrap_or_else(|| "n/a".to_string());
            let created = img.created_at.format("%H:%M:%S").to_string();
            Row::new(vec![format!("{idx:02}"), name.to_string(), sharp, created])
        })
        .collect::<Vec<_>>();

    let mut state = TableState::default();
    if !app.current_item.local_images.is_empty() {
        state.select(Some(
            app.local_image_selected
                .min(app.current_item.local_images.len() - 1),
        ));
    }

    let table = Table::new(
        rows,
        [
            Constraint::Length(4),
            Constraint::Percentage(50),
            Constraint::Length(8),
            Constraint::Length(10),
        ],
    )
    .header(
        Row::new(vec!["#", "Image", "Sharp", "Time"])
            .style(Style::default().add_modifier(Modifier::BOLD)),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title("Captured Frames"),
    )
    .row_highlight_style(Style::default().add_modifier(Modifier::BOLD));

    frame.render_stateful_widget(table, columns[0], &mut state);

    let quality = Paragraph::new(curate_quality_text(app))
        .block(Block::default().borders(Borders::ALL).title("Quality"))
        .wrap(Wrap { trim: true });
    frame.render_widget(quality, columns[1]);
}

fn render_upload(frame: &mut Frame, app: &AppState, _theme: &Theme, area: Rect) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(area);

    let rows = app
        .uploads
        .iter()
        .map(|job| {
            let status = job_status_label(job.status);
            let progress = format!("{:.0}%", job.progress * 100.0);
            Row::new(vec![
                job.id.clone(),
                short_path(&job.path),
                status,
                progress,
                job.retries.to_string(),
                job.last_error.clone().unwrap_or_else(|| "-".to_string()),
            ])
        })
        .collect::<Vec<_>>();

    let mut state = TableState::default();
    if !app.uploads.is_empty() {
        state.select(Some(app.upload_selected.min(app.uploads.len() - 1)));
    }

    let table = Table::new(
        rows,
        [
            Constraint::Length(16),
            Constraint::Percentage(30),
            Constraint::Length(10),
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Percentage(30),
        ],
    )
    .header(
        Row::new(vec!["ID", "Path", "Status", "Prog", "Retry", "Error"])
            .style(Style::default().add_modifier(Modifier::BOLD)),
    )
    .block(Block::default().borders(Borders::ALL).title("Upload Queue"))
    .row_highlight_style(Style::default().add_modifier(Modifier::BOLD));

    frame.render_stateful_widget(table, columns[0], &mut state);

    let summary = Paragraph::new(upload_summary_text(app))
        .block(Block::default().borders(Borders::ALL).title("Actions"))
        .wrap(Wrap { trim: true });
    frame.render_widget(summary, columns[1]);
}

fn render_enrich(frame: &mut Frame, app: &AppState, _theme: &Theme, area: Rect) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(area);

    let rows = app
        .enrich_jobs
        .iter()
        .map(|job| {
            let status = job_status_label(job.status);
            let started = job
                .started_at
                .map(|t| t.format("%H:%M:%S").to_string())
                .unwrap_or_else(|| "-".to_string());
            let finished = job
                .finished_at
                .map(|t| t.format("%H:%M:%S").to_string())
                .unwrap_or_else(|| "-".to_string());
            Row::new(vec![job.id.clone(), status, started, finished])
        })
        .collect::<Vec<_>>();

    let mut state = TableState::default();
    if !app.enrich_jobs.is_empty() {
        state.select(Some(app.enrich_selected.min(app.enrich_jobs.len() - 1)));
    }

    let table = Table::new(
        rows,
        [
            Constraint::Length(16),
            Constraint::Length(12),
            Constraint::Length(10),
            Constraint::Length(10),
        ],
    )
    .header(
        Row::new(vec!["ID", "Status", "Start", "Done"])
            .style(Style::default().add_modifier(Modifier::BOLD)),
    )
    .block(Block::default().borders(Borders::ALL).title("Enrich Jobs"))
    .row_highlight_style(Style::default().add_modifier(Modifier::BOLD));

    frame.render_stateful_widget(table, columns[0], &mut state);

    let details = Paragraph::new(enrich_details_text(app))
        .block(Block::default().borders(Borders::ALL).title("Summary"))
        .wrap(Wrap { trim: true });
    frame.render_widget(details, columns[1]);
}

fn render_listings(frame: &mut Frame, app: &AppState, _theme: &Theme, area: Rect) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(area);

    let rows = app
        .listing_drafts
        .iter()
        .map(|draft| {
            let status = job_status_label(draft.status);
            Row::new(vec![
                draft.id.clone(),
                draft.marketplace.clone(),
                status,
                draft.last_error.clone().unwrap_or_else(|| "-".to_string()),
            ])
        })
        .collect::<Vec<_>>();

    let mut state = TableState::default();
    if !app.listing_drafts.is_empty() {
        state.select(Some(app.listing_selected.min(app.listing_drafts.len() - 1)));
    }

    let table = Table::new(
        rows,
        [
            Constraint::Length(16),
            Constraint::Length(16),
            Constraint::Length(12),
            Constraint::Percentage(40),
        ],
    )
    .header(
        Row::new(vec!["ID", "Market", "Status", "Error"])
            .style(Style::default().add_modifier(Modifier::BOLD)),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title("Draft Listings"),
    )
    .row_highlight_style(Style::default().add_modifier(Modifier::BOLD));

    frame.render_stateful_widget(table, columns[0], &mut state);

    let details = Paragraph::new("TODO: validation issues + listing preview")
        .block(Block::default().borders(Borders::ALL).title("Validation"))
        .wrap(Wrap { trim: true });
    frame.render_widget(details, columns[1]);
}

fn render_activity(frame: &mut Frame, app: &AppState, _theme: &Theme, area: Rect) {
    let items = app
        .activity
        .entries
        .iter()
        .rev()
        .filter(|entry| match app.activity_filter {
            ActivityFilter::All => true,
            ActivityFilter::Info => entry.severity == Severity::Info,
            ActivityFilter::Success => entry.severity == Severity::Success,
            ActivityFilter::Warning => entry.severity == Severity::Warning,
            ActivityFilter::Error => entry.severity == Severity::Error,
        })
        .take(120)
        .map(|entry| {
            let ts = entry.at.format("%H:%M:%S");
            let label = severity_label(entry.severity);
            ListItem::new(format!("[{ts}] {label} {}", entry.message))
        })
        .collect::<Vec<_>>();

    let title = format!(
        "Activity (filter: {})",
        match app.activity_filter {
            ActivityFilter::All => "all",
            ActivityFilter::Info => "info",
            ActivityFilter::Success => "success",
            ActivityFilter::Warning => "warn",
            ActivityFilter::Error => "error",
        }
    );

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));
    frame.render_widget(list, area);
}

fn render_settings(frame: &mut Frame, app: &AppState, _theme: &Theme, area: Rect) {
    let text = format!(
        "Capture dir: {}\nBurst default: {}\nSupabase bucket: {}\nHermes base URL: {}\n\nTODO: editable settings form",
        app.config_view.capture_dir,
        app.config_view.burst_default,
        app.config_view.supabase_bucket,
        app.config_view.api_base_url
    );
    let panel = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL).title("Settings"))
        .wrap(Wrap { trim: true });
    frame.render_widget(panel, area);
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

    let footer = Paragraph::new(Line::from(spans))
        .block(Block::default().borders(Borders::ALL).title("Keys"))
        .wrap(Wrap { trim: true });
    frame.render_widget(footer, area);
}

fn render_help(frame: &mut Frame, theme: &Theme) {
    let area = centered_rect(70, 70, frame.area());
    frame.render_widget(Clear, area);
    let text = [
        "Navigation:",
        "  ←/→ or h/l: switch tabs",
        "  1..8: jump to tab",
        "  ?: help",
        "  q: quit",
        "",
        "Capture:",
        "  s start/stop | p preview | d/D device | c capture | b burst",
        "Curate:",
        "  ↑/↓ select | Enter set hero | d delete | r rename (TODO)",
        "Upload:",
        "  e enqueue hero | a enqueue all | r retry | x cancel",
        "Enrich:",
        "  e enqueue | r retry | x cancel",
        "Listings:",
        "  c create draft | p push live | e export JSON",
        "Activity:",
        "  f filter severity | / search (TODO)",
    ]
    .join("\n");

    let panel = Paragraph::new(text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(Span::styled("Help", theme.title())),
        )
        .wrap(Wrap { trim: true });
    frame.render_widget(panel, area);
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
    let upload_counts = upload_counts(app);
    let hermes = format!("{} jobs", app.enrich_jobs.len());

    format!(
        "Camera: {camera} (dev {})\nStream: {stream}  FPS: {:.1}\nUpload queue: {} pending / {} active / {} failed\nHermes: {hermes}\nMarketplace: eBay: TODO\nCredits: TODO",
        app.device_index, app.capture_status.fps, upload_counts.0, upload_counts.1, upload_counts.2,
    )
}

fn today_metrics_text(app: &AppState) -> String {
    format!(
        "Captured: {}\nEnriched: {}\nListed: {}",
        app.metrics.captured_today, app.metrics.enriched_today, app.metrics.listed_today
    )
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

fn capture_controls_text(app: &AppState) -> String {
    let preview = if app.preview_enabled { "ON" } else { "OFF" };
    let stream = if app.capture_status.streaming {
        "ON"
    } else {
        "OFF"
    };
    format!(
        "Device: {} (name TODO)\nStream: {stream}\nPreview: {preview}\nBurst: {}\nROI: TODO\nExposure: TODO\nFocus: TODO",
        app.device_index, app.burst_count
    )
}

fn capture_stats_text(app: &AppState) -> String {
    let resolution = app
        .capture_status
        .frame_size
        .map(|(w, h)| format!("{w}x{h}"))
        .unwrap_or_else(|| "n/a".to_string());
    let last = app
        .last_capture_path
        .as_ref()
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
        .unwrap_or("none");
    let hero = app
        .current_item
        .selected_hero
        .as_ref()
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
        .unwrap_or("none");
    format!(
        "FPS: {:.1}\nDropped: {}\nResolution: {}\nLast capture: {}\nHero: {}",
        app.capture_status.fps, app.capture_status.dropped_frames, resolution, last, hero
    )
}

fn curate_quality_text(app: &AppState) -> String {
    let hero = app
        .current_item
        .selected_hero
        .as_ref()
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
        .unwrap_or("none");
    let angles = app.current_item.local_images.len();
    format!("Hero: {hero}\nAngles: {angles}\nQuality: TODO")
}

fn upload_summary_text(app: &AppState) -> String {
    let (pending, active, failed) = upload_counts(app);
    format!(
        "Pending: {pending}\nActive: {active}\nFailed: {failed}\n\nActions:\n e enqueue hero\n a enqueue all\n r retry failed\n x cancel selected\n\nTODO: Supabase upload wiring",
    )
}

fn enrich_details_text(app: &AppState) -> String {
    let selected = app.enrich_jobs.get(app.enrich_selected);
    if let Some(job) = selected {
        let urls = job
            .image_urls
            .iter()
            .take(3)
            .map(|u| format!("- {u}"))
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "Job: {}\nStatus: {}\nUsage: {}\nURLs:\n{}\n\nTODO: Hermes enrich wiring",
            job.id,
            job_status_label(job.status),
            job.usage_estimate
                .clone()
                .unwrap_or_else(|| "TODO".to_string()),
            urls
        )
    } else {
        "No job selected.\n\nTODO: Hermes enrich wiring".to_string()
    }
}

fn footer_hints(app: &AppState) -> String {
    let base = "←/→ tabs | 1..8 jump | ? help | q quit";
    let extra = match app.active_tab {
        AppTab::Capture => " | s start/stop | p preview | d/D device | c capture | b burst",
        AppTab::Curate => " | ↑/↓ select | Enter hero | d delete | r rename",
        AppTab::Upload => " | e hero | a all | r retry | x cancel",
        AppTab::Enrich => " | e enqueue | r retry | x cancel",
        AppTab::Listings => " | c draft | p push | e export",
        AppTab::Activity => " | f filter | / search",
        _ => "",
    };
    format!("{base}{extra}")
}

fn upload_counts(app: &AppState) -> (usize, usize, usize) {
    let mut pending = 0;
    let mut active = 0;
    let mut failed = 0;
    for job in &app.uploads {
        match job.status {
            JobStatus::Pending => pending += 1,
            JobStatus::InProgress => active += 1,
            JobStatus::Failed => failed += 1,
            _ => {}
        }
    }
    (pending, active, failed)
}

fn job_status_label(status: JobStatus) -> String {
    match status {
        JobStatus::Pending => "pending",
        JobStatus::InProgress => "active",
        JobStatus::Completed => "done",
        JobStatus::Failed => "failed",
        JobStatus::Canceled => "canceled",
    }
    .to_string()
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

fn stage_label(stage: PipelineStage) -> &'static str {
    match stage {
        PipelineStage::Captured => "Captured",
        PipelineStage::Curated => "Curated",
        PipelineStage::Uploaded => "Uploaded",
        PipelineStage::Enriched => "Enriched",
        PipelineStage::ReadyToList => "Ready",
        PipelineStage::Listed => "Listed",
        PipelineStage::Error => "Error",
    }
}

fn stage_progress(stage: PipelineStage) -> u16 {
    match stage {
        PipelineStage::Captured => 20,
        PipelineStage::Curated => 40,
        PipelineStage::Uploaded => 60,
        PipelineStage::Enriched => 80,
        PipelineStage::ReadyToList => 90,
        PipelineStage::Listed => 100,
        PipelineStage::Error => 100,
    }
}

fn short_path(path: &Path) -> String {
    path.file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string()
}
