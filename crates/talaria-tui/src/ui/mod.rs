mod layout;
mod theme;

use std::path::Path;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Clear, Gauge, List, ListItem, ListState, Paragraph, Row, Table,
    TableState, Tabs, Wrap,
};
use ratatui_image::StatefulImage;
use ratatui_image::protocol::StatefulProtocol;
use serde_json::Value;

use crate::app::{
    AppState, AppTab, ListingFieldKey, PREVIEW_HEIGHT_MAX_PCT, PREVIEW_HEIGHT_MIN_PCT,
    PackageDimensionKey, SettingsField,
};
use crate::types::Severity;

use self::layout::{centered_rect, main_chunks};
use self::theme::Theme;

pub fn draw(frame: &mut Frame, app: &mut AppState) {
    app.update_terminal_preview();
    app.prune_toast();
    let theme = Theme::default();
    frame.render_widget(Block::default().style(theme.base()), frame.area());
    let chunks = main_chunks(frame.area());

    render_tabs(frame, app, &theme, chunks[0]);
    render_body(frame, app, &theme, chunks[1]);
    render_footer(frame, app, &theme, chunks[2]);

    if app.help_open {
        render_help(frame, &theme);
    }
    if app.camera_picker.open {
        render_camera_picker(frame, app, &theme);
    }
    if app.picker.open {
        render_product_picker(frame, app, &theme);
    }
    if app.settings_picker.open {
        render_settings_picker(frame, app, &theme);
    }
}

fn render_tabs(frame: &mut Frame, app: &AppState, theme: &Theme, area: Rect) {
    let titles = [
        " Home ",
        " Quickstart ",
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
            theme
                .panel_block()
                .title(Span::styled("Talaria Mission Control", theme.title())),
        )
        .style(theme.panel())
        .highlight_style(theme.title())
        .divider(" ");
    frame.render_widget(tabs, area);
}

fn render_body(frame: &mut Frame, app: &mut AppState, theme: &Theme, area: Rect) {
    match app.active_tab {
        AppTab::Home => render_home(frame, app, theme, area),
        AppTab::Quickstart => render_quickstart(frame, app, theme, area),
        AppTab::Products => render_products(frame, app, theme, area),
        AppTab::Activity => render_activity(frame, app, theme, area),
        AppTab::Settings => render_settings(frame, app, theme, area),
    }
}

fn render_quickstart(frame: &mut Frame, app: &AppState, theme: &Theme, area: Rect) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    let left = [
        "Quickstart",
        "",
        "1) Create a product",
        "   - Shift+Tab to Products, press n",
        "   - Capture images, then Shift+S to save",
        "",
        "2) Generate structure (HSUF)",
        "   - Tab to Structure, press r",
        "   - Enter to edit any field",
        "",
        "3) Generate listing",
        "   - Set Settings (policies + location)",
        "   - From Context: r (structure), p (draft pipeline), P (publish pipeline)",
        "   - From Structure: g (full listing)",
        "   - Or Tab to Listings: g (full), p (draft), P (publish draft)",
        "",
        "4) Sync + refresh",
        "   - Shift+S syncs product data + media",
    ]
    .join("\n");

    let right = [
        "Hotkeys",
        "",
        "Shift+Tab: next main tab",
        "Tab: Context/Structure/Listings",
        "Shift+S: save + sync",
        "G: back to grid",
        "E: edit full JSON",
        "?: help",
        "q: quit",
        "",
        "Tips",
        "",
        "Use the Structure tab to edit",
        "details before listing.",
        "Listings can be updated any time",
        "from the Listings panel.",
    ]
    .join("\n");

    let spinner = if app.product_syncing
        || app.structure_inference
        || app.listing_inference
        || app.products_loading
    {
        format!(" {}", app.spinner_frame())
    } else {
        String::new()
    };
    let status = format!("Quickstart{spinner}");

    frame.render_widget(
        Paragraph::new(left)
            .style(theme.panel())
            .block(panel_title(theme, &status))
            .wrap(Wrap { trim: true }),
        columns[0],
    );
    frame.render_widget(
        Paragraph::new(right)
            .style(theme.panel())
            .block(panel_title(theme, "Notes"))
            .wrap(Wrap { trim: true }),
        columns[1],
    );
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

    let palette = mondrian_palette();
    let mut idx = 0usize;
    let sys_style = next_style(&palette, &mut idx);
    let focus_style = next_style(&palette, &mut idx);
    let progress_style = next_style(&palette, &mut idx);
    let alerts_style = next_style(&palette, &mut idx);
    let pipeline_style = next_style(&palette, &mut idx);

    frame.render_widget(
        Paragraph::new(system_status_text(app))
            .style(mondrian_style(sys_style))
            .block(mondrian_block(theme, "System Status", sys_style))
            .wrap(Wrap { trim: true }),
        left[0],
    );

    let current_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(left[1]);

    frame.render_widget(
        Paragraph::new(current_focus_text(app))
            .style(mondrian_style(focus_style))
            .block(mondrian_block(theme, "Target + Session", focus_style))
            .wrap(Wrap { trim: true }),
        current_chunks[0],
    );

    let progress = session_progress(app);
    frame.render_widget(
        Gauge::default()
            .block(mondrian_block(theme, "Progress", progress_style))
            .style(mondrian_style(progress_style))
            .gauge_style(mondrian_style(progress_style))
            .label(format!("{progress}%"))
            .percent(progress),
        current_chunks[1],
    );

    frame.render_widget(
        Paragraph::new(alerts_text(app))
            .style(mondrian_style(alerts_style))
            .block(mondrian_block(theme, "Alerts", alerts_style))
            .wrap(Wrap { trim: true }),
        right[0],
    );

    frame.render_widget(
        Paragraph::new(pipeline_text(app))
            .style(mondrian_style(pipeline_style))
            .block(mondrian_block(theme, "Pipeline", pipeline_style))
            .wrap(Wrap { trim: true }),
        right[1],
    );
}

fn render_products(frame: &mut Frame, app: &mut AppState, theme: &Theme, area: Rect) {
    match app.products_mode {
        crate::app::ProductsMode::Grid => render_products_grid(frame, app, theme, area),
        crate::app::ProductsMode::Workspace => render_products_workspace(frame, app, theme, area),
    }
}

fn render_products_grid(frame: &mut Frame, app: &mut AppState, theme: &Theme, area: Rect) {
    let palette = mondrian_palette();
    let mut idx = 0usize;
    let header_style = next_style(&palette, &mut idx);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Min(6)])
        .split(area);

    let spinner = if app.products_loading {
        format!(" {}", app.spinner_frame())
    } else {
        String::new()
    };
    let header_text = format!(
        "Products{spinner}: n = new product | Enter = select product | d = delete (y confirm) | arrows = move"
    );
    frame.render_widget(
        Paragraph::new(header_text)
            .style(mondrian_style(header_style))
            .block(mondrian_block(theme, "Products", header_style))
            .wrap(Wrap { trim: true }),
        chunks[0],
    );

    if app.picker.products.is_empty() {
        let empty_style = next_style(&palette, &mut idx);
        let body = if app.products_loading {
            format!("Loading products {}...", app.spinner_frame())
        } else {
            "No products yet.\n\nPress n to create your first product.".to_string()
        };
        frame.render_widget(
            Paragraph::new(body)
                .style(mondrian_style(empty_style))
                .block(mondrian_block(theme, "Product Grid", empty_style))
                .wrap(Wrap { trim: true }),
            chunks[1],
        );
        return;
    }

    let grid_area = chunks[1];
    let min_cell_width = 26u16;
    let mut cols = (grid_area.width / min_cell_width).max(1) as usize;
    cols = cols.min(4).max(1);
    app.product_grid_cols = cols;

    if app.product_grid_selected >= app.picker.products.len() {
        app.product_grid_selected = 0;
    }

    let rows = (app.picker.products.len() + cols - 1) / cols;
    let row_constraints = (0..rows)
        .map(|_| Constraint::Ratio(1, rows as u32))
        .collect::<Vec<_>>();
    let row_areas = Layout::default()
        .direction(Direction::Vertical)
        .spacing(1)
        .constraints(row_constraints)
        .split(grid_area);

    for (row_idx, row_area) in row_areas.iter().enumerate() {
        let col_constraints = (0..cols)
            .map(|_| Constraint::Ratio(1, cols as u32))
            .collect::<Vec<_>>();
        let col_areas = Layout::default()
            .direction(Direction::Horizontal)
            .spacing(1)
            .constraints(col_constraints)
            .split(*row_area);

        for (col_idx, cell) in col_areas.iter().enumerate() {
            let product_index = row_idx * cols + col_idx;
            if product_index >= app.picker.products.len() {
                continue;
            }
            let product = &app.picker.products[product_index];
            let style = palette[(product_index + idx) % palette.len()];
            let selected = product_index == app.product_grid_selected;
            let title = format!("{}", product.sku_alias);
            let name = product
                .display_name
                .clone()
                .unwrap_or_else(|| "(unnamed)".to_string());
            let updated = product.updated_at.format("%Y-%m-%d").to_string();
            let status_line = format_product_status(product);
            let text = format!(
                "{}\nStatus: {}\nImages: {}\nUpdated: {}",
                truncate(&name, 24),
                status_line,
                product.image_count,
                updated
            );
            let mut block = mondrian_block(theme, &title, style);
            if selected {
                block = block.border_style(
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::BOLD),
                );
            }
            frame.render_widget(
                Paragraph::new(text)
                    .style(mondrian_style(style))
                    .block(block)
                    .wrap(Wrap { trim: true }),
                *cell,
            );
        }
    }
}

fn format_product_status(product: &crate::storage::ProductSummary) -> String {
    let structure = if product.has_structure { "S+" } else { "S-" };
    if product.marketplace_statuses.is_empty() {
        return format!("{structure} L-");
    }
    let listings = product
        .marketplace_statuses
        .iter()
        .map(|status| {
            let label = marketplace_label(&status.marketplace);
            let flag = if status.published { "+" } else { "-" };
            format!("{label}{flag}")
        })
        .collect::<Vec<_>>()
        .join(" ");
    format!("{structure} {listings}")
}

fn marketplace_label(marketplace: &str) -> String {
    let upper = marketplace.to_ascii_uppercase();
    if upper.contains("EBAY_US") {
        "EUS".to_string()
    } else if upper.contains("EBAY_UK") {
        "EUK".to_string()
    } else if upper.contains("EBAY_DE") {
        "EDE".to_string()
    } else {
        upper
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .take(5)
            .collect()
    }
}

fn render_products_workspace(frame: &mut Frame, app: &mut AppState, theme: &Theme, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(6)])
        .split(area);

    render_products_subtabs(frame, app, theme, rows[0]);

    match app.products_subtab {
        crate::app::ProductsSubTab::Context => {
            let palette = mondrian_palette();
            let mut idx = 0usize;
            let images_style = next_style(&palette, &mut idx);
            let text_style = next_style(&palette, &mut idx);

            let columns = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
                .split(rows[1]);

            render_context_images_panel(frame, app, theme, columns[0], images_style);
            render_context_text_panel(frame, app, theme, columns[1], text_style);
        }
        crate::app::ProductsSubTab::Structure => {
            let palette = mondrian_palette();
            let mut idx = 0usize;
            let structure_style = next_style(&palette, &mut idx);
            let structure_detail_style = next_style(&palette, &mut idx);
            let entries = app.structure_entries();

            let columns = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
                .split(rows[1]);

            render_structure_panel(frame, app, theme, columns[0], structure_style, &entries);
            render_structure_detail_panel(
                frame,
                app,
                theme,
                columns[1],
                structure_detail_style,
                &entries,
            );
        }
        crate::app::ProductsSubTab::Listings => {
            let palette = mondrian_palette();
            let mut idx = 0usize;
            let listings_style = next_style(&palette, &mut idx);
            let listings_detail_style = next_style(&palette, &mut idx);
            let entries = app.listing_field_entries();

            let columns = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
                .split(rows[1]);

            render_listings_panel(frame, app, theme, columns[0], listings_style, &entries);
            render_listings_detail_panel(
                frame,
                app,
                theme,
                columns[1],
                listings_detail_style,
                &entries,
            );
        }
    }
}

fn render_products_subtabs(frame: &mut Frame, app: &AppState, theme: &Theme, area: Rect) {
    let sync_marker = if app.product_syncing {
        format!(" {}", app.spinner_frame())
    } else {
        String::new()
    };
    let context_title = if app.products_subtab == crate::app::ProductsSubTab::Context {
        format!(" Context{sync_marker} ")
    } else {
        " Context ".to_string()
    };
    let structure_title = if app.products_subtab == crate::app::ProductsSubTab::Structure {
        format!(" Structure{sync_marker} ")
    } else {
        " Structure ".to_string()
    };
    let listings_title = if app.products_subtab == crate::app::ProductsSubTab::Listings {
        format!(" Listings{sync_marker} ")
    } else {
        " Listings ".to_string()
    };
    let titles = [context_title, structure_title, listings_title]
        .iter()
        .map(|t| Line::from(t.clone()))
        .collect::<Vec<_>>();
    let selected = match app.products_subtab {
        crate::app::ProductsSubTab::Context => 0,
        crate::app::ProductsSubTab::Structure => 1,
        crate::app::ProductsSubTab::Listings => 2,
    };
    let tabs = Tabs::new(titles)
        .select(selected)
        .block(theme.panel_block().title("Product Views"))
        .style(theme.panel())
        .highlight_style(theme.title())
        .divider(" ");
    frame.render_widget(tabs, area);
}

fn render_context_images_panel(
    frame: &mut Frame,
    app: &mut AppState,
    theme: &Theme,
    area: Rect,
    style: BoxStyle,
) {
    let focused = app.products_subtab == crate::app::ProductsSubTab::Context
        && app.context_focus == crate::app::ContextFocus::Images;
    let sku = app
        .active_product
        .as_ref()
        .map(|p| p.sku_alias.as_str())
        .unwrap_or("none");
    let title = format!("Images · {sku}");
    let block = focus_block(mondrian_block(theme, &title, style), focused, theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let has_terminal_preview = app.terminal_preview.is_some();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(if has_terminal_preview {
            vec![
                Constraint::Length(2),
                Constraint::Percentage(app.preview_height_pct()),
                Constraint::Min(4),
            ]
        } else {
            vec![Constraint::Length(2), Constraint::Min(4)]
        })
        .split(inner);

    let entries = app.context_image_entries();
    let stored_count = entries.len();
    let info = format!(
        "Images: {}  |  Shift+S save+sync  |  t camera | v device picker | c capture",
        stored_count
    );
    frame.render_widget(
        Paragraph::new(info)
            .style(mondrian_style(style))
            .wrap(Wrap { trim: true }),
        chunks[0],
    );

    if has_terminal_preview {
        render_terminal_preview(frame, app, theme, chunks[1], style);
    }

    let list_area = if has_terminal_preview {
        chunks[2]
    } else {
        chunks[1]
    };

    if entries.is_empty() {
        frame.render_widget(
            Paragraph::new("No images yet.")
                .style(mondrian_style(style))
                .wrap(Wrap { trim: true }),
            list_area,
        );
        return;
    }

    let rows = entries
        .iter()
        .enumerate()
        .map(|(idx, entry)| {
            let (tag, rel_path, source, sharp, created) = match entry {
                crate::app::ContextImageEntry::Session {
                    rel_path,
                    sharpness_score,
                    created_at,
                    selected,
                } => (
                    if *selected { "*" } else { "" }.to_string(),
                    rel_path.clone(),
                    "session".to_string(),
                    sharpness_score
                        .map(|s| format!("{s:.1}"))
                        .unwrap_or_else(|| "n/a".to_string()),
                    created_at.format("%H:%M:%S").to_string(),
                ),
                crate::app::ContextImageEntry::Product {
                    rel_path,
                    created_at,
                    source,
                    hero,
                } => (
                    if *hero { "H" } else { "" }.to_string(),
                    rel_path.clone(),
                    source.clone(),
                    "n/a".to_string(),
                    created_at.format("%H:%M:%S").to_string(),
                ),
            };
            let name = Path::new(&rel_path)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("image.jpg");
            Row::new(vec![
                tag,
                format!("{idx:02}"),
                name.to_string(),
                source,
                sharp,
                created,
            ])
        })
        .collect::<Vec<_>>();

    let mut state = TableState::default();
    if !entries.is_empty() {
        state.select(Some(
            app.session_frame_selected
                .min(entries.len().saturating_sub(1)),
        ));
    }

    let table = Table::new(
        rows,
        [
            Constraint::Length(3),
            Constraint::Length(4),
            Constraint::Percentage(48),
            Constraint::Length(10),
            Constraint::Length(8),
            Constraint::Length(10),
        ],
    )
    .header(
        Row::new(vec!["Tag", "#", "Filename", "Src", "Sharp", "Time"]).style(mondrian_title(style)),
    )
    .row_highlight_style(
        Style::default()
            .fg(theme.panel)
            .bg(theme.accent)
            .add_modifier(Modifier::BOLD),
    )
    .highlight_symbol("> ")
    .style(mondrian_style(style));

    frame.render_stateful_widget(table, list_area, &mut state);
}

fn render_terminal_preview(
    frame: &mut Frame,
    app: &mut AppState,
    theme: &Theme,
    area: Rect,
    style: BoxStyle,
) {
    let show_camera = app.capture_status.streaming;
    let has_image = app.preview_image_path.is_some();
    let block = mondrian_block(theme, "Preview", style);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let Some(preview) = app.terminal_preview.as_mut() else {
        return;
    };

    if let Some(err) = &preview.last_error {
        frame.render_widget(
            Paragraph::new(err.clone())
                .style(mondrian_style(style))
                .wrap(Wrap { trim: true }),
            inner,
        );
        return;
    }

    let mut render_panel =
        |label: &str, panel_area: Rect, state: &mut Option<StatefulProtocol>, placeholder: &str| {
            let rows = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(1), Constraint::Min(1)])
                .split(panel_area);

            frame.render_widget(Paragraph::new(label).style(mondrian_title(style)), rows[0]);

            if let Some(state) = state.as_mut() {
                frame.render_stateful_widget(StatefulImage::default(), rows[1], state);
                if let Some(result) = state.last_encoding_result() {
                    if let Err(err) = result {
                        preview.last_error = Some(err.to_string());
                    }
                }
            } else {
                frame.render_widget(
                    Paragraph::new(placeholder)
                        .style(mondrian_style(style))
                        .wrap(Wrap { trim: true }),
                    rows[1],
                );
            }
        };

    if show_camera && has_image {
        let panels = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(inner);
        render_panel(
            "Live",
            panels[0],
            &mut preview.camera_state,
            "Waiting for camera...",
        );
        render_panel(
            "Selected",
            panels[1],
            &mut preview.image_state,
            "Select an image to preview.",
        );
        return;
    }

    if show_camera {
        render_panel(
            "Live",
            inner,
            &mut preview.camera_state,
            "Waiting for camera...",
        );
        return;
    }

    if has_image {
        render_panel(
            "Selected",
            inner,
            &mut preview.image_state,
            "Select an image to preview.",
        );
        return;
    }

    frame.render_widget(
        Paragraph::new("Press t to start live preview or select an image.")
            .style(mondrian_style(style))
            .wrap(Wrap { trim: true }),
        inner,
    );
}

fn render_context_text_panel(
    frame: &mut Frame,
    app: &AppState,
    theme: &Theme,
    area: Rect,
    style: BoxStyle,
) {
    let focused = app.products_subtab == crate::app::ProductsSubTab::Context
        && app.context_focus == crate::app::ContextFocus::Text;
    let title = if app.text_editing {
        "Text (editing)"
    } else {
        "Text"
    };
    let block = focus_block(mondrian_block(theme, title, style), focused, theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines = Vec::new();
    if app.text_editing {
        lines.push("Editing (Esc to save)".to_string());
        lines.push(String::new());
    }
    if app.context_text.is_empty() {
        if !app.text_editing {
            if app.context_focus == crate::app::ContextFocus::Text {
                lines.push("Press Enter to edit text.".to_string());
            } else {
                lines.push("Select Text to edit.".to_string());
            }
        }
    } else {
        lines.push(app.context_text.clone());
    }
    let body = lines.join("\n");
    frame.render_widget(
        Paragraph::new(body)
            .style(mondrian_style(style))
            .wrap(Wrap { trim: true }),
        inner,
    );
}

fn render_structure_panel(
    frame: &mut Frame,
    app: &mut AppState,
    theme: &Theme,
    area: Rect,
    style: BoxStyle,
    entries: &[crate::app::StructureFieldEntry],
) {
    let focused = app.products_subtab == crate::app::ProductsSubTab::Structure;
    let spinner = if app.structure_inference {
        format!(" {}", app.spinner_frame())
    } else {
        String::new()
    };
    let title = if app.structure_editing {
        "Structure JSON (editing)".to_string()
    } else {
        format!("Structure Fields{spinner}")
    };
    let block = focus_block(mondrian_block(theme, &title, style), focused, theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if app.structure_editing {
        let mut lines = Vec::new();
        lines.push("Editing full structure (Esc to save)".to_string());
        lines.push(String::new());
        lines.push(app.structure_text.clone());
        let body = lines.join("\n");
        frame.render_widget(
            Paragraph::new(body)
                .style(mondrian_style(style))
                .wrap(Wrap { trim: true }),
            inner,
        );
        return;
    }

    if entries.is_empty() {
        frame.render_widget(
            Paragraph::new("No structure yet. Press r to generate.")
                .style(mondrian_style(style))
                .wrap(Wrap { trim: true }),
            inner,
        );
        return;
    }

    let visible = inner.height as usize;
    let selected = app
        .structure_field_selected
        .min(entries.len().saturating_sub(1));
    app.structure_field_selected = selected;
    if visible > 0 {
        if selected < app.structure_list_offset {
            app.structure_list_offset = selected;
        } else if selected >= app.structure_list_offset + visible {
            app.structure_list_offset = selected + 1 - visible;
        }
    }
    if entries.len() <= visible {
        app.structure_list_offset = 0;
    } else if app.structure_list_offset >= entries.len() {
        app.structure_list_offset = entries.len().saturating_sub(1);
    }

    let items = entries
        .iter()
        .map(|entry| {
            if entry.path == "image" {
                let label = format_structure_image_label(&entry.value);
                ListItem::new(format!("{label}:"))
            } else {
                let value = format_structure_value_inline(&entry.value);
                ListItem::new(format!("{}: {}", entry.path, value))
            }
        })
        .collect::<Vec<_>>();
    let list = List::new(items)
        .style(mondrian_style(style))
        .highlight_style(
            Style::default()
                .fg(style.bg)
                .bg(style.fg)
                .add_modifier(Modifier::BOLD),
        );
    let mut state = ListState::default()
        .with_selected(Some(selected))
        .with_offset(app.structure_list_offset);
    frame.render_stateful_widget(list, inner, &mut state);
}

fn render_structure_detail_panel(
    frame: &mut Frame,
    app: &AppState,
    theme: &Theme,
    area: Rect,
    style: BoxStyle,
    entries: &[crate::app::StructureFieldEntry],
) {
    let focused = app.products_subtab == crate::app::ProductsSubTab::Structure;
    let selected = app
        .structure_field_selected
        .min(entries.len().saturating_sub(1));
    let path = entries
        .get(selected)
        .map(|entry| entry.path.as_str())
        .unwrap_or("none");
    let title = if app.structure_field_editing {
        format!("Field Editor: {path}")
    } else if app.structure_editing {
        "Structure JSON".to_string()
    } else {
        "Structure Detail".to_string()
    };
    let block = focus_block(mondrian_block(theme, &title, style), focused, theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines = Vec::new();
    if app.structure_editing {
        lines.push("Editing full structure JSON (Esc to save).".to_string());
    } else if app.structure_field_editing {
        lines.push(format!("Editing {path} (Esc to save)."));
        lines.push(String::new());
        lines.push(app.structure_field_edit_buffer.clone());
    } else if let Some(entry) = entries.get(selected) {
        if app
            .active_product
            .as_ref()
            .and_then(|p| p.structure_json.as_ref())
            .is_none()
        {
            lines.push("No structure yet. Press r to generate.".to_string());
            lines.push(String::new());
        }
        lines.push(format!("Field: {}", entry.path));
        lines.push(String::new());
        if entry.path == "image" {
            lines.extend(format_images_lines(&entry.value));
            lines.push(String::new());
            lines.push("Enter edit | r generate | E edit JSON".to_string());
            lines.push("Format: one URL per line (or JSON array).".to_string());
        } else {
            lines.push(format_structure_value_full(&entry.value));
            lines.push(String::new());
            lines.push("Enter edit | r generate | E edit JSON".to_string());
        }
    } else {
        lines.push("No structure available.".to_string());
    }

    let body = lines.join("\n");
    frame.render_widget(
        Paragraph::new(body)
            .style(mondrian_style(style))
            .wrap(Wrap { trim: true }),
        inner,
    );
}

fn render_listings_panel(
    frame: &mut Frame,
    app: &mut AppState,
    theme: &Theme,
    area: Rect,
    style: BoxStyle,
    entries: &[crate::app::ListingFieldEntry],
) {
    let focused = app.products_subtab == crate::app::ProductsSubTab::Listings;
    let keys = app.listing_keys();
    let selected = app.listings_selected.min(keys.len().saturating_sub(1));
    app.listings_selected = selected;
    let marketplace = keys
        .get(selected)
        .cloned()
        .unwrap_or_else(|| "EBAY_US".to_string());
    let spinner = if app.listing_inference {
        format!(" {}", app.spinner_frame())
    } else {
        String::new()
    };
    let title = if app.listings_editing {
        format!("Listing JSON (editing) · {marketplace}")
    } else {
        format!("Listing Fields · {marketplace}{spinner}")
    };
    let block = focus_block(mondrian_block(theme, &title, style), focused, theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if app.listings_editing {
        let mut lines = Vec::new();
        lines.push("Editing full listing JSON (Esc to save)".to_string());
        lines.push(String::new());
        lines.push(app.listings_edit_buffer.clone());
        let body = lines.join("\n");
        frame.render_widget(
            Paragraph::new(body)
                .style(mondrian_style(style))
                .wrap(Wrap { trim: true }),
            inner,
        );
        return;
    }

    if entries.is_empty() {
        frame.render_widget(
            Paragraph::new("No listing fields available.")
                .style(mondrian_style(style))
                .wrap(Wrap { trim: true }),
            inner,
        );
        return;
    }

    let visible = inner.height as usize;
    let selected = app
        .listings_field_selected
        .min(entries.len().saturating_sub(1));
    app.listings_field_selected = selected;
    if visible > 0 {
        if selected < app.listings_field_list_offset {
            app.listings_field_list_offset = selected;
        } else if selected >= app.listings_field_list_offset + visible {
            app.listings_field_list_offset = selected + 1 - visible;
        }
    }
    if entries.len() <= visible {
        app.listings_field_list_offset = 0;
    } else if app.listings_field_list_offset >= entries.len() {
        app.listings_field_list_offset = entries.len().saturating_sub(1);
    }

    let items = entries
        .iter()
        .map(|entry| {
            let mut label = entry.label.clone();
            if entry.indent > 0 {
                label = format!("{}{}", " ".repeat(entry.indent), label);
            }
            if entry.key == ListingFieldKey::Aspects
                || entry.key == ListingFieldKey::PackageDimensions
                || entry.key == ListingFieldKey::Images
            {
                ListItem::new(format!("{label}:"))
            } else if entry.key == ListingFieldKey::AspectValue {
                let value = format_aspect_value_inline(&entry.value);
                ListItem::new(format!("{label}: {value}"))
            } else if entry.key == ListingFieldKey::ImageValue {
                let value = format_image_value_inline(&entry.value);
                ListItem::new(format!("{label}: {value}"))
            } else {
                let value = format_structure_value_inline(&entry.value);
                ListItem::new(format!("{label}: {value}"))
            }
        })
        .collect::<Vec<_>>();
    let list = List::new(items)
        .style(mondrian_style(style))
        .highlight_style(
            Style::default()
                .fg(style.bg)
                .bg(style.fg)
                .add_modifier(Modifier::BOLD),
        );
    let mut state = ListState::default()
        .with_selected(Some(selected))
        .with_offset(app.listings_field_list_offset);
    frame.render_stateful_widget(list, inner, &mut state);
}

fn render_listings_detail_panel(
    frame: &mut Frame,
    app: &AppState,
    theme: &Theme,
    area: Rect,
    style: BoxStyle,
    entries: &[crate::app::ListingFieldEntry],
) {
    let focused = app.products_subtab == crate::app::ProductsSubTab::Listings;
    let keys = app.listing_keys();
    let selected_marketplace = keys
        .get(app.listings_selected.min(keys.len().saturating_sub(1)))
        .cloned()
        .unwrap_or_else(|| "EBAY_US".to_string());
    let selected = app
        .listings_field_selected
        .min(entries.len().saturating_sub(1));
    let selected_entry = entries.get(selected);
    let label = selected_entry
        .map(|entry| entry.label.as_str())
        .unwrap_or("none");
    let selected_key = selected_entry.map(|entry| entry.key);
    let title = if app.listings_field_editing {
        format!("Field Editor: {label}")
    } else if app.listings_editing {
        "Listing JSON".to_string()
    } else {
        "Listing Detail".to_string()
    };
    let block = focus_block(mondrian_block(theme, &title, style), focused, theme);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let listing_exists = app
        .active_product
        .as_ref()
        .is_some_and(|product| product.listings.contains_key(selected_marketplace.as_str()));

    let mut lines = Vec::new();
    lines.push(format!("Marketplace: {selected_marketplace} (←/→ switch)"));
    lines.push(String::new());

    if app.listings_editing {
        lines.push("Editing full listing JSON (Esc to save).".to_string());
    } else if app.listings_field_editing {
        lines.push(format!("Editing {label} (Esc to save)."));
        if selected_key == Some(ListingFieldKey::AspectValue) {
            lines.push("Format: Value1, Value2 (or JSON array).".to_string());
        } else if selected_key == Some(ListingFieldKey::Images) {
            lines.push("Format: one URL per line (or JSON array).".to_string());
        } else if selected_key == Some(ListingFieldKey::ImageValue) {
            lines.push("Format: full image URL.".to_string());
        } else if selected_key == Some(ListingFieldKey::PackageWeight) {
            lines.push("Format: 10 OUNCE (or 2 POUND).".to_string());
        } else if selected_key == Some(ListingFieldKey::PackageDimensions) {
            lines.push("Format: L x W x H INCH (ex: 6.0 x 4.5 x 2.0 INCH).".to_string());
        } else if selected_key == Some(ListingFieldKey::PackageDimensionValue) {
            let is_unit = selected_entry.and_then(|entry| entry.dimension_key)
                == Some(PackageDimensionKey::Unit);
            if is_unit {
                lines.push("Format: INCH or CENTIMETER.".to_string());
            } else {
                lines.push("Format: number (rounded up to 1 decimal).".to_string());
            }
        }
        lines.push(String::new());
        lines.push(app.listings_field_edit_buffer.clone());
    } else if let Some(entry) = selected_entry {
        if !listing_exists {
            lines.push("No listing data yet. Run p to draft or edit fields.".to_string());
            lines.push(String::new());
        }
        lines.push(format!("Field: {}", entry.label.as_str()));
        lines.push(String::new());
        if entry.key == ListingFieldKey::Aspects {
            lines.push("Select an aspect below to view or edit values.".to_string());
            lines.push(String::new());
            lines.push(
                "Enter edit | g full | p draft | P publish | E edit JSON | u upload".to_string(),
            );
            lines.push("Format: Value1, Value2 (or JSON array).".to_string());
        } else if entry.key == ListingFieldKey::Images {
            lines.extend(format_images_lines(&entry.value));
            lines.push(String::new());
            lines.push("Select an image below to preview or edit.".to_string());
            lines.push(String::new());
            lines.push(
                "Enter edit | g full | p draft | P publish | E edit JSON | u upload".to_string(),
            );
            lines.push("Format: one URL per line (or JSON array).".to_string());
        } else if entry.key == ListingFieldKey::ImageValue {
            lines.push(format_structure_value_full(&entry.value));
            lines.push(String::new());
            lines.push("Preview opens for matching local captures.".to_string());
            lines.push(
                "Enter edit | g full | p draft | P publish | E edit JSON | u upload".to_string(),
            );
        } else if entry.key == ListingFieldKey::AspectValue {
            lines.extend(format_aspect_values_lines(&entry.value));
            lines.push(String::new());
            lines.push(
                "Enter edit | g full | p draft | P publish | E edit JSON | u upload".to_string(),
            );
        } else if entry.key == ListingFieldKey::PackageWeight {
            lines.push(format_structure_value_full(&entry.value));
            lines.push(String::new());
            lines.push(
                "Enter edit | g full | p draft | P publish | E edit JSON | u upload".to_string(),
            );
            lines.push("Format: 10 OUNCE (or 2 POUND).".to_string());
        } else if entry.key == ListingFieldKey::PackageDimensions {
            lines.push("Select a dimension below to edit.".to_string());
            lines.push(String::new());
            lines.push(
                "Enter edit | g full | p draft | P publish | E edit JSON | u upload".to_string(),
            );
        } else if entry.key == ListingFieldKey::PackageDimensionValue {
            lines.push(format_structure_value_full(&entry.value));
            lines.push(String::new());
            lines.push(
                "Enter edit | g full | p draft | P publish | E edit JSON | u upload".to_string(),
            );
            if entry.dimension_key == Some(PackageDimensionKey::Unit) {
                lines.push("Format: INCH or CENTIMETER.".to_string());
            } else {
                lines.push("Format: number (rounded up to 1 decimal).".to_string());
            }
        } else {
            lines.push(format_structure_value_full(&entry.value));
            lines.push(String::new());
            lines.push(
                "Enter edit | g full | p draft | P publish | E edit JSON | u upload".to_string(),
            );
        }
        lines.push(String::new());
    } else {
        lines.push("No listing fields available.".to_string());
    }

    let body = lines.join("\n");
    frame.render_widget(
        Paragraph::new(body)
            .style(mondrian_style(style))
            .wrap(Wrap { trim: true }),
        inner,
    );
}

fn render_activity(frame: &mut Frame, app: &AppState, theme: &Theme, area: Rect) {
    let style = mondrian_palette()[0];
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
            .style(mondrian_style(style))
            .block(mondrian_block(theme, "Activity", style))
            .highlight_style(Style::default().fg(style.fg).add_modifier(Modifier::BOLD)),
        area,
    );
}

fn render_settings(frame: &mut Frame, app: &AppState, theme: &Theme, area: Rect) {
    let style = mondrian_palette()[0];
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(9), Constraint::Min(6)])
        .split(area);

    let stderr = app
        .stderr_log_path
        .as_ref()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "not redirected".to_string());
    let activity_log = app
        .activity_log_path
        .as_ref()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "not set".to_string());
    let text = format!(
        "captures dir: {}\nlog stderr: {}\nactivity log: {}\n\nConfig:\n  base_url: {}\n  hermes api key: {}\n  hermes online: {}\n  preview height: {}%\n\nTALARIA_CAPTURES_DIR overrides base capture path.",
        app.captures_dir.display(),
        stderr,
        activity_log,
        app.config
            .base_url
            .clone()
            .unwrap_or_else(|| "(none)".to_string()),
        if app.config.hermes_api_key_present {
            "present"
        } else {
            "missing"
        },
        if app.config.online_ready {
            "ready"
        } else {
            "offline"
        },
        app.preview_height_pct,
    );
    frame.render_widget(
        Paragraph::new(text)
            .style(mondrian_style(style))
            .block(mondrian_block(theme, "Settings", style))
            .wrap(Wrap { trim: true }),
        chunks[0],
    );

    let fields = [
        ("Hermes API Key", SettingsField::HermesApiKey),
        ("Preview Height (%)", SettingsField::PreviewHeightPct),
        ("Marketplace", SettingsField::Marketplace),
        ("Merchant Location Key", SettingsField::MerchantLocation),
        ("Fulfillment Policy ID", SettingsField::FulfillmentPolicy),
        ("Payment Policy ID", SettingsField::PaymentPolicy),
        ("Return Policy ID", SettingsField::ReturnPolicy),
        ("HSUF Prompt Rules", SettingsField::HsufPromptRules),
        ("LLM Ingest Model", SettingsField::LlmIngestModel),
        ("LLM Ingest Reasoning", SettingsField::LlmIngestReasoning),
        ("LLM Ingest Web Search", SettingsField::LlmIngestWebSearch),
        ("LLM Aspects Model", SettingsField::LlmAspectsModel),
        ("LLM Aspects Reasoning", SettingsField::LlmAspectsReasoning),
        ("LLM Aspects Web Search", SettingsField::LlmAspectsWebSearch),
    ];
    let rows = fields
        .iter()
        .enumerate()
        .map(|(idx, (label, field))| {
            let mut value = match field {
                SettingsField::HermesApiKey => {
                    if app.config.hermes_api_key_present {
                        "(present)".to_string()
                    } else {
                        "(unset)".to_string()
                    }
                }
                SettingsField::PreviewHeightPct => app.preview_height_pct.to_string(),
                SettingsField::Marketplace => app
                    .ebay_settings
                    .marketplace
                    .clone()
                    .unwrap_or_else(|| "(unset)".to_string()),
                SettingsField::MerchantLocation => app
                    .ebay_settings
                    .merchant_location_key
                    .clone()
                    .unwrap_or_else(|| "(unset)".to_string()),
                SettingsField::FulfillmentPolicy => app
                    .ebay_settings
                    .fulfillment_policy_id
                    .clone()
                    .unwrap_or_else(|| "(unset)".to_string()),
                SettingsField::PaymentPolicy => app
                    .ebay_settings
                    .payment_policy_id
                    .clone()
                    .unwrap_or_else(|| "(unset)".to_string()),
                SettingsField::ReturnPolicy => app
                    .ebay_settings
                    .return_policy_id
                    .clone()
                    .unwrap_or_else(|| "(unset)".to_string()),
                SettingsField::HsufPromptRules => app
                    .prompt_rules
                    .clone()
                    .unwrap_or_else(|| "(unset)".to_string()),
                SettingsField::LlmIngestModel => app
                    .llm_ingest
                    .as_ref()
                    .map(|opts| llm_model_label(&opts.model).to_string())
                    .unwrap_or_else(|| "(unset)".to_string()),
                SettingsField::LlmIngestReasoning => {
                    llm_bool_label(app.llm_ingest.as_ref().and_then(|opts| opts.reasoning))
                }
                SettingsField::LlmIngestWebSearch => {
                    llm_bool_label(app.llm_ingest.as_ref().and_then(|opts| opts.web_search))
                }
                SettingsField::LlmAspectsModel => app
                    .llm_aspects
                    .as_ref()
                    .map(|opts| llm_model_label(&opts.model).to_string())
                    .unwrap_or_else(|| "(unset)".to_string()),
                SettingsField::LlmAspectsReasoning => {
                    llm_bool_label(app.llm_aspects.as_ref().and_then(|opts| opts.reasoning))
                }
                SettingsField::LlmAspectsWebSearch => {
                    llm_bool_label(app.llm_aspects.as_ref().and_then(|opts| opts.web_search))
                }
            };
            if app.settings_editing && app.settings_selected == idx {
                value = app.settings_edit_buffer.clone();
            }
            Row::new(vec![label.to_string(), value])
        })
        .collect::<Vec<_>>();

    let mut state = TableState::default();
    state.select(Some(
        app.settings_selected.min(fields.len().saturating_sub(1)),
    ));

    let table = Table::new(rows, [Constraint::Length(26), Constraint::Percentage(70)])
        .header(Row::new(vec!["Field", "Value"]).style(mondrian_title(style)))
        .block(mondrian_block(theme, "Settings", style))
        .row_highlight_style(Style::default().fg(style.fg).add_modifier(Modifier::BOLD))
        .style(mondrian_style(style));

    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(chunks[1]);

    frame.render_stateful_widget(table, columns[0], &mut state);
    render_settings_detail_panel(frame, app, theme, columns[1], style);
}

fn render_settings_detail_panel(
    frame: &mut Frame,
    app: &AppState,
    theme: &Theme,
    area: Rect,
    style: BoxStyle,
) {
    let fields = [
        ("Hermes API Key", SettingsField::HermesApiKey),
        ("Preview Height (%)", SettingsField::PreviewHeightPct),
        ("Marketplace", SettingsField::Marketplace),
        ("Merchant Location Key", SettingsField::MerchantLocation),
        ("Fulfillment Policy ID", SettingsField::FulfillmentPolicy),
        ("Payment Policy ID", SettingsField::PaymentPolicy),
        ("Return Policy ID", SettingsField::ReturnPolicy),
        ("HSUF Prompt Rules", SettingsField::HsufPromptRules),
        ("LLM Ingest Model", SettingsField::LlmIngestModel),
        ("LLM Ingest Reasoning", SettingsField::LlmIngestReasoning),
        ("LLM Ingest Web Search", SettingsField::LlmIngestWebSearch),
        ("LLM Aspects Model", SettingsField::LlmAspectsModel),
        ("LLM Aspects Reasoning", SettingsField::LlmAspectsReasoning),
        ("LLM Aspects Web Search", SettingsField::LlmAspectsWebSearch),
    ];
    let selected = app.settings_selected.min(fields.len().saturating_sub(1));
    let (label, field) = fields[selected];
    let title = if app.settings_editing {
        format!("Setting Editor: {label}")
    } else {
        "Setting Detail".to_string()
    };
    let block = mondrian_block(theme, &title, style);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let value = match field {
        SettingsField::HermesApiKey => {
            if app.config.hermes_api_key_present {
                "(present)".to_string()
            } else {
                "(unset)".to_string()
            }
        }
        SettingsField::PreviewHeightPct => app.preview_height_pct.to_string(),
        SettingsField::Marketplace => app
            .ebay_settings
            .marketplace
            .clone()
            .unwrap_or_else(|| "(unset)".to_string()),
        SettingsField::MerchantLocation => app
            .ebay_settings
            .merchant_location_key
            .clone()
            .unwrap_or_else(|| "(unset)".to_string()),
        SettingsField::FulfillmentPolicy => app
            .ebay_settings
            .fulfillment_policy_id
            .clone()
            .unwrap_or_else(|| "(unset)".to_string()),
        SettingsField::PaymentPolicy => app
            .ebay_settings
            .payment_policy_id
            .clone()
            .unwrap_or_else(|| "(unset)".to_string()),
        SettingsField::ReturnPolicy => app
            .ebay_settings
            .return_policy_id
            .clone()
            .unwrap_or_else(|| "(unset)".to_string()),
        SettingsField::HsufPromptRules => app
            .prompt_rules
            .clone()
            .unwrap_or_else(|| "(unset)".to_string()),
        SettingsField::LlmIngestModel => app
            .llm_ingest
            .as_ref()
            .map(|opts| llm_model_label(&opts.model).to_string())
            .unwrap_or_else(|| "(unset)".to_string()),
        SettingsField::LlmIngestReasoning => {
            llm_bool_label(app.llm_ingest.as_ref().and_then(|opts| opts.reasoning))
        }
        SettingsField::LlmIngestWebSearch => {
            llm_bool_label(app.llm_ingest.as_ref().and_then(|opts| opts.web_search))
        }
        SettingsField::LlmAspectsModel => app
            .llm_aspects
            .as_ref()
            .map(|opts| llm_model_label(&opts.model).to_string())
            .unwrap_or_else(|| "(unset)".to_string()),
        SettingsField::LlmAspectsReasoning => {
            llm_bool_label(app.llm_aspects.as_ref().and_then(|opts| opts.reasoning))
        }
        SettingsField::LlmAspectsWebSearch => {
            llm_bool_label(app.llm_aspects.as_ref().and_then(|opts| opts.web_search))
        }
    };

    let mut lines = Vec::new();
    if app.settings_editing {
        lines.push(format!("Editing {label} (Enter save, Esc cancel)."));
        lines.push(String::new());
        if matches!(field, SettingsField::HermesApiKey) {
            lines.push("Paste new key. Blank = keep current. Type CLEAR to remove.".to_string());
            lines.push("Or run: talaria auth login".to_string());
            lines.push(String::new());
        }
        if matches!(field, SettingsField::PreviewHeightPct) {
            lines.push(format!(
                "Enter {PREVIEW_HEIGHT_MIN_PCT}-{PREVIEW_HEIGHT_MAX_PCT}. DEFAULT resets."
            ));
            lines.push(String::new());
        }
        if matches!(field, SettingsField::HsufPromptRules) {
            lines.push("Applies to HSUF inference prompts.".to_string());
            lines.push("Type CLEAR to remove.".to_string());
            lines.push(String::new());
        }
        if matches!(
            field,
            SettingsField::LlmIngestModel | SettingsField::LlmAspectsModel
        ) {
            lines.push("Use gpt-5.2, gpt-5-mini, or gpt-5-nano.".to_string());
            lines.push("CLEAR removes the override.".to_string());
            lines.push(String::new());
        }
        if matches!(
            field,
            SettingsField::LlmIngestReasoning
                | SettingsField::LlmIngestWebSearch
                | SettingsField::LlmAspectsReasoning
                | SettingsField::LlmAspectsWebSearch
        ) {
            lines.push("Use true/false (or CLEAR).".to_string());
            lines.push(String::new());
        }
        lines.push(app.settings_edit_buffer.clone());
    } else {
        lines.push(format!("Field: {label}"));
        lines.push(String::new());
        lines.push(value);
        lines.push(String::new());
        if matches!(
            field,
            SettingsField::LlmIngestModel
                | SettingsField::LlmIngestReasoning
                | SettingsField::LlmIngestWebSearch
                | SettingsField::LlmAspectsModel
                | SettingsField::LlmAspectsReasoning
                | SettingsField::LlmAspectsWebSearch
        ) {
            lines.push("Enter pick | Esc cancel".to_string());
        } else {
            lines.push("Enter edit | Esc cancel".to_string());
        }
    }
    let body = lines.join("\n");
    frame.render_widget(
        Paragraph::new(body)
            .style(mondrian_style(style))
            .wrap(Wrap { trim: true }),
        inner,
    );
}

fn settings_field_label(field: SettingsField) -> &'static str {
    match field {
        SettingsField::HermesApiKey => "Hermes API Key",
        SettingsField::PreviewHeightPct => "Preview Height (%)",
        SettingsField::Marketplace => "Marketplace",
        SettingsField::MerchantLocation => "Merchant Location Key",
        SettingsField::FulfillmentPolicy => "Fulfillment Policy ID",
        SettingsField::PaymentPolicy => "Payment Policy ID",
        SettingsField::ReturnPolicy => "Return Policy ID",
        SettingsField::HsufPromptRules => "HSUF Prompt Rules",
        SettingsField::LlmIngestModel => "LLM Ingest Model",
        SettingsField::LlmIngestReasoning => "LLM Ingest Reasoning",
        SettingsField::LlmIngestWebSearch => "LLM Ingest Web Search",
        SettingsField::LlmAspectsModel => "LLM Aspects Model",
        SettingsField::LlmAspectsReasoning => "LLM Aspects Reasoning",
        SettingsField::LlmAspectsWebSearch => "LLM Aspects Web Search",
    }
}

fn llm_model_label(model: &talaria_core::models::LlmModel) -> &'static str {
    match model {
        talaria_core::models::LlmModel::Gpt5_2 => "gpt-5.2",
        talaria_core::models::LlmModel::Gpt5Mini => "gpt-5-mini",
        talaria_core::models::LlmModel::Gpt5Nano => "gpt-5-nano",
    }
}

fn llm_bool_label(value: Option<bool>) -> String {
    match value {
        Some(true) => "true".to_string(),
        Some(false) => "false".to_string(),
        None => "(unset)".to_string(),
    }
}

fn focus_block<'a>(block: Block<'a>, focused: bool, theme: &Theme) -> Block<'a> {
    if focused {
        block.border_style(
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        block
    }
}

#[derive(Clone, Copy)]
struct BoxStyle {
    bg: Color,
    fg: Color,
}

fn mondrian_palette() -> [BoxStyle; 3] {
    [
        BoxStyle {
            bg: hex("#c1040b"),
            fg: Color::White,
        },
        BoxStyle {
            bg: hex("#0d2af0"),
            fg: Color::White,
        },
        BoxStyle {
            bg: hex("#f6a200"),
            fg: Color::Black,
        },
    ]
}

fn next_style(palette: &[BoxStyle; 3], idx: &mut usize) -> BoxStyle {
    let style = palette[*idx % palette.len()];
    *idx += 1;
    style
}

fn mondrian_style(style: BoxStyle) -> Style {
    Style::default().fg(style.fg).bg(style.bg)
}

fn mondrian_title(style: BoxStyle) -> Style {
    Style::default().fg(style.fg).add_modifier(Modifier::BOLD)
}

fn mondrian_block<'a>(theme: &'a Theme, title: &'a str, style: BoxStyle) -> Block<'a> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .style(mondrian_style(style))
        .border_style(theme.border())
        .title(Span::styled(title, mondrian_title(style)))
}

fn panel_title<'a>(theme: &'a Theme, title: &'a str) -> Block<'a> {
    theme
        .panel_block()
        .title(Span::styled(title, theme.title()))
}

fn hex(input: &str) -> Color {
    let hex = input.trim_start_matches('#');
    if hex.len() != 6 {
        return Color::Reset;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
    Color::Rgb(r, g, b)
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
            .style(theme.panel())
            .block(panel_title(theme, "Keys"))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn render_help(frame: &mut Frame, theme: &Theme) {
    let area = centered_rect(70, 70, frame.area());
    frame.render_widget(Clear, area);
    let text = [
        "Navigation:",
        "  Shift+Tab: next main tab",
        "  ?: help",
        "  q: quit",
        "  Quickstart tab: step-by-step flow",
        "",
        "Products grid:",
        "  n new product | Enter select | d delete (y confirm)",
        "  ↑/↓/←/→ move selection",
        "",
        "Products workspace:",
        "  Tab switch view (Context / Structure / Listings)",
        "  G back to grid",
        "",
        "Context view:",
        "  ←/→ focus Images/Text",
        "  ↑/↓ select image | Enter select frame or edit text | Del delete",
        "  t camera on/off | v device picker | d/D device | c capture",
        "  r structure | p draft pipeline | P publish pipeline",
        "  Shift+S save + sync | Esc abandon session | Ctrl+S save text",
        "",
        "Structure view:",
        "  ↑/↓ select field | Enter edit | r generate | g listing | Shift+S save + sync | E edit JSON",
        "  Esc save while editing",
        "",
        "Listings view:",
        "  ←/→ switch marketplace",
        "  ↑/↓ select field | Enter edit | E edit JSON",
        "  g run full | p run draft | P publish draft | Shift+S save + sync | u upload images",
        "  Esc save while editing",
        "  Images format: one URL per line (or JSON array)",
        "  Aspects format: Value1, Value2 (or JSON array)",
        "",
        "Settings view:",
        "  ↑/↓ select field | Enter edit/pick | Enter save | Esc cancel",
    ]
    .join("\n");

    frame.render_widget(
        Paragraph::new(text)
            .style(theme.panel())
            .block(panel_title(theme, "Help"))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn render_product_picker(frame: &mut Frame, app: &mut AppState, theme: &Theme) {
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

    let header = Paragraph::new(format!("Search: {}", app.picker.search))
        .style(theme.panel())
        .block(panel_title(theme, "Select Product"));
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
    .header(Row::new(vec!["SKU", "Name", "Updated", "Images"]).style(theme.title()))
    .block(panel_title(theme, "Products"))
    .row_highlight_style(
        Style::default()
            .fg(theme.accent)
            .add_modifier(Modifier::BOLD),
    )
    .style(theme.panel());
    frame.render_stateful_widget(table, chunks[1], &mut state);

    let footer = Paragraph::new("Type to filter | ↑/↓ select | Enter choose | Esc cancel")
        .style(theme.panel())
        .block(theme.panel_block());
    frame.render_widget(footer, chunks[2]);
}

fn render_camera_picker(frame: &mut Frame, app: &mut AppState, theme: &Theme) {
    let area = centered_rect(70, 55, frame.area());
    frame.render_widget(Clear, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(2),
        ])
        .split(area);

    let header = Paragraph::new("Select Camera")
        .style(theme.panel())
        .block(panel_title(theme, "Camera Devices"));
    frame.render_widget(header, chunks[0]);

    if let Some(err) = &app.camera_picker.error {
        let body = Paragraph::new(format!("Error: {err}"))
            .style(theme.panel())
            .block(theme.panel_block())
            .wrap(Wrap { trim: true });
        frame.render_widget(body, chunks[1]);
    } else if app.camera_picker.devices.is_empty() {
        let body = Paragraph::new("No cameras found. Plug in a USB camera and press r to refresh.")
            .style(theme.panel())
            .block(theme.panel_block())
            .wrap(Wrap { trim: true });
        frame.render_widget(body, chunks[1]);
    } else {
        let rows = app
            .camera_picker
            .devices
            .iter()
            .map(|dev| Row::new(vec![dev.index.to_string(), dev.name.clone()]))
            .collect::<Vec<_>>();

        let mut state = TableState::default();
        state.select(Some(
            app.camera_picker
                .selected
                .min(app.camera_picker.devices.len().saturating_sub(1)),
        ));

        let table = Table::new(rows, [Constraint::Length(8), Constraint::Percentage(80)])
            .header(Row::new(vec!["Index", "Name"]).style(theme.title()))
            .block(panel_title(theme, "Devices"))
            .row_highlight_style(
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )
            .style(theme.panel());
        frame.render_stateful_widget(table, chunks[1], &mut state);
    }

    let footer = Paragraph::new("↑/↓ select | Enter choose | r refresh | Esc cancel")
        .style(theme.panel())
        .block(theme.panel_block());
    frame.render_widget(footer, chunks[2]);
}

fn render_settings_picker(frame: &mut Frame, app: &mut AppState, theme: &Theme) {
    let area = centered_rect(50, 50, frame.area());
    frame.render_widget(Clear, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(2),
        ])
        .split(area);

    let label = settings_field_label(app.settings_picker.field);
    let header = Paragraph::new(format!("Field: {label}"))
        .style(theme.panel())
        .block(panel_title(theme, "Select Option"));
    frame.render_widget(header, chunks[0]);

    if app.settings_picker.options.is_empty() {
        let body = Paragraph::new("No options available.")
            .style(theme.panel())
            .block(theme.panel_block())
            .wrap(Wrap { trim: true });
        frame.render_widget(body, chunks[1]);
    } else {
        let items = app
            .settings_picker
            .options
            .iter()
            .map(|opt| ListItem::new(opt.clone()))
            .collect::<Vec<_>>();
        let mut state = ListState::default();
        state.select(Some(
            app.settings_picker
                .selected
                .min(app.settings_picker.options.len().saturating_sub(1)),
        ));
        let list = List::new(items)
            .block(panel_title(theme, "Options"))
            .highlight_style(
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )
            .style(theme.panel());
        frame.render_stateful_widget(list, chunks[1], &mut state);
    }

    let footer = Paragraph::new("↑/↓ select | Enter choose | Esc cancel")
        .style(theme.panel())
        .block(theme.panel_block());
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
        "Camera: {camera} (device {})\nStream: {stream}  FPS: {:.1}  Dropped: {}\nCaptures: {}",
        app.device_index,
        app.capture_status.fps,
        app.capture_status.dropped_frames,
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

fn pipeline_text(app: &AppState) -> String {
    if !app.config.hermes_api_key_present {
        return [
            "Hermes sign-in required for online mode.",
            "Run: talaria auth login",
            "Restart Talaria after login.",
        ]
        .join("\n");
    }

    let mut lines = Vec::new();
    if let Some(snapshot) = &app.credits {
        lines.push(format!("Credits balance: {}", snapshot.balance));
        lines.push(format!(
            "Used: {} | Listings: {}",
            snapshot.credits_used, snapshot.listings_run
        ));
        if let (Some(from), Some(to)) = (&snapshot.window_from, &snapshot.window_to) {
            lines.push(format!("Window: {} → {}", from, to));
        }
    } else if app.credits_loading {
        lines.push("Fetching credits…".to_string());
    } else {
        lines.push("Credits unavailable (retrying).".to_string());
    }

    if let Some(err) = &app.credits_error {
        lines.push(format!("Last error: {err}"));
    }
    if let Some(updated) = app.credits_last_updated {
        lines.push(format!("Updated {}s ago", updated.elapsed().as_secs()));
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
    if !session.picks.selected_rel_paths.is_empty() {
        return 70;
    }
    if !session.frames.is_empty() {
        return 40;
    }
    10
}

fn footer_hints(app: &AppState) -> String {
    let base = "Shift+Tab tabs | ? help | q quit";
    let base_no_arrows = "Shift+Tab tabs | ? help | q quit";
    match app.active_tab {
        AppTab::Products => match app.products_mode {
            crate::app::ProductsMode::Grid => {
                format!("{base_no_arrows} | n new | Enter select | d delete | ↑/↓/←/→ move")
            }
            crate::app::ProductsMode::Workspace => match app.products_subtab {
                crate::app::ProductsSubTab::Context => format!(
                    "{base_no_arrows} | Tab view | Shift+S save+sync | r structure | p draft | P publish | G grid | ←/→ focus | ↑/↓ select | Enter edit | Del delete | t camera on/off | v device picker | d/D device | c capture | Esc abandon"
                ),
                crate::app::ProductsSubTab::Structure => format!(
                    "{base_no_arrows} | Tab view | Shift+S save+sync | G grid | ↑/↓ select | Enter edit | r generate | g listing | E edit JSON"
                ),
                crate::app::ProductsSubTab::Listings => {
                    format!(
                        "{base_no_arrows} | Tab view | Shift+S save+sync | G grid | ←/→ marketplace | ↑/↓ field | Enter edit | g full | p draft | P publish draft | E edit JSON | u upload"
                    )
                }
            },
        },
        AppTab::Settings => {
            if app.settings_picker.open {
                format!("{base} | ↑/↓ select | Enter choose | Esc cancel")
            } else {
                format!("{base} | ↑/↓ select | Enter edit | Enter save | Esc cancel")
            }
        }
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

fn format_structure_value_inline(value: &Value) -> String {
    match value {
        Value::Null => "-".to_string(),
        Value::String(text) => truncate(text, 80),
        Value::Number(num) => num.to_string(),
        Value::Bool(val) => val.to_string(),
        Value::Array(_) | Value::Object(_) => {
            let raw = serde_json::to_string(value).unwrap_or_else(|_| "<invalid>".to_string());
            truncate(&raw, 80)
        }
    }
}

fn format_structure_value_full(value: &Value) -> String {
    match value {
        Value::Null => "(empty)".to_string(),
        Value::String(text) => text.clone(),
        Value::Number(num) => num.to_string(),
        Value::Bool(val) => val.to_string(),
        Value::Array(_) | Value::Object(_) => {
            serde_json::to_string_pretty(value).unwrap_or_else(|_| "<invalid>".to_string())
        }
    }
}

fn format_aspect_value_inline(value: &Value) -> String {
    let values = clean_aspect_values(coerce_aspect_values(value));
    if values.is_empty() {
        return "(missing)".to_string();
    }
    let joined = values.join(", ");
    truncate(&joined, 80)
}

fn format_aspect_values_lines(value: &Value) -> Vec<String> {
    let values = clean_aspect_values(coerce_aspect_values(value));
    if values.is_empty() {
        return vec!["(missing)".to_string()];
    }
    values
}

fn format_image_value_inline(value: &Value) -> String {
    let Value::String(text) = value else {
        return format_structure_value_inline(value);
    };
    let mut label = text.as_str();
    if let Some(last) = text.rsplit('/').next() {
        label = last;
    }
    if let Some(stripped) = label.split('?').next() {
        label = stripped;
    }
    truncate(label, 80)
}

fn format_structure_image_label(value: &Value) -> String {
    let images = coerce_aspect_values(value)
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    let count = images.len();
    if count == 0 {
        "image (none)".to_string()
    } else if count == 1 {
        "image (1 image)".to_string()
    } else {
        format!("image ({count} images)")
    }
}

fn format_images_lines(value: &Value) -> Vec<String> {
    let images = coerce_aspect_values(value)
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    if images.is_empty() {
        return vec!["(none)".to_string()];
    }
    images
}

fn coerce_aspect_values(value: &Value) -> Vec<String> {
    match value {
        Value::Null => Vec::new(),
        Value::String(text) => vec![text.trim().to_string()],
        Value::Number(num) => vec![num.to_string()],
        Value::Bool(val) => vec![val.to_string()],
        Value::Array(items) => items.iter().flat_map(coerce_aspect_values).collect(),
        Value::Object(obj) => obj
            .get("value")
            .or_else(|| obj.get("values"))
            .map(coerce_aspect_values)
            .unwrap_or_default(),
    }
}

fn clean_aspect_values(mut values: Vec<String>) -> Vec<String> {
    values.retain(|value| !value.trim().is_empty());
    values.sort();
    values.dedup();
    values
}

fn toast_style(theme: &Theme, sev: Severity) -> Style {
    match sev {
        Severity::Info => theme.subtle(),
        Severity::Success => theme.ok(),
        Severity::Warning => theme.warn(),
        Severity::Error => theme.err(),
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    format!("{}…", &s[..max.saturating_sub(1)])
}
