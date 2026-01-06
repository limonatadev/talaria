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
use serde_json::Value;

use crate::app::{AppState, AppTab, SettingsField};
use crate::types::Severity;

use self::layout::{centered_rect, main_chunks};
use self::theme::Theme;

pub fn draw(frame: &mut Frame, app: &mut AppState) {
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
    if app.picker.open {
        render_product_picker(frame, app, &theme);
    }
}

fn render_tabs(frame: &mut Frame, app: &AppState, theme: &Theme, area: Rect) {
    let titles = [" Home ", " Products ", " Activity ", " Settings "]
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
        Paragraph::new("TODO: queue summaries, credits/usage, marketplace connections")
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

    let header_text = "Products: n = new product | Enter = select product | d = delete (y confirm) | arrows = move";
    frame.render_widget(
        Paragraph::new(header_text)
            .style(mondrian_style(header_style))
            .block(mondrian_block(theme, "Products", header_style))
            .wrap(Wrap { trim: true }),
        chunks[0],
    );

    if app.picker.products.is_empty() {
        let empty_style = next_style(&palette, &mut idx);
        frame.render_widget(
            Paragraph::new("No products yet.\n\nPress n to create your first product.")
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
            let text = format!(
                "{}\nImages: {}\nUpdated: {}",
                truncate(&name, 24),
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
    let titles = [" Context ", " Structure ", " Listings "]
        .iter()
        .map(|t| Line::from(*t))
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
    app: &AppState,
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

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(4)])
        .split(inner);

    if app.context_images_from_session() {
        let Some(session) = &app.active_session else {
            return;
        };
        let info = format!(
            "Session: {}  |  Frames: {}  |  s camera | c capture, b burst, Enter select, Del delete",
            session.session_id,
            session.frames.len()
        );
        frame.render_widget(
            Paragraph::new(info)
                .style(mondrian_style(style))
                .wrap(Wrap { trim: true }),
            chunks[0],
        );

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
                let selected = if session.picks.selected_rel_paths.contains(&f.rel_path) {
                    "*"
                } else {
                    ""
                };
                Row::new(vec![
                    selected.to_string(),
                    format!("{idx:02}"),
                    name.to_string(),
                    sharp,
                    created,
                ])
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
                Constraint::Length(4),
                Constraint::Percentage(50),
                Constraint::Length(10),
                Constraint::Length(10),
            ],
        )
        .header(
            Row::new(vec!["Sel", "#", "Filename", "Sharp", "Time"]).style(mondrian_title(style)),
        )
        .row_highlight_style(
            Style::default()
                .fg(theme.panel)
                .bg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ")
        .style(mondrian_style(style));

        frame.render_stateful_widget(table, chunks[1], &mut state);
        return;
    }

    let Some(product) = &app.active_product else {
        frame.render_widget(
            Paragraph::new("No product selected.\n\nPress n to start a new product.")
                .style(mondrian_style(style))
                .wrap(Wrap { trim: true }),
            inner,
        );
        return;
    };
    let count = product.images.len();
    let info = format!("Product images: {count}  |  Synced from storage");
    frame.render_widget(
        Paragraph::new(info)
            .style(mondrian_style(style))
            .wrap(Wrap { trim: true }),
        chunks[0],
    );

    if count == 0 {
        frame.render_widget(
            Paragraph::new("No product images found.")
                .style(mondrian_style(style))
                .wrap(Wrap { trim: true }),
            chunks[1],
        );
        return;
    }

    let hero_rel = product.hero_rel_path.as_deref();
    let rows = product
        .images
        .iter()
        .enumerate()
        .map(|(idx, img)| {
            let name = Path::new(&img.rel_path)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("image.jpg");
            let created = img.created_at.format("%H:%M:%S").to_string();
            let source = if img.rel_path.starts_with("remote/") {
                "remote"
            } else if img.rel_path.starts_with("curated/") {
                "curated"
            } else {
                "local"
            };
            let hero = if hero_rel == Some(img.rel_path.as_str()) {
                "H"
            } else {
                ""
            };
            Row::new(vec![
                hero.to_string(),
                format!("{idx:02}"),
                name.to_string(),
                source.to_string(),
                created,
            ])
        })
        .collect::<Vec<_>>();

    let mut state = TableState::default();
    state.select(Some(app.session_frame_selected.min(count - 1)));

    let table = Table::new(
        rows,
        [
            Constraint::Length(3),
            Constraint::Length(4),
            Constraint::Percentage(55),
            Constraint::Length(10),
            Constraint::Length(10),
        ],
    )
    .header(Row::new(vec!["H", "#", "Filename", "Src", "Time"]).style(mondrian_title(style)))
    .row_highlight_style(
        Style::default()
            .fg(theme.panel)
            .bg(theme.accent)
            .add_modifier(Modifier::BOLD),
    )
    .highlight_symbol("> ")
    .style(mondrian_style(style));

    frame.render_stateful_widget(table, chunks[1], &mut state);
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
    let title = if app.structure_editing {
        "Structure JSON (editing)"
    } else {
        "Structure Fields"
    };
    let block = focus_block(mondrian_block(theme, title, style), focused, theme);
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
            let value = format_structure_value_inline(&entry.value);
            ListItem::new(format!("{}: {}", entry.path, value))
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
        lines.push(format_structure_value_full(&entry.value));
        lines.push(String::new());
        lines.push("Enter edit | r generate | E edit JSON".to_string());
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
    let title = if app.listings_editing {
        format!("Listing JSON (editing) · {marketplace}")
    } else {
        format!("Listing Fields · {marketplace}")
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
            let value = format_structure_value_inline(&entry.value);
            ListItem::new(format!("{}: {}", entry.label, value))
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
    let label = entries
        .get(selected)
        .map(|entry| entry.label)
        .unwrap_or("none");
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
        lines.push(String::new());
        lines.push(app.listings_field_edit_buffer.clone());
    } else if let Some(entry) = entries.get(selected) {
        if !listing_exists {
            lines.push("No listing data yet. Run r to draft or edit fields.".to_string());
            lines.push(String::new());
        }
        lines.push(format!("Field: {}", entry.label));
        lines.push(String::new());
        lines.push(format_structure_value_full(&entry.value));
        lines.push(String::new());
        lines.push("Enter edit | r draft | p full | E edit JSON | u upload".to_string());
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
    let text = format!(
        "captures dir: {}\nlog stderr: {}\n\nConfig:\n  base_url: {}\n  hermes api key: {}\n  hermes online: {}\n\nTALARIA_CAPTURES_DIR overrides base capture path.",
        app.captures_dir.display(),
        stderr,
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
    );
    frame.render_widget(
        Paragraph::new(text)
            .style(mondrian_style(style))
            .block(mondrian_block(theme, "Settings", style))
            .wrap(Wrap { trim: true }),
        chunks[0],
    );

    let fields = [
        ("Marketplace", SettingsField::Marketplace),
        ("Merchant Location Key", SettingsField::MerchantLocation),
        ("Fulfillment Policy ID", SettingsField::FulfillmentPolicy),
        ("Payment Policy ID", SettingsField::PaymentPolicy),
        ("Return Policy ID", SettingsField::ReturnPolicy),
    ];
    let rows = fields
        .iter()
        .enumerate()
        .map(|(idx, (label, field))| {
            let mut value = match field {
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
        .block(mondrian_block(theme, "eBay Settings", style))
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
        ("Marketplace", SettingsField::Marketplace),
        ("Merchant Location Key", SettingsField::MerchantLocation),
        ("Fulfillment Policy ID", SettingsField::FulfillmentPolicy),
        ("Payment Policy ID", SettingsField::PaymentPolicy),
        ("Return Policy ID", SettingsField::ReturnPolicy),
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
    };

    let mut lines = Vec::new();
    if app.settings_editing {
        lines.push(format!("Editing {label} (Enter save, Esc cancel)."));
        lines.push(String::new());
        lines.push(app.settings_edit_buffer.clone());
    } else {
        lines.push(format!("Field: {label}"));
        lines.push(String::new());
        lines.push(value);
        lines.push(String::new());
        lines.push("Enter edit | Esc cancel".to_string());
    }
    let body = lines.join("\n");
    frame.render_widget(
        Paragraph::new(body)
            .style(mondrian_style(style))
            .wrap(Wrap { trim: true }),
        inner,
    );
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
        "  ←/→: switch tabs (not in Products)",
        "  h/l: switch tabs",
        "  ?: help",
        "  q: quit",
        "",
        "Products grid:",
        "  n new product | Enter select | d delete (y confirm)",
        "  ↑/↓/←/→ move selection",
        "",
        "Products workspace:",
        "  Tab / Shift+Tab switch view (Context / Structure / Listings)",
        "  g back to grid",
        "",
        "Context view:",
        "  ←/→ focus Images/Text",
        "  ↑/↓ select image | Enter select frame or edit text | Del delete",
        "  s camera on/off | d/D device | c capture | b burst",
        "  x commit + upload | Esc abandon session",
        "",
        "Structure view:",
        "  ↑/↓ select field | Enter edit | r generate | E edit JSON",
        "  Esc save while editing",
        "",
        "Listings view:",
        "  ←/→ switch marketplace",
        "  ↑/↓ select field | Enter edit | E edit JSON",
        "  r run draft | p run full pipeline | u upload images",
        "  Esc save while editing",
        "",
        "Settings view:",
        "  ↑/↓ select field | Enter edit | Enter save | Esc cancel",
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
        "Camera: {camera}\nStream: {stream}  FPS: {:.1}  Dropped: {}\nCaptures: {}",
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
    let base = "←/→ tabs | h/l tabs | ? help | q quit";
    let base_no_arrows = "h/l tabs | ? help | q quit";
    match app.active_tab {
        AppTab::Products => match app.products_mode {
            crate::app::ProductsMode::Grid => {
                format!("{base_no_arrows} | n new | Enter select | d delete | ↑/↓/←/→ move")
            }
            crate::app::ProductsMode::Workspace => match app.products_subtab {
                crate::app::ProductsSubTab::Context => format!(
                    "{base_no_arrows} | Tab view | g grid | ←/→ focus | ↑/↓ select | Enter edit | Del delete | s camera on/off | d/D device | c capture | b burst | x commit + upload | Esc abandon"
                ),
                crate::app::ProductsSubTab::Structure => format!(
                    "{base_no_arrows} | Tab view | g grid | ↑/↓ select | Enter edit | r generate | E edit JSON"
                ),
                crate::app::ProductsSubTab::Listings => {
                    format!(
                        "{base_no_arrows} | Tab view | g grid | ←/→ marketplace | ↑/↓ field | Enter edit | r draft | p full | E edit JSON | u upload"
                    )
                }
            },
        },
        AppTab::Settings => format!("{base} | ↑/↓ select | Enter edit | Enter save | Esc cancel"),
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
